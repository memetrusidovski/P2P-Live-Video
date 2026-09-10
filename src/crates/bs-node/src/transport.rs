//! One UDP socket for both QUIC and the protocol's plain pre-session frames.
//!
//! QUIC v1 packets always have the fixed bit (0x40) set in their first byte;
//! BitStream frames start with the protocol version byte (0x01), whose fixed
//! bit is clear. [`DemuxSocket`] sits under quinn's endpoint, copies every
//! non-QUIC datagram to a channel for the driver, and lets quinn drop the
//! original as an undecodable packet.

use std::io::{self, IoSliceMut};
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use bytes::Bytes;
use quinn::udp::{RecvMeta, Transmit};
use quinn::{AsyncUdpSocket, UdpPoller};
use tokio::sync::mpsc;

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
    plain_tx: mpsc::UnboundedSender<PlainDatagram>,
}

impl DemuxSocket {
    /// Wrap a bound std socket for the tokio runtime.
    pub fn bind(
        addr: SocketAddr,
    ) -> io::Result<(Arc<Self>, mpsc::UnboundedReceiver<PlainDatagram>)> {
        let std_sock = std::net::UdpSocket::bind(addr)?;
        std_sock.set_nonblocking(true)?;
        let inner = quinn::Runtime::wrap_udp_socket(&quinn::TokioRuntime, std_sock)?;
        let (tx, rx) = mpsc::unbounded_channel();
        Ok((
            Arc::new(Self {
                inner,
                plain_tx: tx,
            }),
            rx,
        ))
    }

    /// Send a plain (non-QUIC) datagram. Drops on `WouldBlock`, like UDP.
    pub fn send_plain(&self, to: SocketAddr, data: &[u8]) -> io::Result<()> {
        let t = Transmit {
            destination: to,
            ecn: None,
            contents: data,
            segment_size: None,
            src_ip: None,
        };
        match self.inner.try_send(&t) {
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => Ok(()),
            other => other,
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
                        let _ = self.plain_tx.send(PlainDatagram {
                            from: m.addr,
                            data: Bytes::copy_from_slice(d),
                        });
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
