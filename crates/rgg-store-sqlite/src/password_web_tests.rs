// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
use crate::{SqliteStore, database as sqlx, messaging_tests::setup};
use rgg_application::{
    accounts::{
        AccountError, AccountRepository, LegacyPassword, SealedSecret, SecretError, SecretProtector,
    },
    messaging::{
        Authenticator, Clock, MessageBundle, MessagingError, MessagingRepository, PasswordProof,
        QueueLimits,
    },
    password::{PasswordChange, PasswordRepository, PasswordService},
    presence::Presence,
    sessions::{IoFence, SessionHub},
};
use rgg_credentials::Keyring;
use rgg_domain::{MessageText, UserId};
use std::{sync::Arc, time::Duration};
fn uid(n: u32) -> UserId {
    UserId::new(n).unwrap()
}
fn password(value: &str) -> LegacyPassword {
    LegacyPassword::new(zeroize::Zeroizing::new(value.as_bytes().to_vec())).unwrap()
}
fn change(old: &str, new: &str) -> PasswordChange {
    PasswordChange {
        user: uid(1),
        current: password(old),
        replacement: password(new),
    }
}
struct TestClock;
impl Clock for TestClock {
    fn unix_seconds(&self) -> i64 {
        100
    }
}
struct Proof(&'static [u8]);
impl PasswordProof for Proof {
    fn matches(&self, value: &[u8]) -> bool {
        value == self.0
    }
}
fn hub(store: &Arc<SqliteStore>) -> Arc<SessionHub> {
    Arc::new(SessionHub::new(
        store.clone(),
        Arc::new(TestClock),
        QueueLimits::default(),
    ))
}
async fn count(store: &SqliteStore, query: &str) -> i64 {
    sqlx::query_scalar(query)
        .fetch_one(&store.pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn socket_and_publication_barriers_precede_commit_and_stale_login_cannot_kick_new_session() {
    let (_root, store, ring) = setup().await;
    let hub = hub(&store);
    let auth = Authenticator::new(store.clone(), ring.clone()).unwrap();
    let service = Arc::new(PasswordService::new(store.clone(), ring, hub.clone()).unwrap());
    let actor = hub.account(uid(1)).await.unwrap();
    let observer = hub.account(uid(2)).await.unwrap();
    let old_fence = IoFence::new();
    let observer_fence = IoFence::new();
    let old = actor
        .attach(
            auth.authenticate(uid(1), &Proof(b"LocalTest9!"))
                .await
                .unwrap(),
            [1; 16],
            old_fence.clone(),
        )
        .await
        .unwrap();
    let watching = observer
        .attach(
            auth.authenticate(uid(2), &Proof(b"LocalTest9!"))
                .await
                .unwrap(),
            [2; 16],
            observer_fence.clone(),
        )
        .await
        .unwrap();
    observer
        .privacy(watching.clone(), vec![], Presence::offline())
        .await
        .unwrap();
    actor
        .privacy(old.clone(), vec![], Presence::offline())
        .await
        .unwrap();
    observer
        .accept(
            watching.clone(),
            MessageBundle {
                recipient: uid(1),
                client_sequence: 23,
                presentation: 8,
                text: MessageText::new("oczekuje w lease".into()).unwrap(),
                styles: vec![],
                conference: Default::default(),
            },
        )
        .await
        .unwrap();
    let leased = actor.claim(old.clone()).await.unwrap().unwrap();
    let publication = hub.presence_snapshot(&watching).await.unwrap();
    let stale = auth
        .authenticate(uid(1), &Proof(b"LocalTest9!"))
        .await
        .unwrap();
    let change_service = service.clone();
    let mut changing = tokio::spawn(async move {
        change_service
            .change(change("LocalTest9!", "NewLocalTest8!"))
            .await
    });
    tokio::time::timeout(Duration::from_secs(2), old_fence.stop.cancelled())
        .await
        .unwrap();
    assert!(!changing.is_finished());
    assert_eq!(
        store
            .login_credential(uid(1))
            .await
            .unwrap()
            .credential_version,
        1
    );
    old_fence.stopped.cancel();
    assert!(
        tokio::time::timeout(Duration::from_millis(30), &mut changing)
            .await
            .is_err()
    );
    assert_eq!(
        store
            .login_credential(uid(1))
            .await
            .unwrap()
            .credential_version,
        1
    );
    drop(publication);
    assert_eq!(changing.await.unwrap().unwrap(), 2);
    assert!(!store.session_is_current(&old).await.unwrap());
    assert_eq!(
        store
            .record_transport_write(&old, &leased, 100)
            .await
            .unwrap_err(),
        MessagingError::Stale
    );
    assert_eq!(
        count(
            &store,
            "SELECT count(*) FROM deliveries WHERE state='Pending'"
        )
        .await,
        1
    );
    assert_eq!(
        count(
            &store,
            "SELECT count(*) FROM delivery_attempts WHERE outcome='Interrupted'"
        )
        .await,
        1
    );
    assert_eq!(
        count(
            &store,
            "SELECT count(*) FROM admin_audit WHERE actor='local_web_with_current_password'"
        )
        .await,
        1
    );
    let newer = IoFence::new();
    let key = actor
        .attach(
            auth.authenticate(uid(1), &Proof(b"NewLocalTest8!"))
                .await
                .unwrap(),
            [3; 16],
            newer.clone(),
        )
        .await
        .unwrap();
    assert_eq!(
        actor
            .attach(stale, [4; 16], IoFence::new())
            .await
            .unwrap_err(),
        MessagingError::Stale
    );
    assert!(!newer.stop.is_cancelled());
    actor.closed(old.id).await.unwrap();
    assert!(store.session_is_current(&key).await.unwrap());
    actor
        .privacy(key.clone(), vec![], Presence::offline())
        .await
        .unwrap();
    let replay = actor.claim(key).await.unwrap().unwrap();
    assert_eq!(replay.id, leased.id);
    assert_ne!(replay.token, leased.token);
    newer.stopped.cancel();
    observer_fence.stopped.cancel();
    hub.finish_shutdown().await;
    store.close().await;
}

#[tokio::test]
async fn attempts_are_persistent_bounded_and_do_not_allocate_for_unknown_or_reset_on_clock_rollback()
 {
    let (root, store, _ring) = setup().await;
    for _ in 0..5 {
        assert!(store.charge_password_attempt(uid(1), 100).await.unwrap());
    }
    assert!(!store.charge_password_attempt(uid(1), 101).await.unwrap());
    assert!(!store.charge_password_attempt(uid(999), 101).await.unwrap());
    assert_eq!(
        count(&store, "SELECT count(*) FROM password_change_budget").await,
        1
    );
    store.close().await;
    drop(store);
    let store = SqliteStore::open(root.path()).await.unwrap();
    assert!(!store.charge_password_attempt(uid(1), 50).await.unwrap());
    assert!(!store.charge_password_attempt(uid(1), 699).await.unwrap());
    assert!(store.charge_password_attempt(uid(1), 700).await.unwrap());
    assert_eq!(
        count(
            &store,
            "SELECT attempts FROM password_change_budget WHERE uin=1"
        )
        .await,
        1
    );
    store.retire_account(uid(1)).await.unwrap();
    assert_eq!(
        count(&store, "SELECT count(*) FROM password_change_budget").await,
        0
    );
    store.close().await;
}

struct Reuse(Arc<Keyring>, SealedSecret);
impl SecretProtector for Reuse {
    fn open(&self, u: UserId, s: &SealedSecret) -> Result<LegacyPassword, SecretError> {
        self.0.open(u, s)
    }
    fn seal(&self, _: UserId, _: &LegacyPassword) -> Result<SealedSecret, SecretError> {
        Ok(self.1.clone())
    }
}
#[tokio::test]
async fn wrong_unchanged_and_reused_nonce_preserve_secret_generation_and_audit() {
    let (_root, store, ring) = setup().await;
    let hub = hub(&store);
    let service = PasswordService::new(store.clone(), ring.clone(), hub.clone()).unwrap();
    assert_eq!(
        service
            .change(change("WrongTest9!", "NewLocalTest8!"))
            .await
            .unwrap_err(),
        AccountError::IncorrectPassword
    );
    assert_eq!(
        service
            .change(change("LocalTest9!", "LocalTest9!"))
            .await
            .unwrap_err(),
        AccountError::UnchangedPassword
    );
    let original = store.login_credential(uid(1)).await.unwrap();
    let bad = PasswordService::new(
        store.clone(),
        Arc::new(Reuse(ring, original.secret.clone())),
        hub.clone(),
    )
    .unwrap();
    assert_eq!(
        bad.change(change("LocalTest9!", "NewLocalTest8!"))
            .await
            .unwrap_err(),
        AccountError::Secret(SecretError::NonceReuse)
    );
    let after = store.login_credential(uid(1)).await.unwrap();
    assert_eq!(after.secret.nonce, original.secret.nonce);
    assert_eq!(after.secret.ciphertext, original.secret.ciphertext);
    assert_eq!(after.credential_version, 1);
    assert_eq!(
        count(
            &store,
            "SELECT session_generation FROM accounts WHERE uin=1"
        )
        .await,
        0
    );
    assert_eq!(count(&store, "SELECT count(*) FROM admin_audit").await, 0);
    assert_eq!(count(&store, "SELECT count(*) FROM secret_nonces").await, 3);
    hub.finish_shutdown().await;
    store.close().await;
}

#[tokio::test]
async fn concurrent_changes_commit_exactly_one_secret_and_audit() {
    let (_root, store, ring) = setup().await;
    let hub = hub(&store);
    let service = PasswordService::new(store.clone(), ring.clone(), hub.clone()).unwrap();
    let (a, b) = tokio::join!(
        service.change(change("LocalTest9!", "NewLocalTest8!")),
        service.change(change("LocalTest9!", "OtherLocal9!"))
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    let after = store.login_credential(uid(1)).await.unwrap();
    assert_eq!(after.credential_version, 2);
    assert!(
        ring.open(uid(1), &after.secret)
            .unwrap()
            .matches(&password(if a.is_ok() {
                "NewLocalTest8!"
            } else {
                "OtherLocal9!"
            }))
    );
    assert_eq!(count(&store, "SELECT count(*) FROM admin_audit").await, 1);
    assert_eq!(count(&store, "SELECT count(*) FROM secret_nonces").await, 4);
    assert_eq!(
        count(
            &store,
            "SELECT attempts FROM password_change_budget WHERE uin=1"
        )
        .await,
        2
    );
    hub.finish_shutdown().await;
    store.close().await;
}
