//! Latency, loss, egress queueing and session lifecycle model.

use std::collections::HashMap;
use std::net::SocketAddr;

use bs_core::{Channel, Duration, Instant};
use rand::Rng;
use rand_chacha::ChaCha8Rng;

use crate::scenario::NetworkSpec;

/// Per-node network attributes.
#[derive(Debug, Clone)]
pub struct Endpoint {
    /// Region.
    pub region: u8,
    /// Upload capacity, kbps.
    pub upload_kbps: u32,
    /// Egress busy until.
    pub egress_free_at: Instant,
    /// Whether the host is up.
    pub alive: bool,
    /// Bytes sent, by media/control.
    pub tx_media: u64,
    /// Control bytes sent.
    pub tx_control: u64,
    /// Datagrams dropped by queue overflow.
    pub queue_drops: u64,
    /// Datagrams dropped by random loss.
    pub loss_drops: u64,
}

/// The network.
pub struct Network {
    spec: NetworkSpec,
    endpoints: HashMap<SocketAddr, Endpoint>,
    /// Last scheduled arrival per (from, to, channel) so reliable channels stay ordered.
    stream_tail: HashMap<(SocketAddr, SocketAddr, u8), Instant>,
    /// Open sessions (unordered pair).
    sessions: HashMap<(SocketAddr, SocketAddr), ()>,
    pair_seed: u64,
    /// Current datagram loss (scriptable).
    pub loss: f64,
}

/// Outcome of scheduling a send.
#[derive(Debug, Clone, Copy)]
pub enum Delivery {
    /// Arrives at.
    At(Instant),
    /// Dropped.
    Dropped,
}

impl Network {
    /// New network.
    pub fn new(spec: NetworkSpec, seed: u64) -> Self {
        Self {
            loss: spec.loss,
            spec,
            endpoints: HashMap::new(),
            stream_tail: HashMap::new(),
            sessions: HashMap::new(),
            pair_seed: seed,
        }
    }

    /// Add a host.
    pub fn add(&mut self, addr: SocketAddr, region: u8, upload_kbps: u32) {
        self.endpoints.insert(
            addr,
            Endpoint {
                region,
                upload_kbps,
                egress_free_at: Instant::ZERO,
                alive: true,
                tx_media: 0,
                tx_control: 0,
                queue_drops: 0,
                loss_drops: 0,
            },
        );
    }
    /// Whether alive.
    pub fn alive(&self, a: &SocketAddr) -> bool {
        self.endpoints.get(a).map(|e| e.alive).unwrap_or(false)
    }
    /// Kill a host; returns the peers it had sessions with.
    pub fn kill(&mut self, a: SocketAddr) -> Vec<SocketAddr> {
        if let Some(e) = self.endpoints.get_mut(&a) {
            e.alive = false;
        }
        let peers: Vec<SocketAddr> = self
            .sessions
            .keys()
            .filter_map(|(x, y)| {
                if *x == a {
                    Some(*y)
                } else if *y == a {
                    Some(*x)
                } else {
                    None
                }
            })
            .collect();
        self.sessions.retain(|(x, y), _| *x != a && *y != a);
        peers
    }

    fn key(a: SocketAddr, b: SocketAddr) -> (SocketAddr, SocketAddr) {
        if a < b {
            (a, b)
        } else {
            (b, a)
        }
    }
    /// Record an open session.
    pub fn open_session(&mut self, a: SocketAddr, b: SocketAddr) {
        self.sessions.insert(Self::key(a, b), ());
    }
    /// Close a session; returns whether it existed.
    pub fn close_session(&mut self, a: SocketAddr, b: SocketAddr) -> bool {
        self.sessions.remove(&Self::key(a, b)).is_some()
    }
    /// Whether a session exists.
    pub fn has_session(&self, a: SocketAddr, b: SocketAddr) -> bool {
        self.sessions.contains_key(&Self::key(a, b))
    }
    /// Number of open sessions.
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    /// Deterministic base one-way latency for a pair.
    pub fn one_way_latency(&self, a: &SocketAddr, b: &SocketAddr) -> Duration {
        let (ea, eb) = match (self.endpoints.get(a), self.endpoints.get(b)) {
            (Some(x), Some(y)) => (x, y),
            _ => return Duration::from_millis(50),
        };
        let r = if ea.region == eb.region {
            self.spec.intra_region_rtt_ms
        } else {
            self.spec.inter_region_rtt_ms
        };
        // Hash the unordered pair with the seed for a stable per-pair draw in [0,1).
        let (x, y) = Self::key(*a, *b);
        let mut h = blake3::Hasher::new();
        h.update(&self.pair_seed.to_le_bytes());
        h.update(x.to_string().as_bytes());
        h.update(y.to_string().as_bytes());
        let d = h.finalize();
        let u = u32::from_le_bytes([
            d.as_bytes()[0],
            d.as_bytes()[1],
            d.as_bytes()[2],
            d.as_bytes()[3],
        ]) as f64
            / u32::MAX as f64;
        let rtt_ms = r.min + (r.max - r.min) * u;
        Duration::from_secs_f64(rtt_ms / 2000.0)
    }
    /// RTT for a pair.
    pub fn rtt(&self, a: &SocketAddr, b: &SocketAddr) -> Duration {
        self.one_way_latency(a, b) * 2
    }

    /// Schedule a frame of `bytes` from `from` to `to` on `channel` at `now`.
    pub fn send(
        &mut self,
        rng: &mut ChaCha8Rng,
        now: Instant,
        from: SocketAddr,
        to: SocketAddr,
        channel: Channel,
        bytes: usize,
        is_media: bool,
    ) -> Delivery {
        let latency = self.one_way_latency(&from, &to);
        let jitter =
            Duration::from_secs_f64(rng.random_range(0.0..=self.spec.jitter_ms.max(0.0)) / 1000.0);
        let wire_bytes = bytes as u64 + self.spec.packet_overhead as u64;
        let Some(e) = self.endpoints.get_mut(&from) else {
            return Delivery::Dropped;
        };
        if !e.alive {
            return Delivery::Dropped;
        }
        // Egress serialisation.
        let tx = Duration::from_secs_f64(
            wire_bytes as f64 * 8.0 / (e.upload_kbps.max(1) as f64 * 1000.0),
        );
        let start = if e.egress_free_at > now {
            e.egress_free_at
        } else {
            now
        };
        let backlog = start.duration_since(now);
        let unreliable = matches!(channel, Channel::Datagram | Channel::Udp);
        if unreliable && backlog.as_millis_f64() > self.spec.max_queue_ms {
            e.queue_drops += 1;
            return Delivery::Dropped;
        }
        e.egress_free_at = start + tx;
        if is_media {
            e.tx_media += wire_bytes;
        } else {
            e.tx_control += wire_bytes;
        }
        if unreliable && self.loss > 0.0 && rng.random::<f64>() < self.loss {
            e.loss_drops += 1;
            return Delivery::Dropped;
        }
        let mut arrival = e.egress_free_at + latency + jitter;
        if !unreliable {
            let k = (from, to, channel_code(channel));
            let tail = self.stream_tail.entry(k).or_insert(Instant::ZERO);
            if *tail > arrival {
                arrival = *tail;
            }
            *tail = arrival;
        }
        if !self.alive(&to) {
            return Delivery::Dropped;
        }
        Delivery::At(arrival)
    }

    /// Session open latency.
    pub fn session_open_delay(&self, a: &SocketAddr, b: &SocketAddr) -> Duration {
        self.rtt(a, b).mul_f64(self.spec.session_open_rtts)
    }
    /// Idle timeout.
    pub fn idle_timeout(&self) -> Duration {
        Duration::from_secs_f64(self.spec.idle_timeout_s)
    }
    /// Discovery RTT.
    pub fn discovery_rtt(&self) -> Duration {
        Duration::from_secs_f64(self.spec.discovery_rtt_ms / 1000.0)
    }
    /// Regions.
    pub fn regions(&self) -> u8 {
        self.spec.regions.max(1)
    }
    /// Sum of bytes by kind over alive and dead hosts.
    pub fn totals(&self) -> (u64, u64, u64, u64) {
        let mut m = 0;
        let mut c = 0;
        let mut q = 0;
        let mut l = 0;
        for e in self.endpoints.values() {
            m += e.tx_media;
            c += e.tx_control;
            q += e.queue_drops;
            l += e.loss_drops;
        }
        (m, c, q, l)
    }
}

fn channel_code(c: Channel) -> u8 {
    match c {
        Channel::Udp => 0,
        Channel::Control => 1,
        Channel::Tree(t) => 16 + t.0,
        Channel::Datagram => 2,
    }
}
