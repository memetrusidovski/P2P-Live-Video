//! Admission of plain (pre-session) datagrams: the two-tier token buckets of the
//! specification's kernel filter (Ch7 §7.2, App B §B.1.4), run in user space.
//!
//! SECURITY: Every plain frame costs the node a signature check, and a PING costs a
//!           signed answer. Without a budget per source, a flood of valid frames
//!           from spoofed addresses stops the node's timers and its playback.

use std::collections::{HashMap, HashSet};
use std::net::IpAddr;
use std::time::Instant;

/// Packets per second from a source with an open session (`RATE_PPS_PEER`).
pub const RATE_PPS_PEER: f64 = 2000.0;
/// Bucket capacity of a source with an open session (`BURST_PEER`).
pub const BURST_PEER: f64 = 1000.0;
/// Packets per second from a source with no session (`RATE_PPS_UNKNOWN`).
pub const RATE_PPS_UNKNOWN: f64 = 50.0;
/// Bucket capacity of a source with no session (`BURST_UNKNOWN`).
pub const BURST_UNKNOWN: f64 = 100.0;
/// Packets per second from all sources with no session together (`GLOBAL_NEW_PPS`).
pub const GLOBAL_NEW_PPS: f64 = 5000.0;
/// Sources with no session that have a bucket (`MAX_UNKNOWN`).
pub const MAX_UNKNOWN: usize = 16_384;

/// A token bucket: `rate` tokens per second up to `burst`.
#[derive(Debug, Clone, Copy)]
struct Bucket {
    tokens: f64,
    at: Instant,
}

impl Bucket {
    fn full(burst: f64, now: Instant) -> Self {
        Self {
            tokens: burst,
            at: now,
        }
    }

    /// Take one token if the bucket holds one.
    fn take(&mut self, rate: f64, burst: f64, now: Instant) -> bool {
        let dt = now.saturating_duration_since(self.at).as_secs_f64();
        self.tokens = (self.tokens + dt * rate).min(burst);
        self.at = now;
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

/// Decides which plain datagrams reach the driver.
#[derive(Debug)]
pub struct PlainLimiter {
    /// Source addresses with an open session.
    peers: HashSet<IpAddr>,
    peer_buckets: HashMap<IpAddr, Bucket>,
    unknown: HashMap<IpAddr, Bucket>,
    global: Bucket,
}

impl PlainLimiter {
    /// A limiter with no peers and full buckets.
    pub fn new(now: Instant) -> Self {
        Self {
            peers: HashSet::new(),
            peer_buckets: HashMap::new(),
            unknown: HashMap::new(),
            global: Bucket::full(GLOBAL_NEW_PPS, now),
        }
    }

    /// Replace the set of source addresses that have an open session.
    pub fn set_peers(&mut self, peers: HashSet<IpAddr>) {
        self.peer_buckets.retain(|ip, _| peers.contains(ip));
        self.peers = peers;
    }

    /// Whether a plain datagram from `from` is handed to the driver.
    ///
    /// A source with an open session draws on its own peer bucket. Any other
    /// source draws on its own first-contact bucket and on the bucket that all
    /// such sources share.
    pub fn admit(&mut self, from: IpAddr, now: Instant) -> bool {
        // WHY: A packet with a loopback source cannot arrive from the network. A
        //      test swarm on one host sends every frame from 127.0.0.1.
        if from.is_loopback() {
            return true;
        }
        if self.peers.contains(&from) {
            return self
                .peer_buckets
                .entry(from)
                .or_insert_with(|| Bucket::full(BURST_PEER, now))
                .take(RATE_PPS_PEER, BURST_PEER, now);
        }
        if !self.unknown.contains_key(&from) && self.unknown.len() >= MAX_UNKNOWN {
            // RULE: Evict one entry, never refuse the newcomer. A flood of spoofed
            //       sources must not lock every new viewer out of the table.
            if let Some(victim) = self.unknown.keys().next().copied() {
                self.unknown.remove(&victim);
            }
        }
        let own = self
            .unknown
            .entry(from)
            .or_insert_with(|| Bucket::full(BURST_UNKNOWN, now))
            .take(RATE_PPS_UNKNOWN, BURST_UNKNOWN, now);
        // RULE: Test the source's bucket first. A source over its own budget must
        //       not drain the bucket that all first contacts share.
        own && self.global.take(GLOBAL_NEW_PPS, GLOBAL_NEW_PPS, now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn ip(a: u8, b: u8) -> IpAddr {
        IpAddr::from([203, 0, a, b])
    }

    #[test]
    fn an_unknown_source_gets_its_burst_then_its_rate() {
        let t0 = Instant::now();
        let mut l = PlainLimiter::new(t0);
        let admitted = (0..500).filter(|_| l.admit(ip(113, 1), t0)).count();
        assert_eq!(admitted, BURST_UNKNOWN as usize);
        // One second later the bucket holds one second of the rate.
        let t1 = t0 + Duration::from_secs(1);
        let admitted = (0..500).filter(|_| l.admit(ip(113, 1), t1)).count();
        assert_eq!(admitted, RATE_PPS_UNKNOWN as usize);
        // Another source is not affected.
        assert!(l.admit(ip(113, 2), t1));
    }

    #[test]
    fn a_peer_gets_the_peer_budget() {
        let t0 = Instant::now();
        let mut l = PlainLimiter::new(t0);
        l.set_peers(HashSet::from([ip(113, 1)]));
        let admitted = (0..5000).filter(|_| l.admit(ip(113, 1), t0)).count();
        assert_eq!(admitted, BURST_PEER as usize);
        // The session closed: the source is a first contact again.
        l.set_peers(HashSet::new());
        let admitted = (0..5000).filter(|_| l.admit(ip(113, 1), t0)).count();
        assert_eq!(admitted, BURST_UNKNOWN as usize);
    }

    #[test]
    fn a_spoofed_flood_is_held_to_the_global_budget() {
        let t0 = Instant::now();
        let mut l = PlainLimiter::new(t0);
        // 40,000 packets in one second, each from another source address.
        let mut admitted = 0;
        for i in 0..40_000u32 {
            let now = t0 + Duration::from_micros(i as u64 * 25);
            let from = IpAddr::from([198, 18, (i >> 8) as u8, i as u8]);
            if l.admit(from, now) {
                admitted += 1;
            }
        }
        // The full bucket plus one second of refill.
        assert!(admitted <= 2 * GLOBAL_NEW_PPS as usize, "{admitted}");
        assert!(l.unknown.len() <= MAX_UNKNOWN);
        // A peer still gets through during the flood.
        l.set_peers(HashSet::from([ip(113, 9)]));
        assert!(l.admit(ip(113, 9), t0 + Duration::from_secs(1)));
    }

    #[test]
    fn loopback_is_never_limited() {
        let t0 = Instant::now();
        let mut l = PlainLimiter::new(t0);
        let lo: IpAddr = "127.0.0.1".parse().unwrap();
        assert!((0..10_000).all(|_| l.admit(lo, t0)));
    }
}
