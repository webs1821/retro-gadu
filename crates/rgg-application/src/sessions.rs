// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
//! Bounded per-account actors. No account actor ever waits on another account actor.
use crate::messaging::*;
use rgg_domain::UserId;
use std::{
    collections::HashMap,
    sync::{
        Arc, Weak,
        atomic::{AtomicU8, Ordering},
    },
    time::Duration,
};
use tokio::{
    sync::{Mutex, Notify, mpsc, oneshot},
    task::JoinSet,
};
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct IoFence {
    pub stop: CancellationToken,
    pub stopped: CancellationToken,
    reason: Arc<AtomicU8>,
}
impl Default for IoFence {
    fn default() -> Self {
        Self::new()
    }
}
impl IoFence {
    pub fn new() -> Self {
        Self {
            stop: CancellationToken::new(),
            stopped: CancellationToken::new(),
            reason: Arc::new(AtomicU8::new(0)),
        }
    }
    pub fn revoke(&self, replaced: bool) {
        self.reason
            .fetch_max(if replaced { 2 } else { 1 }, Ordering::Release);
        self.stop.cancel();
    }
    pub fn replaced(&self) -> bool {
        self.reason.load(Ordering::Acquire) == 2
    }
    async fn barrier(&self, replaced: bool) -> Result<(), MessagingError> {
        self.revoke(replaced);
        tokio::time::timeout(Duration::from_secs(6), self.stopped.cancelled())
            .await
            .map_err(|_| MessagingError::Busy)
    }
}

type Reply<T> = oneshot::Sender<Result<T, MessagingError>>;
enum Request {
    Attach(VerifiedIdentity, [u8; 16], IoFence, Reply<SessionKey>),
    Initial(SessionKey, Reply<()>),
    Privacy(SessionKey, Vec<ContactRule>, bool, Reply<()>),
    Contact(SessionKey, ContactRule, bool, Reply<()>),
    Friends(SessionKey, bool, Reply<()>),
    Accept(SessionKey, MessageBundle, Reply<i64>),
    Claim(SessionKey, Reply<Option<DeliveryLease>>),
    Written(SessionKey, DeliveryLease, Reply<()>),
    Closed([u8; 16], Reply<()>),
    Revoke(i64, Reply<()>),
}

pub struct AccountHandle {
    sender: mpsc::Sender<Request>,
    pub wake: Notify,
}
impl AccountHandle {
    async fn call<T>(&self, make: impl FnOnce(Reply<T>) -> Request) -> Result<T, MessagingError> {
        let (reply, receive) = oneshot::channel();
        tokio::time::timeout(Duration::from_secs(12), async {
            self.sender
                .send(make(reply))
                .await
                .map_err(|_| MessagingError::Busy)?;
            receive.await.map_err(|_| MessagingError::Busy)?
        })
        .await
        .map_err(|_| MessagingError::Busy)?
    }
    pub async fn attach(
        &self,
        identity: VerifiedIdentity,
        id: [u8; 16],
        fence: IoFence,
    ) -> Result<SessionKey, MessagingError> {
        self.call(|r| Request::Attach(identity, id, fence, r)).await
    }
    pub async fn begin_initial(&self, key: SessionKey) -> Result<(), MessagingError> {
        self.call(|r| Request::Initial(key, r)).await
    }
    pub async fn privacy(
        &self,
        key: SessionKey,
        rules: Vec<ContactRule>,
        friends: bool,
    ) -> Result<(), MessagingError> {
        self.call(|r| Request::Privacy(key, rules, friends, r))
            .await
    }
    pub async fn contact(
        &self,
        key: SessionKey,
        rule: ContactRule,
        remove: bool,
    ) -> Result<(), MessagingError> {
        self.call(|r| Request::Contact(key, rule, remove, r)).await
    }
    pub async fn friends(&self, key: SessionKey, value: bool) -> Result<(), MessagingError> {
        self.call(|r| Request::Friends(key, value, r)).await
    }
    pub async fn accept(
        &self,
        key: SessionKey,
        message: MessageBundle,
    ) -> Result<i64, MessagingError> {
        self.call(|r| Request::Accept(key, message, r)).await
    }
    pub async fn claim(&self, key: SessionKey) -> Result<Option<DeliveryLease>, MessagingError> {
        self.call(|r| Request::Claim(key, r)).await
    }
    pub async fn written(
        &self,
        key: SessionKey,
        lease: DeliveryLease,
    ) -> Result<(), MessagingError> {
        self.call(|r| Request::Written(key, lease, r)).await
    }
    /// The transport has already dropped its socket and signalled stopped, before awaiting us.
    pub async fn closed(&self, id: [u8; 16]) -> Result<(), MessagingError> {
        self.call(|r| Request::Closed(id, r)).await
    }
    pub async fn revoke_credentials(&self, version: i64) -> Result<(), MessagingError> {
        self.call(|r| Request::Revoke(version, r)).await
    }
}

pub struct SessionHub {
    repository: Arc<dyn MessagingRepository>,
    clock: Arc<dyn Clock>,
    limits: Arc<QueueLimits>,
    handles: Mutex<HashMap<UserId, Weak<AccountHandle>>>,
    tasks: Mutex<JoinSet<()>>,
    pub shutdown: CancellationToken,
}
impl SessionHub {
    pub fn new(
        repository: Arc<dyn MessagingRepository>,
        clock: Arc<dyn Clock>,
        limits: QueueLimits,
    ) -> Self {
        Self {
            repository,
            clock,
            limits: Arc::new(limits),
            handles: Mutex::new(HashMap::new()),
            tasks: Mutex::new(JoinSet::new()),
            shutdown: CancellationToken::new(),
        }
    }
    pub async fn account(&self, user: UserId) -> Result<Arc<AccountHandle>, MessagingError> {
        if self.shutdown.is_cancelled() {
            return Err(MessagingError::Busy);
        }
        let mut handles = self.handles.lock().await;
        if let Some(handle) = handles.get(&user).and_then(Weak::upgrade) {
            // A closed actor may still have a live socket. Do not create a second owner.
            return if handle.sender.is_closed() {
                Err(MessagingError::Busy)
            } else {
                Ok(handle)
            };
        }
        handles.retain(|_, v| v.strong_count() > 0);
        if handles.len() >= 128 {
            return Err(MessagingError::Busy);
        }
        let (sender, receiver) = mpsc::channel(32);
        let handle = Arc::new(AccountHandle {
            sender,
            wake: Notify::new(),
        });
        let actor = AccountActor {
            user,
            repository: self.repository.clone(),
            clock: self.clock.clone(),
            limits: self.limits.clone(),
            active: None,
            stop: self.shutdown.clone(),
        };
        let mut tasks = self.tasks.lock().await;
        while tasks.try_join_next().is_some() {}
        tasks.spawn(actor.run(receiver));
        handles.insert(user, Arc::downgrade(&handle));
        Ok(handle)
    }
    /// Called by the coordinator AFTER the sender actor's commit, without holding an actor lock.
    pub async fn wake(&self, user: UserId) {
        if let Some(handle) = self.handles.lock().await.get(&user).and_then(Weak::upgrade) {
            handle.wake.notify_one();
        }
    }
    pub async fn prune(&self) -> Result<(), MessagingError> {
        self.repository.prune(self.clock.unix_seconds()).await
    }
    pub async fn finish_shutdown(&self) {
        self.shutdown.cancel();
        let mut tasks = self.tasks.lock().await;
        if tokio::time::timeout(Duration::from_secs(8), async {
            while tasks.join_next().await.is_some() {}
        })
        .await
        .is_err()
        {
            tasks.abort_all();
            while tasks.join_next().await.is_some() {}
        }
    }
}

struct Active {
    key: SessionKey,
    fence: IoFence,
    ready: bool,
    lease: Option<[u8; 16]>,
}
struct AccountActor {
    user: UserId,
    repository: Arc<dyn MessagingRepository>,
    clock: Arc<dyn Clock>,
    limits: Arc<QueueLimits>,
    active: Option<Active>,
    stop: CancellationToken,
}
impl AccountActor {
    fn valid(&self, key: &SessionKey, ready: bool, idle: bool) -> Result<(), MessagingError> {
        let active = self.active.as_ref().ok_or(MessagingError::Stale)?;
        if &active.key != key || active.fence.stop.is_cancelled() {
            return Err(MessagingError::Stale);
        }
        if ready && !active.ready {
            return Err(MessagingError::NotReady);
        }
        if idle && active.lease.is_some() {
            return Err(MessagingError::Busy);
        }
        Ok(())
    }
    async fn attach(
        &mut self,
        identity: VerifiedIdentity,
        id: [u8; 16],
        fence: IoFence,
    ) -> Result<SessionKey, MessagingError> {
        if identity.user != self.user || fence.stopped.is_cancelled() {
            return Err(MessagingError::Stale);
        }
        if let Some(old) = &self.active {
            old.fence.barrier(true).await?;
        }
        let key = self
            .repository
            .attach_session_if_current(
                self.user,
                identity.credential_version,
                id,
                self.clock.unix_seconds(),
            )
            .await?;
        self.active = Some(Active {
            key: key.clone(),
            fence,
            ready: false,
            lease: None,
        });
        Ok(key)
    }
    async fn closed(&mut self, id: [u8; 16]) -> Result<(), MessagingError> {
        if let Some(active) = &self.active {
            if active.key.id != id {
                return Ok(());
            }
            if !active.fence.stopped.is_cancelled() {
                return Err(MessagingError::Busy);
            }
            let result = self
                .repository
                .detach_session(&active.key, self.clock.unix_seconds())
                .await;
            self.active = None;
            match result {
                Err(MessagingError::Stale) => Ok(()),
                other => other,
            }
        } else {
            Ok(())
        }
    }
    async fn run(mut self, mut receiver: mpsc::Receiver<Request>) {
        loop {
            let request = tokio::select! {biased;_=self.stop.cancelled()=>break,request=receiver.recv()=>match request{Some(r)=>r,None=>break}};
            let now = self.clock.unix_seconds();
            match request {
                Request::Attach(identity, id, fence, reply) => {
                    let _ = reply.send(self.attach(identity, id, fence).await);
                }
                Request::Initial(key, reply) => {
                    let result = async {
                        self.valid(&key, false, true)?;
                        self.repository.begin_initial_state(&key).await?;
                        self.active.as_mut().expect("validated").ready = false;
                        Ok(())
                    }
                    .await;
                    let _ = reply.send(result);
                }
                Request::Privacy(key, rules, friends, reply) => {
                    let result = async {
                        self.valid(&key, false, true)?;
                        self.repository
                            .replace_privacy(&key, &rules, friends, now)
                            .await?;
                        self.active.as_mut().expect("validated").ready = true;
                        Ok(())
                    }
                    .await;
                    let _ = reply.send(result);
                }
                Request::Contact(key, rule, remove, reply) => {
                    let result = async {
                        self.valid(&key, true, true)?;
                        self.repository
                            .change_contact(&key, rule, remove, now)
                            .await
                    }
                    .await;
                    let _ = reply.send(result);
                }
                Request::Friends(key, value, reply) => {
                    let result = async {
                        self.valid(&key, false, true)?;
                        self.repository.change_friends_only(&key, value).await
                    }
                    .await;
                    let _ = reply.send(result);
                }
                Request::Accept(key, message, reply) => {
                    let result = async {
                        self.valid(&key, true, false)?;
                        self.repository
                            .accept_message_bundle(&key, message, now, &self.limits)
                            .await
                    }
                    .await;
                    let _ = reply.send(result);
                }
                Request::Claim(key, reply) => {
                    let result = async {
                        self.valid(&key, true, true)?;
                        self.repository.claim_delivery(&key, now).await
                    }
                    .await;
                    if let Ok(Some(lease)) = &result {
                        self.active.as_mut().expect("validated").lease = Some(lease.token);
                    }
                    if let Err(Ok(Some(lease))) = reply.send(result) {
                        // No writer received this lease: no I/O permission escaped the actor.
                        let _ = self.repository.release_delivery(&key, &lease, now).await;
                        if let Some(active) = &mut self.active {
                            active.lease = None;
                        }
                    }
                }
                Request::Written(key, lease, reply) => {
                    let result = async {
                        self.valid(&key, true, false)?;
                        if self.active.as_ref().and_then(|a| a.lease) != Some(lease.token) {
                            return Err(MessagingError::Stale);
                        }
                        self.repository
                            .record_transport_write(&key, &lease, now)
                            .await?;
                        self.active.as_mut().expect("validated").lease = None;
                        Ok(())
                    }
                    .await;
                    let _ = reply.send(result);
                }
                Request::Closed(id, reply) => {
                    let _ = reply.send(self.closed(id).await);
                }
                Request::Revoke(version, reply) => {
                    let result = async {
                        if let Some(active) = &self.active {
                            active.fence.barrier(false).await?;
                        }
                        self.repository
                            .revoke_credentials(self.user, version, now)
                            .await?;
                        self.active = None;
                        Ok(())
                    }
                    .await;
                    let _ = reply.send(result);
                }
            }
        }
        if let Some(active) = self.active.take()
            && active.fence.barrier(false).await.is_ok()
        {
            let _ = self
                .repository
                .detach_session(&active.key, self.clock.unix_seconds())
                .await;
        }
    }
}
