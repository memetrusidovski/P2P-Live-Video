//! One UDP socket for both QUIC and the protocol's plain pre-session frames.
//!
//! QUIC v1 packets always have the fixed bit (0x40) set in their first byte;
//! BitStream frames start with the protocol version byte (0x01), whose fixed
//! bit is clear. [`DemuxSocket`] sits under quinn's endpoint, copies every
//! non-QUIC datagram to a channel for the driver, and lets quinn drop the
//! original as an undecodable packet.

use std::collections::VecDeque;
use std::io::{self, IoSliceMut};
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use bytes::Bytes;
use quinn::udp::{RecvMeta, Transmit};
use quinn::{AsyncUdpSocket, UdpPoller};
use tokio::sync::{mpsc, Notify};

/// Plain datagrams held while the socket is not writable. Low-rate control
/// traffic never needs more; the oldest is dropped past the bound.
const PLAIN_BACKLOG: usize = 8192;
/// Plain datagrams waiting for the driver. A full queue drops the newest.
///
/// WHY: The driver decodes one datagram per loop turn. An unbounded queue grows by
///      the excess of a flood until the process runs out of memory.
const PLAIN_QUEUE: usize = 4096;
/// Kernel socket buffers. The default (a few hundred KB) overflows at the
/// symbol rate of a loaded relay; the kernel caps the request where needed.
const SOCKET_BUFFER_BYTES: usize = 8 << 20;

/// A plain datagram received on the shared socket.
#[derive(Debug)]
pub struct PlainDatagram {
    /// Observed source.
    pub from: SocketAddr,
    /// Bytes.
    pub data: Bytes,
}

/// The demultiplexing socket.
#[derive(Debug)]
pub struct DemuxSocket {
    inner: Arc<dyn AsyncUdpSocket>,
    plain_tx: mpsc::Sender<PlainDatagram>,
    /// Per-source budgets for plain datagrams.
    limiter: Mutex<crate::ratelimit::PlainLimiter>,
    /// Plain datagrams that met a full socket buffer, oldest first.
    backlog: Mutex<VecDeque<(SocketAddr, Bytes)>>,
    /// Wakes the flush task when the backlog gains an entry.
    flush: Notify,
    /// Plain datagrams received.
    pub plain_in: AtomicU64,
    /// Plain sends that met a full socket buffer and were held.
    pub plain_held: AtomicU64,
    /// Plain datagrams dropped from a full backlog.
    pub plain_dropped: AtomicU64,
    /// Plain datagrams refused by the per-source budgets.
    pub plain_rate_limited: AtomicU64,
    /// Plain datagrams dropped because the driver's queue was full.
    pub plain_queue_dropped: AtomicU64,
}

impl DemuxSocket {
    /// Wrap a bound std socket for the tokio runtime. Must be called on a runtime:
    /// the flush task for held plain datagrams is spawned here.
    pub fn bind(addr: SocketAddr) -> io::Result<(Arc<Self>, mpsc::Receiver<PlainDatagram>)> {
        let std_sock = std::net::UdpSocket::bind(addr)?;
        std_sock.set_nonblocking(true)?;
        let sock2 = socket2::SockRef::from(&std_sock);
        let _ = sock2.set_send_buffer_size(SOCKET_BUFFER_BYTES);
        let _ = sock2.set_recv_buffer_size(SOCKET_BUFFER_BYTES);
        // WHY: The kernel caps the request without an error. A stock Linux kernel
        //      grants about a 20th of it, and a loaded relay then drops symbols.
        static WARNED: std::sync::Once = std::sync::Once::new();
        if let Ok(got) = sock2.recv_buffer_size() {
            if got < SOCKET_BUFFER_BYTES / 2 {
                WARNED.call_once(|| {
                    tracing::warn!(
                        "UDP receive buffer is {} KiB of the {} KiB asked for; on Linux raise \
                         net.core.rmem_max and net.core.wmem_max to {}",
                        got / 1024,
                        SOCKET_BUFFER_BYTES / 1024,
                        SOCKET_BUFFER_BYTES
                    );
                });
            }
        }
        let inner = quinn::Runtime::wrap_udp_socket(&quinn::TokioRuntime, std_sock)?;
        let (tx, rx) = mpsc::channel(PLAIN_QUEUE);
        let sock = Arc::new(Self {
            inner,
            plain_tx: tx,
            limiter: Mutex::new(crate::ratelimit::PlainLimiter::new(
                std::time::Instant::now(),
            )),
            backlog: Mutex::new(VecDeque::new()),
            flush: Notify::new(),
            plain_in: AtomicU64::new(0),
            plain_held: AtomicU64::new(0),
            plain_dropped: AtomicU64::new(0),
            plain_rate_limited: AtomicU64::new(0),
            plain_queue_dropped: AtomicU64::new(0),
        });
        tokio::spawn(Arc::clone(&sock).flush_backlog());
        Ok((sock, rx))
    }

    /// Send a plain (non-QUIC) datagram. A datagram the socket cannot take right
    /// now is held and sent when the socket is writable again.
    ///
    /// RULE: Never drop a plain frame on a full buffer. DHT answers and PONGs
    ///       have no retransmission. A relay at full symbol rate keeps the
    ///       buffer full for seconds at a time, and dropped answers lock joiners out.
    pub fn send_plain(&self, to: SocketAddr, data: &[u8]) -> io::Result<()> {
        let held = !self.backlog.lock().expect("backlog lock").is_empty();
        if !held {
            match self.try_send_plain(to, data) {
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
                other => return other,
            }
        }
        let mut backlog = self.backlog.lock().expect("backlog lock");
        if backlog.len() >= PLAIN_BACKLOG {
            backlog.pop_front();
            self.plain_dropped.fetch_add(1, Ordering::Relaxed);
        }
        self.plain_held.fetch_add(1, Ordering::Relaxed);
        backlog.push_back((to, Bytes::copy_from_slice(data)));
        drop(backlog);
        self.flush.notify_one();
        Ok(())
    }

    fn try_send_plain(&self, to: SocketAddr, data: &[u8]) -> io::Result<()> {
        self.inner.try_send(&Transmit {
            destination: to,
            ecn: None,
            contents: data,
            segment_size: None,
            src_ip: None,
        })
    }

    /// Drain the backlog in order each time the socket becomes writable.
    async fn flush_backlog(self: Arc<Self>) {
        let mut poller = self.inner.clone().create_io_poller();
        loop {
            self.flush.notified().await;
            loop {
                let next = self.backlog.lock().expect("backlog lock").front().cloned();
                let Some((to, data)) = next else { break };
                match self.try_send_plain(to, &data) {
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                        if std::future::poll_fn(|cx| poller.as_mut().poll_writable(cx))
                            .await
                            .is_err()
                        {
                            // A dead socket: hold the backlog for the next wake.
                            break;
                        }
                    }
                    // Sent, or failed for a reason a retry does not fix.
                    _ => {
                        self.backlog.lock().expect("backlog lock").pop_front();
                    }
                }
            }
        }
    }

    /// Replace the set of source addresses that have an open session. Their plain
    /// datagrams draw on the peer budget.
    pub fn set_peers(&self, peers: std::collections::HashSet<std::net::IpAddr>) {
        self.limiter.lock().expect("limiter lock").set_peers(peers);
    }

    /// Hand one received plain datagram to the driver, within the source's budget
    /// and the queue's bound.
    fn queue_plain(&self, from: SocketAddr, data: &[u8]) {
        let admitted = self
            .limiter
            .lock()
            .expect("limiter lock")
            .admit(from.ip(), std::time::Instant::now());
        if !admitted {
            self.plain_rate_limited.fetch_add(1, Ordering::Relaxed);
            return;
        }
        let d = PlainDatagram {
            from,
            data: Bytes::copy_from_slice(data),
        };
        if self.plain_tx.try_send(d).is_err() {
            self.plain_queue_dropped.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Whether a datagram is a QUIC packet (fixed bit set).
    #[inline]
    pub fn is_quic(first: u8) -> bool {
        first & 0x40 != 0
    }
}

impl AsyncUdpSocket for DemuxSocket {
    fn create_io_poller(self: Arc<Self>) -> Pin<Box<dyn UdpPoller>> {
        self.inner.clone().create_io_poller()
    }
    fn try_send(&self, transmit: &Transmit<'_>) -> io::Result<()> {
        self.inner.try_send(transmit)
    }
    fn poll_recv(
        &self,
        cx: &mut Context<'_>,
        bufs: &mut [IoSliceMut<'_>],
        meta: &mut [RecvMeta],
    ) -> Poll<io::Result<usize>> {
        let r = self.inner.poll_recv(cx, bufs, meta);
        if let Poll::Ready(Ok(n)) = &r {
            for i in 0..*n {
                let m = &meta[i];
                let stride = if m.stride == 0 { m.len } else { m.stride };
                let data = &bufs[i][..m.len];
                let mut off = 0;
                while off < m.len && stride > 0 {
                    let end = (off + stride).min(m.len);
                    let d = &data[off..end];
                    if !d.is_empty() && !Self::is_quic(d[0]) {
                        self.plain_in.fetch_add(1, Ordering::Relaxed);
                        self.queue_plain(m.addr, d);
                    }
                    off = end;
                }
            }
        }
        r
    }
    fn local_addr(&self) -> io::Result<SocketAddr> {
        self.inner.local_addr()
    }
    fn max_transmit_segments(&self) -> usize {
        self.inner.max_transmit_segments()
    }
    fn max_receive_segments(&self) -> usize {
        self.inner.max_receive_segments()
    }
    fn may_fragment(&self) -> bool {
        self.inner.may_fragment()
    }
}
