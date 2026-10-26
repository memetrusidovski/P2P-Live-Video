//! S/Kademlia routing table and disjoint-path lookups (Ch2 §2.1).
//!
//! Pure data structures: the node integration (RPC handlers, timers, guardian
//! storage) lives in `node/dht.rs`. Everything here is deterministic and unit
//! tested on its own.

use std::collections::{BTreeMap, BTreeSet};

use bs_wire::{NodeId, PeerRecord};

use crate::io::PeerAddr;
use crate::time::Instant;

/// A 256-bit key in the S/Kademlia space (a NodeID or a `K_s`).
pub type Key = [u8; 32];

/// XOR distance `d(a, b)` as a big-endian 256-bit integer.
pub fn distance(a: &Key, b: &Key) -> Key {
    bs_crypto::xor_distance(a, b)
}

/// Index of the k-bucket holding `other` relative to `local`: the length of the
/// common prefix (0..=255). `None` when the ids are equal.
pub fn bucket_index(local: &NodeId, other: &NodeId) -> Option<usize> {
    let d = distance(&local.0, &other.0);
    let lz = bs_crypto::leading_zero_bits(&d);
    if lz >= 256 {
        None
    } else {
        Some(lz as usize)
    }
}

/// `K_s = Blake3(StreamID)`: the key a stream's guardians cluster around (Ch2 §2.3.2).
pub fn stream_key(stream_id: &bs_wire::StreamId) -> Key {
    bs_crypto::blake3_hash(stream_id.as_bytes()).0
}

/// Registration sampling rule (Ch2 §2.3.2): a peer registers iff
/// `Blake3(NodeID)[0:2] mod 2^s == 0`.
pub fn in_register_sample(node_id: &NodeId, sample_log2: u8) -> bool {
    if sample_log2 == 0 {
        return true;
    }
    let h = bs_crypto::blake3_hash(node_id.as_bytes());
    let v = u16::from_be_bytes([h.0[0], h.0[1]]) as u32;
    v % (1u32 << sample_log2.min(16)) == 0
}

/// A routing-table entry.
#[derive(Debug, Clone)]
pub struct Contact {
    /// The peer.
    pub record: PeerRecord,
    /// Last time it answered us.
    pub last_seen: Instant,
}

/// 256 k-buckets of reachable relay-class contacts (Ch2 §2.1.1, §2.3.2 rule 1).
#[derive(Debug, Clone)]
pub struct RoutingTable {
    local: NodeId,
    k: usize,
    buckets: Vec<Vec<Contact>>,
}

impl RoutingTable {
    /// Empty table for `local` with bucket capacity `k`.
    pub fn new(local: NodeId, k: usize) -> Self {
        Self {
            local,
            k,
            buckets: vec![Vec::new(); 256],
        }
    }

    /// Our id.
    pub fn local(&self) -> NodeId {
        self.local
    }

    /// Contacts held.
    pub fn len(&self) -> usize {
        self.buckets.iter().map(|b| b.len()).sum()
    }

    /// Whether empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Whether `id` is a contact.
    pub fn contains(&self, id: &NodeId) -> bool {
        self.get(id).is_some()
    }

    /// The contact for `id`.
    pub fn get(&self, id: &NodeId) -> Option<&Contact> {
        let j = bucket_index(&self.local, id)?;
        self.buckets[j].iter().find(|c| c.record.node_id == *id)
    }

    /// Whether the bucket for `id` can take one more contact.
    pub fn has_room(&self, id: &NodeId) -> bool {
        match bucket_index(&self.local, id) {
            Some(j) => self.buckets[j].len() < self.k || self.contains(id),
            None => false,
        }
    }

    /// Insert or refresh a contact. Returns `false` when the bucket is full (the
    /// classic Kademlia rule keeps the long-lived contacts and drops the newcomer).
    pub fn insert(&mut self, record: PeerRecord, now: Instant) -> bool {
        let Some(j) = bucket_index(&self.local, &record.node_id) else {
            return false;
        };
        let b = &mut self.buckets[j];
        if let Some(pos) = b.iter().position(|c| c.record.node_id == record.node_id) {
            let mut c = b.remove(pos);
            c.record = record;
            c.last_seen = now;
            b.push(c); // most recently seen at the tail
            return true;
        }
        if b.len() >= self.k {
            return false;
        }
        b.push(Contact {
            record,
            last_seen: now,
        });
        true
    }

    /// Note that `id` answered us now.
    pub fn touch(&mut self, id: &NodeId, now: Instant) {
        if let Some(j) = bucket_index(&self.local, id) {
            let b = &mut self.buckets[j];
            if let Some(pos) = b.iter().position(|c| c.record.node_id == *id) {
                let mut c = b.remove(pos);
                c.last_seen = now;
                b.push(c);
            }
        }
    }

    /// Drop a contact (it stopped answering).
    pub fn remove(&mut self, id: &NodeId) {
        if let Some(j) = bucket_index(&self.local, id) {
            self.buckets[j].retain(|c| c.record.node_id != *id);
        }
    }

    /// The `n` contacts closest to `target` by XOR distance, excluding `exclude`.
    pub fn closest(&self, target: &Key, n: usize, exclude: &BTreeSet<NodeId>) -> Vec<PeerRecord> {
        let mut v: Vec<(Key, PeerRecord)> = self
            .buckets
            .iter()
            .flatten()
            .filter(|c| !exclude.contains(&c.record.node_id))
            .map(|c| (distance(&c.record.node_id.0, target), c.record))
            .collect();
        v.sort_by_key(|(d, _)| *d);
        v.truncate(n);
        v.into_iter().map(|(_, r)| r).collect()
    }

    /// Every contact.
    pub fn contacts(&self) -> impl Iterator<Item = &Contact> {
        self.buckets.iter().flatten()
    }

    /// Non-empty bucket sizes, by index.
    pub fn bucket_sizes(&self) -> Vec<(usize, usize)> {
        self.buckets
            .iter()
            .enumerate()
            .filter(|(_, b)| !b.is_empty())
            .map(|(i, b)| (i, b.len()))
            .collect()
    }
}

/// What a lookup asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LookupKind {
    /// FIND_NODE: the k closest contacts.
    Node,
    /// FIND_VALUE: the value under the key, or the k closest contacts.
    Value,
}

/// Why the node runs a lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    /// Bootstrap self-lookup (Ch2 §2.1.3 step 3).
    SelfLookup,
    /// FIND_VALUE toward `K_s` that stops at the first guardians holding the Stream
    /// Record; they get the `GET_PEERS` (Ch2 §2.1.3 step 5).
    Discovery,
    /// Full FIND_NODE toward `K_s`: the k closest contacts, for `REGISTER_PEER`.
    Guardians,
    /// FIND_NODE toward one NodeID, to learn its observed address.
    Resolve,
}

impl Purpose {
    /// Label for events.
    pub fn label(self) -> &'static str {
        match self {
            Purpose::SelfLookup => "self",
            Purpose::Discovery => "discovery",
            Purpose::Guardians => "guardians",
            Purpose::Resolve => "resolve",
        }
    }
}

#[derive(Debug, Clone)]
struct Path {
    /// Unqueried candidates by distance.
    candidates: BTreeMap<Key, PeerRecord>,
    /// Queries in flight on this path (at most `width`).
    outstanding: Vec<(PeerRecord, Instant)>,
    /// Rounds taken.
    hops: u8,
}

/// One iterative S/Kademlia lookup with `α` disjoint paths (Ch2 §2.1.2): a contact
/// learned on one path is never queried on another.
#[derive(Debug, Clone)]
pub struct Lookup {
    /// Identifier for the node's bookkeeping.
    pub id: u32,
    /// Target key.
    pub target: Key,
    /// Kind.
    pub kind: LookupKind,
    /// Purpose.
    pub purpose: Purpose,
    /// Start time.
    pub started: Instant,
    /// The latest value returned by a FIND_VALUE hit.
    pub value: Option<bs_wire::StreamRecord>,
    /// Responders that returned the value (the guardians reached).
    pub holders: Vec<PeerRecord>,
    /// Stop once this many holders answered (0 = run to exhaustion).
    pub stop_at_holders: usize,
    /// Queries sent.
    pub queried: u16,
    local: NodeId,
    k: usize,
    width: usize,
    paths: Vec<Path>,
    claimed: BTreeSet<NodeId>,
    responded: BTreeMap<Key, PeerRecord>,
}

impl Lookup {
    /// Start a lookup from `seeds` (the closest contacts known), spread over `alpha` paths.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: u32,
        local: NodeId,
        target: Key,
        kind: LookupKind,
        purpose: Purpose,
        seeds: Vec<PeerRecord>,
        alpha: usize,
        k: usize,
        now: Instant,
    ) -> Self {
        let alpha = alpha.max(1);
        let mut paths = vec![
            Path {
                candidates: BTreeMap::new(),
                outstanding: Vec::new(),
                hops: 0,
            };
            alpha
        ];
        let mut claimed = BTreeSet::new();
        for (i, s) in seeds.into_iter().enumerate() {
            if s.node_id == local || !claimed.insert(s.node_id) {
                continue;
            }
            paths[i % alpha]
                .candidates
                .insert(distance(&s.node_id.0, &target), s);
        }
        Self {
            id,
            target,
            kind,
            purpose,
            started: now,
            value: None,
            holders: Vec::new(),
            stop_at_holders: 0,
            queried: 0,
            local,
            k,
            width: alpha,
            paths,
            claimed,
            responded: BTreeMap::new(),
        }
    }

    fn kth_distance(&self) -> Option<Key> {
        if self.responded.len() >= self.k {
            self.responded.keys().nth(self.k - 1).copied()
        } else {
            None
        }
    }

    /// Advance: expire timed-out queries and return the next queries to send, up
    /// to `width` in flight per path (the S/Kademlia α per disjoint path).
    pub fn poll(&mut self, now: Instant, timeout: crate::time::Duration) -> Vec<PeerRecord> {
        if self.stopped() {
            return Vec::new();
        }
        let kth = self.kth_distance();
        let width = self.width.max(1);
        let mut out = Vec::new();
        for p in &mut self.paths {
            // No answer within the timeout: the contact stays claimed, never re-asked.
            p.outstanding
                .retain(|(_, sent)| now.duration_since(*sent) < timeout);
            // Candidates no closer than our k-th closest responder cannot improve the
            // result: drop them and let the path finish.
            if let Some(kth) = kth {
                p.candidates.retain(|d, _| *d < kth);
            }
            let mut advanced = false;
            while p.outstanding.len() < width {
                let Some((d, rec)) = p.candidates.iter().next().map(|(d, r)| (*d, *r)) else {
                    break;
                };
                p.candidates.remove(&d);
                p.outstanding.push((rec, now));
                self.queried += 1;
                out.push(rec);
                advanced = true;
            }
            if advanced {
                p.hops += 1;
            }
        }
        out
    }

    fn stopped(&self) -> bool {
        self.stop_at_holders > 0 && self.holders.len() >= self.stop_at_holders
    }

    /// A FIND response from `from`. Returns the contacts newly learned (for the
    /// table's ping-before-insert), or `None` when the response matches no query.
    pub fn on_response(
        &mut self,
        from: PeerAddr,
        responder_id: NodeId,
        nodes: Vec<PeerRecord>,
        value: Option<bs_wire::StreamRecord>,
        now: Instant,
    ) -> Option<Vec<PeerRecord>> {
        let _ = now;
        let idx = self
            .paths
            .iter()
            .position(|p| p.outstanding.iter().any(|(r, _)| r.addr.0 == from))?;
        let pos = self.paths[idx]
            .outstanding
            .iter()
            .position(|(r, _)| r.addr.0 == from)
            .unwrap();
        let (mut responder, _) = self.paths[idx].outstanding.remove(pos);
        if responder.node_id != responder_id {
            // A bootstrap placeholder (address only) learns its real id from the answer.
            self.claimed.remove(&responder.node_id);
            self.claimed.insert(responder_id);
            responder.node_id = responder_id;
        }
        self.responded
            .insert(distance(&responder.node_id.0, &self.target), responder);
        if let Some(v) = value {
            self.value = Some(v);
            if !self.holders.iter().any(|h| h.node_id == responder.node_id) {
                self.holders.push(responder);
            }
            return Some(Vec::new());
        }
        let mut learned = Vec::new();
        for n in nodes {
            if n.node_id == self.local || !self.claimed.insert(n.node_id) {
                continue;
            }
            // Paths stay disjoint (each contact is claimed once), but an idle path
            // takes new candidates first so a single seed still fans out to α paths.
            let target = self
                .paths
                .iter()
                .enumerate()
                .min_by_key(|(i, p)| (p.candidates.len() + p.outstanding.len(), *i != idx))
                .map(|(i, _)| i)
                .unwrap_or(idx);
            self.paths[target]
                .candidates
                .insert(distance(&n.node_id.0, &self.target), n);
            learned.push(n);
        }
        Some(learned)
    }

    /// The record we queried at `from`, if a query is outstanding there.
    pub fn outstanding_record(&self, from: PeerAddr) -> Option<PeerRecord> {
        self.paths
            .iter()
            .flat_map(|p| p.outstanding.iter())
            .find(|(r, _)| r.addr.0 == from)
            .map(|(r, _)| *r)
    }

    /// Whether a query to `from` is outstanding.
    pub fn awaits(&self, from: PeerAddr) -> bool {
        self.paths
            .iter()
            .any(|p| p.outstanding.iter().any(|(r, _)| r.addr.0 == from))
    }

    /// Finished: enough holders answered, or no path has work left.
    pub fn is_done(&self) -> bool {
        if self.stopped() {
            return true;
        }
        let kth = self.kth_distance();
        self.paths.iter().all(|p| {
            p.outstanding.is_empty()
                && p.candidates
                    .iter()
                    .all(|(d, _)| kth.map(|k| *d >= k).unwrap_or(false))
        })
    }

    /// The k closest contacts that answered.
    pub fn result(&self) -> Vec<PeerRecord> {
        self.responded.values().take(self.k).copied().collect()
    }

    /// Longest path in hops.
    pub fn hops(&self) -> u8 {
        self.paths.iter().map(|p| p.hops).max().unwrap_or(0)
    }

    /// Contacts that answered on any path.
    pub fn responders(&self) -> usize {
        self.responded.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bs_wire::{NodeClass, PeerFlags, TreeSet, WireAddr};
    use std::net::{Ipv4Addr, SocketAddr};

    fn rec(b: u8) -> PeerRecord {
        PeerRecord {
            node_id: NodeId([b; 32]),
            node_class: NodeClass::Relay,
            assigned_trees: TreeSet(1),
            flags: PeerFlags::PUBLIC,
            addr: WireAddr(SocketAddr::new(Ipv4Addr::new(10, 0, 0, b).into(), 4000)),
        }
    }

    #[test]
    fn buckets_by_common_prefix() {
        let local = NodeId([0x80; 32]);
        assert_eq!(bucket_index(&local, &NodeId([0x00; 32])), Some(0));
        assert_eq!(bucket_index(&local, &NodeId([0xC0; 32])), Some(1));
        assert_eq!(bucket_index(&local, &local), None);
    }

    #[test]
    fn table_insert_closest_and_capacity() {
        let mut t = RoutingTable::new(NodeId([0; 32]), 2);
        let now = Instant::ZERO;
        assert!(t.insert(rec(0x10), now));
        assert!(t.insert(rec(0x11), now)); // same bucket as 0x10
        assert!(!t.insert(rec(0x12), now)); // bucket full
        assert!(t.insert(rec(0x10), now)); // refresh is always fine
        assert!(t.insert(rec(0x80), now));
        let c = t.closest(&[0x11; 32], 2, &BTreeSet::new());
        assert_eq!(c[0].node_id, NodeId([0x11; 32]));
        assert_eq!(c[1].node_id, NodeId([0x10; 32]));
        assert_eq!(t.len(), 3);
        t.remove(&NodeId([0x80; 32]));
        assert_eq!(t.len(), 2);
    }

    #[test]
    fn lookup_paths_are_disjoint_and_terminate() {
        let local = NodeId([0; 32]);
        let target = [0xFF; 32];
        let seeds = vec![rec(0x01), rec(0x02), rec(0x03)];
        let mut l = Lookup::new(
            1,
            local,
            target,
            LookupKind::Node,
            Purpose::Guardians,
            seeds,
            3,
            2,
            Instant::ZERO,
        );
        assert!(!l.is_done());
        let q = l.poll(Instant::ZERO, crate::time::Duration::from_millis(100));
        assert_eq!(q.len(), 3);
        // 0x01 answers with 0xF0 and 0xF1; 0x02 names 0xF0 too (already claimed) and 0xE0.
        let learned = l
            .on_response(
                rec(0x01).addr.0,
                NodeId([0x01; 32]),
                vec![rec(0xF0), rec(0xF1)],
                None,
                Instant::ZERO,
            )
            .unwrap();
        assert_eq!(learned.len(), 2);
        let learned = l
            .on_response(
                rec(0x02).addr.0,
                NodeId([0x02; 32]),
                vec![rec(0xF0), rec(0xE0)],
                None,
                Instant::ZERO,
            )
            .unwrap();
        assert_eq!(learned.len(), 1);
        // 0x03 times out; the three learned contacts are spread over the idle paths.
        let later = Instant::ZERO + crate::time::Duration::from_millis(200);
        let q = l.poll(later, crate::time::Duration::from_millis(100));
        assert_eq!(q.len(), 3);
        assert!(q.iter().any(|r| r.node_id == NodeId([0xF1; 32])));
        l.on_response(rec(0xF1).addr.0, NodeId([0xF1; 32]), vec![], None, later)
            .unwrap();
        l.on_response(rec(0xE0).addr.0, NodeId([0xE0; 32]), vec![], None, later)
            .unwrap();
        // 0xF0 answers with an already-claimed contact: nothing new is learned.
        let learned = l
            .on_response(
                rec(0xF0).addr.0,
                NodeId([0xF0; 32]),
                vec![rec(0x02)],
                None,
                later,
            )
            .unwrap();
        assert!(learned.is_empty());
        let q = l.poll(later, crate::time::Duration::from_millis(100));
        assert!(q.is_empty());
        assert!(l.is_done());
        let r = l.result();
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].node_id, NodeId([0xF1; 32]));
        assert_eq!(r[1].node_id, NodeId([0xF0; 32]));
        assert!(l.hops() >= 2);
    }

    #[test]
    fn sample_rule() {
        assert!(in_register_sample(&NodeId([1; 32]), 0));
        let n = (0u8..=255)
            .filter(|b| in_register_sample(&NodeId([*b; 32]), 3))
            .count();
        assert!(n > 10 && n < 60, "about 1/8 of ids: {n}");
    }
}
