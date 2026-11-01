// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
//! Bound HTTP sockets before parsing headers, including response-body backpressure.
use axum::serve::Listener;
use serde::Deserialize;
use std::{
    collections::HashMap,
    future::Future,
    io,
    net::{IpAddr, SocketAddr},
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll, ready},
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    net::{TcpListener, TcpStream},
    time::{Instant, Sleep, sleep_until},
};

#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Limits {
    pub max_connections: usize,
    pub per_ip: usize,
    pub idle_seconds: u64,
    pub write_seconds: u64,
    pub lifetime_seconds: u64,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            max_connections: 64,
            per_ip: 16,
            idle_seconds: 15,
            write_seconds: 10,
            lifetime_seconds: 600,
        }
    }
}
impl Limits {
    pub fn valid(&self) -> bool {
        (1..=1024).contains(&self.max_connections)
            && (1..=self.max_connections).contains(&self.per_ip)
            && (1..=300).contains(&self.idle_seconds)
            && (1..=120).contains(&self.write_seconds)
            && (2..=3600).contains(&self.lifetime_seconds)
            && self.lifetime_seconds >= self.idle_seconds.max(self.write_seconds)
    }
}

#[derive(Default)]
struct Counts {
    total: usize,
    by_ip: HashMap<IpAddr, usize>,
}
struct Admission {
    limits: Limits,
    counts: Mutex<Counts>,
}
impl Admission {
    fn acquire(self: &Arc<Self>, ip: IpAddr) -> Option<Permit> {
        let mut counts = self.counts.lock().expect("HTTP admission lock");
        if counts.total >= self.limits.max_connections
            || counts.by_ip.get(&ip).copied().unwrap_or(0) >= self.limits.per_ip
        {
            return None;
        }
        counts.total += 1;
        *counts.by_ip.entry(ip).or_default() += 1;
        Some(Permit {
            admission: self.clone(),
            ip,
        })
    }
}
struct Permit {
    admission: Arc<Admission>,
    ip: IpAddr,
}
impl Drop for Permit {
    fn drop(&mut self) {
        let mut counts = self.admission.counts.lock().expect("HTTP admission lock");
        counts.total -= 1;
        let count = counts.by_ip.get_mut(&self.ip).expect("HTTP peer permit");
        *count -= 1;
        if *count == 0 {
            counts.by_ip.remove(&self.ip);
        }
    }
}

pub struct BoundedListener {
    inner: TcpListener,
    admission: Arc<Admission>,
}
impl BoundedListener {
    pub fn new(inner: TcpListener, limits: Limits) -> Self {
        Self {
            inner,
            admission: Arc::new(Admission {
                limits,
                counts: Mutex::default(),
            }),
        }
    }
}
impl Listener for BoundedListener {
    type Io = BoundedIo<TcpStream>;
    type Addr = SocketAddr;
    async fn accept(&mut self) -> (Self::Io, SocketAddr) {
        loop {
            let (stream, peer) = Listener::accept(&mut self.inner).await;
            if let Some(permit) = self.admission.acquire(peer.ip()) {
                return (
                    BoundedIo::new(stream, &self.admission.limits, Some(permit)),
                    peer,
                );
            }
            // No HTTP task/header allocation and no per-rejection log amplification.
            drop(stream);
            tokio::task::yield_now().await;
        }
    }
    fn local_addr(&self) -> io::Result<SocketAddr> {
        self.inner.local_addr()
    }
}

pub struct BoundedIo<T> {
    inner: T,
    _permit: Option<Permit>,
    read_timer: Pin<Box<Sleep>>,
    write_timer: Pin<Box<Sleep>>,
    idle_deadline: Instant,
    lifetime_deadline: Instant,
    blocked_write: Option<Instant>,
    idle_duration: Duration,
    write_duration: Duration,
}
impl<T> BoundedIo<T> {
    fn new(inner: T, limits: &Limits, permit: Option<Permit>) -> Self {
        let idle_duration = Duration::from_secs(limits.idle_seconds);
        let now = Instant::now();
        let idle_deadline = now + idle_duration;
        Self {
            inner,
            _permit: permit,
            read_timer: Box::pin(sleep_until(idle_deadline)),
            write_timer: Box::pin(sleep_until(idle_deadline)),
            idle_deadline,
            lifetime_deadline: now + Duration::from_secs(limits.lifetime_seconds),
            blocked_write: None,
            idle_duration,
            write_duration: Duration::from_secs(limits.write_seconds),
        }
    }
    fn deadlines(&mut self, cx: &mut Context<'_>, writing: bool) -> io::Result<()> {
        let mut deadline = self.idle_deadline.min(self.lifetime_deadline);
        if let Some(blocked) = self.blocked_write {
            deadline = deadline.min(blocked);
        }
        // Separate registrations: a read task must not replace the write task's waker.
        // Progress can leave an earlier timer armed; its next poll recomputes the deadline.
        let timer = if writing {
            &mut self.write_timer
        } else {
            &mut self.read_timer
        };
        if timer.deadline() != deadline {
            timer.as_mut().reset(deadline);
        }
        if timer.as_mut().poll(cx).is_ready() {
            Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "HTTP socket deadline",
            ))
        } else {
            Ok(())
        }
    }
    fn progress(&mut self) {
        self.idle_deadline = Instant::now() + self.idle_duration;
    }
    fn write_result<R>(
        &mut self,
        cx: &mut Context<'_>,
        result: Poll<io::Result<R>>,
    ) -> Poll<io::Result<R>> {
        if result.is_pending() {
            self.blocked_write
                .get_or_insert_with(|| Instant::now() + self.write_duration);
            self.deadlines(cx, true)?;
        }
        result
    }
}
impl BoundedIo<TcpStream> {
    pub fn tcp_nodelay(&self) {
        let _ = self.inner.set_nodelay(true);
    }
}
impl<T: AsyncRead + Unpin> AsyncRead for BoundedIo<T> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        this.deadlines(cx, false)?;
        let before = buf.filled().len();
        let result = ready!(Pin::new(&mut this.inner).poll_read(cx, buf));
        if result.is_ok() && buf.filled().len() > before {
            this.progress();
        }
        Poll::Ready(result)
    }
}
impl<T: AsyncWrite + Unpin> AsyncWrite for BoundedIo<T> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        this.deadlines(cx, true)?;
        let result = Pin::new(&mut this.inner).poll_write(cx, buf);
        if matches!(result, Poll::Ready(Ok(n)) if n > 0) {
            this.progress();
            this.blocked_write = None;
        }
        this.write_result(cx, result)
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        this.deadlines(cx, true)?;
        let result = Pin::new(&mut this.inner).poll_flush(cx);
        this.write_result(cx, result)
    }
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        this.deadlines(cx, true)?;
        let result = Pin::new(&mut this.inner).poll_shutdown(cx);
        this.write_result(cx, result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt, duplex};
    fn limits() -> Limits {
        Limits {
            max_connections: 2,
            per_ip: 1,
            idle_seconds: 3,
            write_seconds: 1,
            lifetime_seconds: 10,
        }
    }

    #[test]
    fn admission_is_bounded_and_removes_inactive_addresses() {
        let state = Arc::new(Admission {
            limits: limits(),
            counts: Mutex::default(),
        });
        let a = "127.0.0.1".parse().unwrap();
        let b = "::1".parse().unwrap();
        let c = "127.0.0.2".parse().unwrap();
        let first = state.acquire(a).unwrap();
        assert!(state.acquire(a).is_none());
        let second = state.acquire(b).unwrap();
        assert!(state.acquire(c).is_none());
        drop(first);
        let replacement = state.acquire(c).unwrap();
        drop(second);
        drop(replacement);
        let counts = state.counts.lock().unwrap();
        assert_eq!(counts.total, 0);
        assert!(counts.by_ip.is_empty());
    }
    #[tokio::test(start_paused = true)]
    async fn both_directions_extend_idle_but_idle_eventually_expires() {
        let (stream, mut peer) = duplex(16);
        let mut stream = BoundedIo::new(stream, &limits(), None);
        tokio::time::advance(Duration::from_secs(2)).await;
        stream.write_all(b"x").await.unwrap(); // A long response does not need incoming bytes.
        tokio::time::advance(Duration::from_secs(2)).await;
        peer.write_all(b"y").await.unwrap();
        assert_eq!(stream.read_u8().await.unwrap(), b'y');
        let start = Instant::now();
        assert_eq!(
            stream.read_u8().await.unwrap_err().kind(),
            io::ErrorKind::TimedOut
        );
        assert_eq!(start.elapsed(), Duration::from_secs(3));
    }
    #[tokio::test(start_paused = true)]
    async fn lifetime_cannot_be_extended_by_continuous_traffic() {
        let (stream, mut peer) = duplex(16);
        let mut stream = BoundedIo::new(
            stream,
            &Limits {
                lifetime_seconds: 4,
                ..limits()
            },
            None,
        );
        tokio::time::advance(Duration::from_secs(2)).await;
        peer.write_all(b"x").await.unwrap();
        stream.read_u8().await.unwrap();
        tokio::time::advance(Duration::from_secs(2)).await;
        peer.write_all(b"x").await.unwrap();
        assert_eq!(
            stream.read_u8().await.unwrap_err().kind(),
            io::ErrorKind::TimedOut
        );
    }
    #[tokio::test(start_paused = true)]
    async fn blocked_output_is_not_kept_alive_by_incoming_bytes() {
        let (stream, mut peer) = duplex(1);
        let stream = BoundedIo::new(stream, &limits(), None);
        let (mut reader, mut writer) = tokio::io::split(stream);
        let task = tokio::spawn(async move { writer.write_all(b"xx").await });
        tokio::task::yield_now().await; // Second byte waits for output space.
        for _ in 0..4 {
            tokio::time::advance(Duration::from_millis(200)).await;
            peer.write_all(b"y").await.unwrap();
            assert_eq!(reader.read_u8().await.unwrap(), b'y');
        }
        assert_eq!(
            task.await.unwrap().unwrap_err().kind(),
            io::ErrorKind::TimedOut
        );
    }
    #[tokio::test(start_paused = true)]
    async fn empty_flush_does_not_reset_a_stalled_write() {
        let (stream, _peer) = duplex(1);
        let mut stream = BoundedIo::new(stream, &limits(), None);
        stream.write_all(b"x").await.unwrap();
        std::future::poll_fn(|cx| {
            assert!(Pin::new(&mut stream).poll_write(cx, b"y").is_pending());
            Poll::Ready(())
        })
        .await;
        tokio::time::advance(Duration::from_millis(800)).await;
        stream.flush().await.unwrap();
        tokio::time::advance(Duration::from_millis(200)).await;
        assert_eq!(
            stream.write(b"y").await.unwrap_err().kind(),
            io::ErrorKind::TimedOut
        );
    }
}
