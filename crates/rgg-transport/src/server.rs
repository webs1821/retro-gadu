// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
use crate::{FrameCodec, TransportError};
use bytes::BytesMut;
use futures_util::{SinkExt, StreamExt};
use rand::TryRngCore;
use rgg_application::{
    contacts::ArchiveResult,
    messaging::{Authenticator, MessagingError, PasswordProof, SessionKey},
    presence::{Availability, Presence},
    sessions::{AccountHandle, IoFence, SessionHub},
    telemetry::{
        Admission, ConnectionEnd, ConnectionObservation, Operation, Outcome, RuntimeTelemetry,
    },
};
use rgg_protocol::{
    frame::Frame, gg6, gg6_contacts as archive_wire, gg6_directory as directory_wire,
    gg6_messages as wire, gg6_presence as presence_wire, legacy_hash::gg32,
};
use std::{collections::HashMap, net::IpAddr, sync::Arc, time::Duration};
use subtle::ConstantTimeEq;
use tokio::{
    io::AsyncWriteExt,
    net::{TcpListener, TcpStream},
    task::JoinSet,
    time::{Instant, timeout, timeout_at},
};
use tokio_util::{
    codec::{Decoder, Encoder, Framed},
    sync::DropGuard,
};

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Limits {
    pub max_connections: usize,
    pub per_ip: usize,
    pub login_seconds: u64,
    pub initial_seconds: u64,
    pub idle_seconds: u64,
    pub write_millis: u64,
    pub archive_seconds: u64,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            max_connections: 64,
            per_ip: 16,
            login_seconds: 5,
            initial_seconds: 5,
            idle_seconds: 90,
            write_millis: 2000,
            archive_seconds: 10,
        }
    }
}
impl Limits {
    pub fn valid(&self) -> bool {
        (1..=128).contains(&self.max_connections)
            && (1..=self.max_connections).contains(&self.per_ip)
            && (1..=60).contains(&self.login_seconds)
            && (1..=60).contains(&self.initial_seconds)
            && (5..=3600).contains(&self.idle_seconds)
            && (100..=5000).contains(&self.write_millis)
            && (1..=60).contains(&self.archive_seconds)
    }
}
pub struct Context {
    pub hub: Arc<SessionHub>,
    pub authentication: Authenticator,
    pub limits: Limits,
    pub images: Arc<dyn rgg_application::images::ImageTransfers>,
    pub telemetry: Arc<RuntimeTelemetry>,
}
struct Proof {
    seed: u32,
    response: u32,
}
impl PasswordProof for Proof {
    fn matches(&self, password: &[u8]) -> bool {
        bool::from(
            gg32(password, self.seed)
                .to_le_bytes()
                .ct_eq(&self.response.to_le_bytes()),
        )
    }
}

struct Inbound;
impl Decoder for Inbound {
    type Item = Frame;
    type Error = TransportError;
    fn decode(&mut self, b: &mut BytesMut) -> Result<Option<Frame>, Self::Error> {
        if b.len() >= 8 {
            let kind = u32::from_le_bytes(b[..4].try_into().expect("header"));
            let len = u32::from_le_bytes(b[4..8].try_into().expect("header")) as usize;
            if len > wire::inbound_limit(kind) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "profile packet limit",
                )
                .into());
            }
        }
        FrameCodec.decode(b)
    }
    fn decode_eof(&mut self, b: &mut BytesMut) -> Result<Option<Frame>, Self::Error> {
        let f = self.decode(b)?;
        if f.is_none() {
            rgg_protocol::frame::finish(b)?;
        }
        Ok(f)
    }
}
impl Encoder<Frame> for Inbound {
    type Error = TransportError;
    fn encode(&mut self, f: Frame, b: &mut BytesMut) -> Result<(), Self::Error> {
        FrameCodec.encode(f, b)
    }
}
// Field drop order is essential: socket BEFORE stopped signal BEFORE the actor's last owner.
struct Socket {
    io: Framed<TcpStream, Inbound>,
    _stopped: DropGuard,
    actor: Option<Arc<AccountHandle>>,
    id: [u8; 16],
    key: Option<SessionKey>,
    // Dropped after the socket, including when its task is aborted.
    observation: ConnectionObservation,
}
type Failure = Box<dyn std::error::Error + Send + Sync>;
#[derive(Debug, thiserror::Error)]
enum ConversationStop {
    #[error("closed")]
    Closed,
    #[error("login required")]
    LoginRequired,
    #[error("idle deadline")]
    Idle,
    #[error("initial state deadline")]
    Initial,
    #[error("ambiguous or incomplete contact upload")]
    Upload,
    #[error("packet rate")]
    Rate,
    #[error("unexpected command")]
    Unexpected,
}
fn application_outcome(error: &MessagingError) -> Outcome {
    match error {
        MessagingError::Stale => Outcome::Stale,
        MessagingError::NotReady => Outcome::NotReady,
        MessagingError::Rejected => Outcome::Rejected,
        MessagingError::Quota => Outcome::Quota,
        MessagingError::Invalid => Outcome::Invalid,
        MessagingError::Storage => Outcome::Storage,
        MessagingError::Integrity => Outcome::Integrity,
        MessagingError::Busy => Outcome::Busy,
    }
}
fn failure_outcome(error: &Failure) -> Outcome {
    if let Some(error) = error.downcast_ref::<MessagingError>() {
        return application_outcome(error);
    }
    if error.is::<tokio::time::error::Elapsed>() {
        return Outcome::Timeout;
    }
    if error.is::<gg6::DecodeError>() {
        return Outcome::Invalid;
    }
    if let Some(error) = error.downcast_ref::<TransportError>() {
        return match error {
            TransportError::Frame(_) => Outcome::Invalid,
            TransportError::Io(error) if error.kind() == std::io::ErrorKind::InvalidData => {
                Outcome::Invalid
            }
            TransportError::Io(_) => Outcome::Transport,
        };
    }
    if let Some(error) = error.downcast_ref::<ConversationStop>() {
        return match error {
            ConversationStop::Closed => Outcome::Transport,
            ConversationStop::LoginRequired | ConversationStop::Unexpected => Outcome::Invalid,
            ConversationStop::Idle | ConversationStop::Initial | ConversationStop::Upload => {
                Outcome::Timeout
            }
            ConversationStop::Rate => Outcome::Other,
        };
    }
    Outcome::Other
}
fn connection_end(result: &Result<(), Failure>) -> ConnectionEnd {
    let Err(error) = result else {
        return ConnectionEnd::Finished;
    };
    match error.downcast_ref::<ConversationStop>() {
        Some(ConversationStop::Closed) => return ConnectionEnd::PeerClosed,
        Some(ConversationStop::Rate) => return ConnectionEnd::Rate,
        _ => (),
    }
    match failure_outcome(error) {
        Outcome::Invalid => ConnectionEnd::Protocol,
        Outcome::Timeout => ConnectionEnd::Timeout,
        Outcome::Transport => ConnectionEnd::Io,
        Outcome::Storage => ConnectionEnd::Storage,
        Outcome::Stale => ConnectionEnd::Stale,
        _ => ConnectionEnd::Other,
    }
}
async fn send(socket: &mut Socket, frame: Frame, limits: &Limits) -> Result<(), Failure> {
    timeout(
        Duration::from_millis(limits.write_millis),
        socket.io.send(frame),
    )
    .await??;
    Ok(())
}
#[derive(Default)]
struct PresenceCache {
    revision: u64,
    entries: HashMap<u32, (Presence, u8)>,
}
async fn publish_presence(
    socket: &mut Socket,
    ctx: &Context,
    key: &SessionKey,
    cache: &mut PresenceCache,
    initial: bool,
) -> Result<(), Failure> {
    if !initial && cache.revision == ctx.hub.presence_revision() {
        return Ok(());
    }
    timeout(Duration::from_millis(ctx.limits.write_millis), async {
        // Do not await an account actor while holding this gate: mutations use its exclusive side.
        let publication = ctx.hub.presence_snapshot(key).await?;
        if initial {
            for chunk in publication.entries.chunks(128) {
                socket.io.send(presence_wire::snapshot(chunk)?).await?;
            }
        } else {
            for view in &publication.entries {
                if cache.entries.get(&view.user.get())
                    != Some(&(view.presence.clone(), view.image_limit_kib))
                {
                    socket.io.send(presence_wire::changed(view)?).await?;
                }
            }
        }
        cache.entries = publication
            .entries
            .iter()
            .map(|v| (v.user.get(), (v.presence.clone(), v.image_limit_kib)))
            .collect();
        cache.revision = publication.revision;
        Ok::<(), Failure>(())
    })
    .await??;
    Ok(())
}

pub async fn serve(listener: TcpListener, context: Arc<Context>) -> std::io::Result<()> {
    let mut tasks = JoinSet::new();
    let mut ips: HashMap<IpAddr, usize> = HashMap::new();
    let mut arrivals: HashMap<IpAddr, (f64, Instant)> = HashMap::new();
    let mut maintenance = tokio::time::interval(Duration::from_secs(30));
    maintenance.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let result = loop {
        tokio::select! {biased;
            _=context.hub.shutdown.cancelled()=>break Ok(()),
            result=tasks.join_next(),if !tasks.is_empty()=>{if let Some(Ok(ip))=result&& let Some(count)=ips.get_mut(&ip){*count-=1;if *count==0{ips.remove(&ip);}}},
            _=maintenance.tick()=>{if context.hub.prune().await.is_err(){break Err(std::io::Error::other("queue maintenance failed"));}},
            incoming=listener.accept()=>{
                let (stream,address)=match incoming{Ok(v)=>v,Err(e)=>break Err(e)};let ip=address.ip();let now=Instant::now();
                arrivals.retain(|_,(_,last)|now.duration_since(*last)<Duration::from_secs(60));
                if !arrivals.contains_key(&ip)&&arrivals.len()>=512{context.telemetry.admission(Admission::AddressCapacity);continue;}
                let (tokens,last)=arrivals.entry(ip).or_insert((64.0,now));*tokens=(*tokens+now.duration_since(*last).as_secs_f64()*16.0).min(64.0);*last=now;
                if *tokens<1.0{context.telemetry.admission(Admission::Rate);continue;}
                if tasks.len()>=context.limits.max_connections{context.telemetry.admission(Admission::GlobalLimit);continue;}
                if ips.get(&ip).copied().unwrap_or(0)>=context.limits.per_ip{context.telemetry.admission(Admission::IpLimit);continue;}*tokens-=1.0;
                stream.set_nodelay(true)?;socket2::SockRef::from(&stream).set_send_buffer_size(32*1024)?;
                context.telemetry.admission(Admission::Accepted);
                *ips.entry(ip).or_default()+=1;let ctx=context.clone();tasks.spawn(async move{connection(stream,ctx).await;ip});
            }
        }
    };
    context.hub.shutdown.cancel();
    drop(listener);
    if timeout(Duration::from_secs(8), async {
        while tasks.join_next().await.is_some() {}
    })
    .await
    .is_err()
    {
        tasks.abort_all();
        while tasks.join_next().await.is_some() {}
    }
    context.hub.finish_shutdown().await;
    result
}
async fn connection(stream: TcpStream, context: Arc<Context>) {
    let fence = IoFence::new();
    let mut socket = Socket {
        io: Framed::new(stream, Inbound),
        _stopped: fence.stopped.clone().drop_guard(),
        actor: None,
        id: [0; 16],
        key: None,
        observation: context.telemetry.connection(),
    };
    let end = tokio::select! {biased;
        _=context.hub.shutdown.cancelled()=>ConnectionEnd::Shutdown,
        _=fence.stop.cancelled()=>ConnectionEnd::Replaced,
        result=conversation(&mut socket,&context,&fence)=>connection_end(&result),
    };
    socket.observation.finish(end);
    if fence.replaced() && socket.io.write_buffer().is_empty() {
        let _ = timeout(Duration::from_millis(200), socket.io.send(wire::empty(0xb))).await;
    }
    let _ = timeout(Duration::from_millis(200), socket.io.get_mut().shutdown()).await;
    let cleanup = socket.actor.clone().map(|actor| (actor, socket.id));
    if let Some(key) = &socket.key {
        context.images.disconnect(key);
    }
    drop(socket);
    if let Some((actor, id)) = cleanup {
        let _ = actor.closed(id).await;
    }
}
async fn conversation(socket: &mut Socket, ctx: &Context, fence: &IoFence) -> Result<(), Failure> {
    let mut login_observation = ctx.telemetry.operation(Operation::Login);
    let mut random = [0; 4];
    rand::rngs::OsRng.try_fill_bytes(&mut random).map_err(|_| {
        login_observation.finish(Outcome::Other);
        "random unavailable"
    })?;
    let seed = u32::from_le_bytes(random);
    let login_deadline = Instant::now() + Duration::from_secs(ctx.limits.login_seconds);
    let login = timeout_at(login_deadline, async {
        send(socket, gg6::welcome(seed), &ctx.limits).await?;
        let frame = socket.io.next().await.ok_or(ConversationStop::Closed)??;
        if frame.kind != gg6::LOGIN60 {
            return Err(ConversationStop::LoginRequired.into());
        }
        let login = gg6::decode_login(&frame.payload)?;
        presence_wire::from_login(&login)?;
        let identity = ctx
            .authentication
            .authenticate(
                login.user,
                &Proof {
                    seed,
                    response: login.response,
                },
            )
            .await?;
        Ok::<_, Failure>((login, identity))
    })
    .await;
    let (login, identity) = match login {
        Ok(Ok(v)) => v,
        failed => {
            login_observation.finish(match &failed {
                Err(_) => Outcome::Timeout,
                Ok(Err(error)) => failure_outcome(error),
                Ok(Ok(_)) => unreachable!("successful branch handled"),
            });
            send(socket, wire::empty(9), &ctx.limits).await?;
            return Ok(());
        }
    };
    let actor = ctx
        .hub
        .account(login.user)
        .await
        .inspect_err(|e| login_observation.finish(application_outcome(e)))?;
    socket.actor = Some(actor.clone());
    rand::rngs::OsRng
        .try_fill_bytes(&mut socket.id)
        .map_err(|_| {
            login_observation.finish(Outcome::Other);
            "random unavailable"
        })?;
    let key = actor
        .attach(identity, socket.id, fence.clone())
        .await
        .inspect_err(|e| login_observation.finish(application_outcome(e)))?;
    socket.key = Some(key.clone());
    ctx.hub
        .set_image_limit(&key, login.image_limit_kib)
        .await
        .inspect_err(|e| login_observation.finish(application_outcome(e)))?;
    send(socket, wire::empty(3), &ctx.limits)
        .await
        .inspect_err(|e| login_observation.finish(failure_outcome(e)))?;
    login_observation.finish(Outcome::Success);
    drop(login_observation);
    let mut ready = false;
    let mut staging = None;
    let mut initial = Instant::now() + Duration::from_secs(ctx.limits.initial_seconds);
    let mut own_presence = presence_wire::from_login(&login)?;
    let mut presence_cache = PresenceCache::default();
    let mut upload_replies: Vec<u8> = Vec::new();
    let mut upload_deadline = Instant::now();
    let mut idle = Instant::now() + Duration::from_secs(ctx.limits.idle_seconds);
    let mut poll = tokio::time::interval(Duration::from_millis(500));
    poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut tokens = 400.0f64;
    let mut last = Instant::now();
    let mut drain = false;
    loop {
        let frame = tokio::select! {biased;
            _=tokio::time::sleep_until(idle)=>return Err(ConversationStop::Idle.into()),
            _=tokio::time::sleep_until(initial),if !ready=>return Err(ConversationStop::Initial.into()),
            _=tokio::time::sleep_until(upload_deadline),if !upload_replies.is_empty()=>return Err(ConversationStop::Upload.into()),
            frame=socket.io.next()=>Some(frame.ok_or(ConversationStop::Closed)??),
            _=actor.wake.notified(),if ready=>None,
            _=poll.tick(),if ready=>None,
            _=std::future::ready(()),if ready&&drain=>None,
        };
        if let Some(frame) = frame {
            let now = Instant::now();
            tokens = (tokens + now.duration_since(last).as_secs_f64() * 100.0).min(400.0);
            last = now;
            if tokens < 1.0 {
                return Err(ConversationStop::Rate.into());
            }
            tokens -= 1.0;
            idle = now + Duration::from_secs(ctx.limits.idle_seconds);
            match frame.kind {
                8 if frame.payload.is_empty() => send(socket, wire::empty(7), &ctx.limits).await?,
                0xf | 0x10 | 0x12 => {
                    let rules = wire::contacts(&frame.payload)?;
                    if frame.kind == 0xf {
                        if let Some(all) = &mut staging {
                            let all: &mut Vec<_> = all;
                            all.extend(rules);
                            if all.len() > 4000 {
                                return Err("contact quota".into());
                            }
                        } else {
                            actor.begin_initial(key.clone()).await?;
                            if ready {
                                initial = Instant::now()
                                    + Duration::from_secs(ctx.limits.initial_seconds);
                            }
                            ready = false;
                            staging = Some(rules);
                        }
                    } else {
                        if frame.kind == 0x12 && staging.is_some() {
                            return Err("EMPTY during staging".into());
                        }
                        let mut all = staging.take().unwrap_or_default();
                        all.extend(rules);
                        if all.len() > 4000 {
                            return Err("contact quota".into());
                        }
                        actor
                            .privacy(key.clone(), all, own_presence.clone())
                            .await?;
                        ready = true;
                        drain = true;
                        if own_presence.availability == Availability::Offline {
                            return Ok(());
                        }
                        publish_presence(socket, ctx, &key, &mut presence_cache, true).await?;
                    }
                }
                0xd | 0xe if ready => {
                    let rules = wire::contacts(&frame.payload)?;
                    if rules.len() != 1 {
                        return Err("contact length".into());
                    }
                    actor
                        .contact(key.clone(), rules[0], frame.kind == 0xe)
                        .await?;
                    publish_presence(socket, ctx, &key, &mut presence_cache, false).await?;
                }
                2 => {
                    own_presence = presence_wire::decode_status(&frame.payload)?;
                    if ready {
                        actor.presence(key.clone(), own_presence.clone()).await?;
                    }
                    if own_presence.availability == Availability::Offline {
                        return Ok(());
                    }
                }
                0x16 if ready => {
                    match archive_wire::decode(&frame.payload)? {
                        archive_wire::Request::Get => {
                            timeout(Duration::from_millis(ctx.limits.write_millis), async {
                                let archive = actor.export(key.clone()).await?;
                                for frame in archive_wire::export(&archive.text)? {
                                    socket.io.send(frame).await?;
                                }
                                Ok::<(), Failure>(())
                            })
                            .await??;
                        }
                        archive_wire::Request::Put(segment) => {
                            if segment.start {
                                upload_replies.clear();
                                upload_deadline = Instant::now()
                                    + Duration::from_secs(ctx.limits.archive_seconds);
                            }
                            let ack = if segment.start { 0 } else { 2 };
                            let result = actor.archive(key.clone(), segment).await?;
                            upload_replies.push(ack);
                            if upload_replies.len() > 130 {
                                return Err("archive fragment quota".into());
                            }
                            if matches!(result, ArchiveResult::Published(_)) {
                                // No success acknowledgement escapes before atomic publication.
                                timeout(Duration::from_millis(ctx.limits.write_millis), async {
                                    for ack in upload_replies.drain(..) {
                                        socket.io.send(archive_wire::reply(ack, &[])).await?;
                                    }
                                    Ok::<(), TransportError>(())
                                })
                                .await??;
                            }
                        }
                    }
                }
                0x14 if ready => {
                    let (operation, sequence) =
                        directory_wire::header(&frame.payload).ok_or("directory header")?;
                    match directory_wire::decode(&frame.payload) {
                        Err(_) => {
                            send(
                                socket,
                                directory_wire::failure(operation, sequence),
                                &ctx.limits,
                            )
                            .await?
                        }
                        Ok(request) => match request.operation {
                            directory_wire::Operation::Write(profile) => {
                                let response = match actor.profile(key.clone(), profile).await {
                                    Ok(()) => directory_wire::written(sequence),
                                    Err(MessagingError::Invalid | MessagingError::Quota) => {
                                        directory_wire::failure(operation, sequence)
                                    }
                                    Err(e) => return Err(e.into()),
                                };
                                send(socket, response, &ctx.limits).await?;
                            }
                            directory_wire::Operation::Read(query) => {
                                timeout(Duration::from_millis(ctx.limits.write_millis), async {
                                    match ctx.hub.directory(&key, query).await {
                                        Ok(publication) => {
                                            let response = directory_wire::result(
                                                sequence,
                                                key.user,
                                                &publication.result,
                                            )?;
                                            socket.io.send(response).await?;
                                            drop(publication);
                                        }
                                        Err(MessagingError::Invalid | MessagingError::Quota) => {
                                            socket
                                                .io
                                                .send(directory_wire::failure(operation, sequence))
                                                .await?
                                        }
                                        Err(e) => return Err(e.into()),
                                    }
                                    Ok::<(), Failure>(())
                                })
                                .await??;
                            }
                        },
                    }
                }
                0xb if ready => {
                    let (recipient, sequence, no_ack) =
                        wire::message_header(&frame.payload).ok_or("message header")?;
                    if frame.payload.get(12) == Some(&0) {
                        match rgg_protocol::gg6_images::decode(&frame.payload) {
                            Ok((peer, control)) => {
                                timeout(Duration::from_millis(ctx.limits.write_millis), async {
                                    let publication = ctx
                                        .hub
                                        .image_command(&key, peer, control, ctx.images.as_ref())
                                        .await?;
                                    if !no_ack {
                                        socket
                                            .io
                                            .send(wire::acknowledgement(
                                                publication.status,
                                                recipient,
                                                sequence,
                                            ))
                                            .await?;
                                    }
                                    if let Some((sender, control)) = &publication.response {
                                        socket
                                            .io
                                            .send(rgg_protocol::gg6_images::received(
                                                *sender, 0, control,
                                            )?)
                                            .await?;
                                    }
                                    drop(publication);
                                    Ok::<(), Failure>(())
                                })
                                .await??;
                                ctx.hub.wake(peer).await;
                            }
                            Err(_) => {
                                if !no_ack {
                                    send(
                                        socket,
                                        wire::acknowledgement(6, recipient, sequence),
                                        &ctx.limits,
                                    )
                                    .await?;
                                }
                            }
                        }
                        continue;
                    }
                    let mut acceptance = ctx.telemetry.operation(Operation::Accept);
                    let status = match wire::decode_message(&frame.payload) {
                        Err(_) => {
                            acceptance.finish(Outcome::Invalid);
                            6
                        }
                        Ok(message) => {
                            let user = message.recipient;
                            match actor.accept(key.clone(), message).await {
                                Ok(_) => {
                                    acceptance.finish(Outcome::Success);
                                    #[cfg(feature = "test-hooks")]
                                    fault("after_accept_commit", sequence);
                                    #[cfg(feature = "test-hooks")]
                                    let drop_wake =
                                        std::env::var_os("RGG_TEST_DROP_WAKEUPS").is_some();
                                    #[cfg(not(feature = "test-hooks"))]
                                    let drop_wake = false;
                                    if !drop_wake {
                                        ctx.hub.wake(user).await;
                                    }
                                    3
                                }
                                Err(MessagingError::Quota) => {
                                    acceptance.finish(Outcome::Quota);
                                    4
                                }
                                Err(e @ (MessagingError::Rejected | MessagingError::Invalid)) => {
                                    acceptance.finish(application_outcome(&e));
                                    6
                                }
                                Err(e) => {
                                    acceptance.finish(application_outcome(&e));
                                    return Err(e.into());
                                }
                            }
                        }
                    };
                    if !no_ack {
                        send(
                            socket,
                            wire::acknowledgement(status, recipient, sequence),
                            &ctx.limits,
                        )
                        .await?;
                    }
                }
                _ => return Err(ConversationStop::Unexpected.into()),
            }
        } else {
            publish_presence(socket, ctx, &key, &mut presence_cache, false).await?;
            let image_sent = timeout(Duration::from_millis(ctx.limits.write_millis), async {
                let publication = ctx.hub.next_image(&key, ctx.images.as_ref()).await?;
                let present = publication.response.is_some();
                if let Some((sender, control)) = &publication.response {
                    socket
                        .io
                        .send(rgg_protocol::gg6_images::received(*sender, 0, control)?)
                        .await?;
                }
                drop(publication);
                Ok::<_, Failure>(present)
            })
            .await??;
            if image_sent {
                drain = true;
                continue;
            }
            // The monotonic deadline includes the claim wait; UTC is never a write authorization.
            let mut delivery = ctx.telemetry.operation(Operation::Delivery);
            let deadline = Instant::now() + Duration::from_millis(ctx.limits.write_millis);
            let claimed = timeout_at(deadline, actor.claim(key.clone()))
                .await
                .inspect_err(|_| delivery.finish(Outcome::Timeout))?
                .inspect_err(|e| delivery.finish(application_outcome(e)))?;
            if let Some(lease) = claimed {
                #[cfg(feature = "test-hooks")]
                fault("before_delivery_write", lease.client_sequence);
                let frame =
                    wire::received(&lease).inspect_err(|_| delivery.finish(Outcome::Invalid))?;
                timeout_at(deadline, socket.io.send(frame))
                    .await
                    .inspect_err(|_| delivery.finish(Outcome::Timeout))?
                    .inspect_err(|_| delivery.finish(Outcome::Transport))?;
                #[cfg(feature = "test-hooks")]
                fault("after_delivery_write", lease.client_sequence);
                actor
                    .written(key.clone(), lease)
                    .await
                    .inspect_err(|e| delivery.finish(application_outcome(e)))?;
                delivery.finish(Outcome::Success);
                drain = true;
            } else {
                delivery.discard_empty_poll();
                drain = false;
            }
        }
    }
}
#[cfg(feature = "test-hooks")]
fn fault(stage: &str, sequence: u32) {
    if sequence == 0xfeed && std::env::var("RGG_TEST_CRASH").as_deref() == Ok(stage) {
        std::process::exit(86);
    }
}

#[cfg(test)]
mod telemetry_tests {
    use super::*;
    use tokio::io::AsyncReadExt;

    #[tokio::test]
    async fn abort_drops_the_real_socket_fence_connection_and_operation_observations() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mut client = TcpStream::connect(listener.local_addr().unwrap())
            .await
            .unwrap();
        let (stream, _) = listener.accept().await.unwrap();
        let telemetry = Arc::new(RuntimeTelemetry::default());
        let telemetry_task = telemetry.clone();
        let fence = IoFence::new();
        let stopped = fence.stopped.clone();
        let (started, received) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            let _socket = Socket {
                io: Framed::new(stream, Inbound),
                _stopped: fence.stopped.drop_guard(),
                actor: None,
                id: [0; 16],
                key: None,
                observation: telemetry_task.connection(),
            };
            let _login = telemetry_task.operation(Operation::Login);
            started.send(()).unwrap();
            std::future::pending::<()>().await;
        });
        received.await.unwrap();
        assert_eq!(telemetry.snapshot().active_connections, 1);
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        assert!(stopped.is_cancelled());
        let snapshot = telemetry.snapshot();
        assert_eq!(snapshot.active_connections, 0);
        assert_eq!(snapshot.endings[ConnectionEnd::Cancelled as usize], 1);
        assert_eq!(
            snapshot.outcomes[Operation::Login as usize][Outcome::Cancelled as usize],
            1
        );
        assert_eq!(
            timeout(Duration::from_secs(1), client.read(&mut [0; 1]))
                .await
                .unwrap()
                .unwrap(),
            0
        );
    }
}
