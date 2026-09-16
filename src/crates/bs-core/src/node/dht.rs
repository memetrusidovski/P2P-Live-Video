//! DHT integration (Ch2): bootstrap, S/Kademlia lookups, guardian storage for our
//! stream, registration and the publisher's `STORE_RECORD` loop. Pure structures
//! live in [`crate::dht`]; this file is the RPC and timer glue on [`Node`].

use std::collections::{BTreeMap, BTreeSet};

use bs_crypto::Signer;
use bs_wire::frames::{
    FindFrame, FindRequest, FindResponse, FindResult, GetPeers, GetPeersFrame, GetPeersResponse,
    Pong, RegisterPeer, StoreRecord, StoreRecordAck, ACK_TREES,
};
use bs_wire::{
    Frame, FrameType, NodeClass, NodeId, PeerFlags, PeerRecord, Reachability, StreamRecord,
    TreeSet, WireAddr,
};
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};

use super::{Lifecycle, Node};
use crate::dht::{in_register_sample, stream_key, Key, Lookup, LookupKind, Purpose, RoutingTable};
use crate::io::{Channel, Event, Output, PeerAddr};
use crate::time::{Duration, Instant};

/// How a node finds peers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DiscoveryMode {
    /// The real S/Kademlia DHT over plain-UDP frames (Ch2).
    #[default]
    Dht,
    /// The simulator's oracle answers `Output::Discover` (M1 stand-in).
    Oracle,
}

/// A registration held by this node as a guardian.
#[derive(Debug, Clone)]
struct Reg {
    record: PeerRecord,
    last_seen: Instant,
}

/// Guardian-side storage for one stream (Ch2 §2.3.2).
#[derive(Debug, Default)]
pub struct GuardianStore {
    regs: BTreeMap<NodeId, Reg>,
    /// Latest accepted Stream Record.
    pub record: Option<StreamRecord>,
    /// Distinct GET_PEERS senders in the last 10 s and their starved bits.
    queriers: BTreeMap<NodeId, (Instant, TreeSet)>,
    /// Queries served.
    pub queries: u64,
    /// Registrations accepted.
    pub registrations: u64,
}

impl GuardianStore {
    fn register(&mut self, record: PeerRecord, now: Instant) {
        self.regs.insert(
            record.node_id,
            Reg {
                record,
                last_seen: now,
            },
        );
    }

    fn expire(&mut self, now: Instant, ttl: Duration) {
        self.regs
            .retain(|_, r| now.duration_since(r.last_seen) < ttl);
        self.queriers
            .retain(|_, (t, _)| now.duration_since(*t) < Duration::from_secs(10));
    }

    fn active(&self, now: Instant, window: Duration) -> impl Iterator<Item = &PeerRecord> {
        self.regs
            .values()
            .filter(move |r| now.duration_since(r.last_seen) < window)
            .map(|r| &r.record)
    }

    fn counts(&self, now: Instant, window: Duration) -> (u32, u32, [u16; ACK_TREES]) {
        let mut active = 0u32;
        let mut relay = 0u32;
        let mut per_tree = [0u16; ACK_TREES];
        for r in self.active(now, window) {
            active += 1;
            if r.node_class == NodeClass::Relay {
                relay += 1;
                for (m, c) in per_tree.iter_mut().enumerate() {
                    if r.assigned_trees.contains(bs_wire::TreeId(m as u8 + 1)) {
                        *c = c.saturating_add(1);
                    }
                }
            }
        }
        (active, relay, per_tree)
    }

    fn query_counts(&self) -> (u16, [u16; ACK_TREES]) {
        let mut starved = [0u16; ACK_TREES];
        for (_, s) in self.queriers.values() {
            for (m, c) in starved.iter_mut().enumerate() {
                if s.contains(bs_wire::TreeId(m as u8 + 1)) {
                    *c = c.saturating_add(1);
                }
            }
        }
        (self.queriers.len().min(u16::MAX as usize) as u16, starved)
    }
}

/// An in-progress discovery (`GET_PEERS` round).
#[derive(Debug)]
struct DiscoveryRun {
    wanted: TreeSet,
    starved: TreeSet,
    /// Guardian lookup this run waits for.
    lookup: Option<u32>,
    /// GET_PEERS targets asked, and the deadline for their answers.
    asked: usize,
    deadline: Option<Instant>,
    responses: Vec<GetPeersResponse>,
}

/// DHT state on a node.
pub struct DhtState {
    /// Discovery mode.
    pub mode: DiscoveryMode,
    /// Bootstrap contacts (Ch2 §2.1.3 step 1).
    pub bootstrap: Vec<PeerAddr>,
    /// The k-buckets.
    pub table: RoutingTable,
    lookups: Vec<Lookup>,
    next_lookup_id: u32,
    /// Contacts pinged before insertion (Ch2 §2.3.2 rule 1).
    pending_pings: BTreeMap<PeerAddr, (Option<PeerRecord>, Instant)>,
    self_lookup_started: bool,
    next_bootstrap: Option<Instant>,
    /// The guardians of `K_s` from the last full lookup (registration targets).
    pub guardians: Vec<PeerRecord>,
    guardians_at: Option<Instant>,
    /// Whether a full guardian lookup is running.
    guardians_lookup: Option<u32>,
    /// Record holders reached by the last discovery lookup (`GET_PEERS` targets).
    holders: Vec<PeerRecord>,
    discovery: Option<DiscoveryRun>,
    /// Guardian storage for our stream.
    pub store: GuardianStore,
    next_register: Option<Instant>,
    registered: Option<PeerRecord>,
    /// Retry of the initial discovery while no Stream Record has arrived.
    next_discovery_retry: Option<Instant>,
    /// STORE_RECORD_ACKs collected since the last record (publisher).
    acks: Vec<StoreRecordAck>,
    /// Counters.
    pub rpc_sent: u64,
    /// Counters.
    pub rpc_served: u64,
    /// Lookups completed.
    pub lookups_done: u64,
}

impl DhtState {
    /// New state for `local`.
    pub fn new(local: NodeId, mode: DiscoveryMode, bootstrap: Vec<PeerAddr>, k: usize) -> Self {
        Self {
            mode,
            bootstrap,
            table: RoutingTable::new(local, k),
            lookups: Vec::new(),
            next_lookup_id: 1,
            pending_pings: BTreeMap::new(),
            self_lookup_started: false,
            next_bootstrap: None,
            guardians: Vec::new(),
            guardians_at: None,
            guardians_lookup: None,
            holders: Vec::new(),
            discovery: None,
            store: GuardianStore::default(),
            next_register: None,
            registered: None,
            next_discovery_retry: None,
            acks: Vec::new(),
            rpc_sent: 0,
            rpc_served: 0,
            lookups_done: 0,
        }
    }
}

fn usable_contact(r: &PeerRecord) -> bool {
    r.node_class == NodeClass::Relay && r.flags.reachability != Reachability::Symmetric
}

fn median(v: &mut [u32]) -> Option<u32> {
    if v.is_empty() {
        return None;
    }
    v.sort_unstable();
    Some(v[v.len() / 2])
}

impl Node {
    // ------------------------------------------------------------------ accessors

    /// Contacts in the routing table.
    pub fn dht_contacts(&self) -> usize {
        self.dht.table.len()
    }
    /// Registrations this node holds as a guardian.
    pub fn dht_registrations(&self) -> usize {
        self.dht.store.regs.len()
    }
    /// DHT RPCs answered.
    pub fn dht_rpc_served(&self) -> u64 {
        self.dht.rpc_served
    }
    /// Discovery mode.
    pub fn discovery_mode(&self) -> DiscoveryMode {
        self.dht.mode
    }
    /// The routing table's contacts (records), for invariant checks.
    pub fn dht_contact_records(&self) -> Vec<PeerRecord> {
        self.dht.table.contacts().map(|c| c.record).collect()
    }

    fn k_s(&self) -> Key {
        stream_key(&self.stream_id())
    }

    /// Only reachable relay-class nodes answer DHT RPCs (Ch2 §2.3.2 rule 1).
    pub(super) fn serves_dht(&self) -> bool {
        self.dht.mode == DiscoveryMode::Dht && usable_contact(&self.peer_record())
    }

    // ------------------------------------------------------------------ bootstrap

    /// Bootstrap (Ch2 §2.1.3): the self-lookup goes straight to the seeds; their
    /// answers prove reachability and fill the first buckets.
    pub(super) fn dht_start(&mut self) {
        if self.dht.mode != DiscoveryMode::Dht {
            return;
        }
        if !self.dht.self_lookup_started && !self.dht.bootstrap.is_empty() {
            self.dht.self_lookup_started = true;
            let me = self.node_id().0;
            self.start_lookup(me, LookupKind::Node, Purpose::SelfLookup, 0);
            self.dht.next_bootstrap = Some(self.now + Duration::from_secs(1));
        }
    }

    /// Address-only placeholders for the seeds; a FIND answer supplies the real id.
    fn seed_records(&self) -> Vec<PeerRecord> {
        self.dht
            .bootstrap
            .iter()
            .filter(|a| **a != self.cfg.addr)
            .map(|a| {
                let mut h = Vec::with_capacity(40);
                h.extend_from_slice(b"seed:");
                h.extend_from_slice(a.to_string().as_bytes());
                PeerRecord {
                    node_id: NodeId(bs_crypto::blake3_hash(&h).0),
                    node_class: NodeClass::Relay,
                    assigned_trees: TreeSet::EMPTY,
                    flags: PeerFlags::PUBLIC,
                    addr: WireAddr(*a),
                }
            })
            .collect()
    }

    fn dht_ping(&mut self, addr: PeerAddr, rec: Option<PeerRecord>) {
        if self.dht.pending_pings.contains_key(&addr) {
            return;
        }
        self.dht.pending_pings.insert(addr, (rec, self.now));
        self.send_ping(addr);
    }

    /// A candidate contact learned from a record: ping before insertion.
    fn dht_learn(&mut self, rec: PeerRecord) {
        if rec.node_id == self.node_id() || !usable_contact(&rec) {
            return;
        }
        if self.dht.table.contains(&rec.node_id) {
            self.dht.table.touch(&rec.node_id, self.now);
            return;
        }
        if self.dht.table.has_room(&rec.node_id) {
            self.dht_ping(rec.addr.0, Some(rec));
        }
    }

    /// A contact that just answered one of our RPCs is reachable: insert directly.
    fn dht_insert_responsive(&mut self, rec: PeerRecord) {
        if rec.node_id != self.node_id() && usable_contact(&rec) {
            self.dht.table.insert(rec, self.now);
        }
    }

    /// PONG hook: complete a ping-before-insert and kick off bootstrap steps.
    pub(super) fn dht_on_pong(&mut self, from: PeerAddr, p: &Pong) {
        let Some((rec, _)) = self.dht.pending_pings.remove(&from) else {
            return;
        };
        let Some(rec) = rec else { return };
        if rec.node_id != self.node_id() && rec.node_id == p.validation.node_id {
            self.dht.table.insert(rec, self.now);
        }
    }

    // ------------------------------------------------------------------ lookups

    fn start_lookup(
        &mut self,
        target: Key,
        kind: LookupKind,
        purpose: Purpose,
        stop_at_holders: usize,
    ) -> Option<u32> {
        let exclude = BTreeSet::from([self.node_id()]);
        let mut seeds = self.dht.table.closest(&target, self.params.dht_k, &exclude);
        if seeds.is_empty() {
            seeds = self.seed_records();
        }
        if seeds.is_empty() {
            return None;
        }
        let id = self.dht.next_lookup_id;
        self.dht.next_lookup_id += 1;
        let mut l = Lookup::new(
            id,
            self.node_id(),
            target,
            kind,
            purpose,
            seeds,
            self.params.dht_alpha,
            self.params.dht_k,
            self.now,
        );
        l.stop_at_holders = stop_at_holders;
        self.dht.lookups.push(l);
        self.dht_drive_lookups();
        Some(id)
    }

    fn send_find(&mut self, to: PeerAddr, kind: LookupKind, target: Key) {
        let ty = match kind {
            LookupKind::Node => FrameType::FIND_NODE,
            LookupKind::Value => FrameType::FIND_VALUE,
        };
        let vb = self.validation_block(ty, &target);
        let req = FindFrame::Request(FindRequest {
            validation: vb,
            target: NodeId(target),
        });
        let f = match kind {
            LookupKind::Node => Frame::FindNode(req),
            LookupKind::Value => Frame::FindValue(req),
        };
        self.dht.rpc_sent += 1;
        self.send(to, Channel::Udp, f, None);
    }

    fn dht_drive_lookups(&mut self) {
        let now = self.now;
        let timeout = self.params.dht_rpc_timeout;
        let mut sends: Vec<(PeerAddr, LookupKind, Key)> = Vec::new();
        for l in &mut self.dht.lookups {
            for rec in l.poll(now, timeout) {
                sends.push((rec.addr.0, l.kind, l.target));
            }
        }
        for (to, kind, target) in sends {
            self.send_find(to, kind, target);
        }
        let done: Vec<Lookup> = {
            let (done, rest): (Vec<Lookup>, Vec<Lookup>) = std::mem::take(&mut self.dht.lookups)
                .into_iter()
                .partition(|l| l.is_done());
            self.dht.lookups = rest;
            done
        };
        for l in done {
            self.dht_finish_lookup(l);
        }
    }

    fn dht_finish_lookup(&mut self, l: Lookup) {
        self.dht.lookups_done += 1;
        let ms = self
            .now
            .duration_since(l.started)
            .as_millis()
            .min(u32::MAX as u64) as u32;
        self.emit(Event::DhtLookup {
            purpose: l.purpose.label(),
            hops: l.hops(),
            queried: l.queried,
            found: l.responders().min(u16::MAX as usize) as u16,
            ms,
        });
        match l.purpose {
            Purpose::SelfLookup => {}
            Purpose::Discovery => {
                // The guardians reached are the record holders; fall back to the closest
                // responders when nobody holds the record yet (a swarm before its first
                // STORE_RECORD).
                self.dht.holders = if l.holders.is_empty() {
                    l.result()
                } else {
                    l.holders.clone()
                };
                let waiting = self
                    .dht
                    .discovery
                    .as_ref()
                    .map(|d| d.lookup == Some(l.id))
                    .unwrap_or(false);
                if waiting {
                    if let Some(d) = self.dht.discovery.as_mut() {
                        d.lookup = None;
                    }
                    self.dht_send_get_peers();
                }
                // Refresh the full guardian set in the background for registration.
                let stale = self
                    .dht
                    .guardians_at
                    .map(|t| self.now.duration_since(t) >= self.params.dht_guardian_cache)
                    .unwrap_or(true);
                if stale && self.dht.guardians_lookup.is_none() {
                    let ks = self.k_s();
                    self.dht.guardians_lookup =
                        self.start_lookup(ks, LookupKind::Node, Purpose::Guardians, 0);
                }
            }
            Purpose::Guardians => {
                self.dht.guardians = l.result();
                self.dht.guardians_at = Some(self.now);
                self.dht.guardians_lookup = None;
                if self.dht.registered.is_none() && self.matrix.is_some() {
                    self.register_self();
                }
            }
        }
    }

    fn dht_on_find_response(&mut self, from: PeerAddr, resp: FindResponse) {
        let Some(idx) = self.dht.lookups.iter().position(|l| l.awaits(from)) else {
            return;
        };
        let (nodes, value) = match resp.result {
            FindResult::Nodes(v) => (v, None),
            FindResult::Value(s) => (Vec::new(), Some(*s)),
        };
        // The responder answered: it is reachable and belongs in the table.
        let mut responder_rec = self.dht.lookups[idx].outstanding_record(from);
        let learned = self.dht.lookups[idx]
            .on_response(from, resp.responder, nodes.clone(), value, self.now)
            .unwrap_or_default();
        if let Some(r) = responder_rec.as_mut() {
            r.node_id = resp.responder;
            // Prefer the responder's own record if it named itself (class, trees, flags).
            if let Some(own) = nodes.iter().find(|n| n.node_id == resp.responder) {
                *r = *own;
            }
            self.dht_insert_responsive(*r);
        }
        for r in learned {
            self.dht_learn(r);
        }
        self.dht_drive_lookups();
    }

    // ------------------------------------------------------------------ discovery (GET_PEERS)

    /// DHT-mode `request_discovery`: resolve the guardians of `K_s`, then ask α of them.
    pub(super) fn dht_discover(&mut self, wanted: TreeSet, starved: TreeSet) {
        self.dht.discovery = Some(DiscoveryRun {
            wanted,
            starved,
            lookup: None,
            asked: 0,
            deadline: None,
            responses: Vec::new(),
        });
        let fresh = self
            .dht
            .guardians_at
            .map(|t| self.now.duration_since(t) < self.params.dht_guardian_cache)
            .unwrap_or(false);
        if fresh && !self.dht.guardians.is_empty() {
            self.dht_send_get_peers();
        } else {
            self.dht_begin_guardian_lookup();
        }
    }

    fn dht_begin_guardian_lookup(&mut self) {
        let ks = self.k_s();
        let alpha = self.params.dht_alpha.max(1);
        if let Some(id) = self.start_lookup(ks, LookupKind::Value, Purpose::Discovery, alpha) {
            if let Some(d) = self.dht.discovery.as_mut() {
                d.lookup = Some(id);
            }
        } else {
            // Nobody to ask yet: fail fast, the join backoff re-queries.
            self.dht_finish_discovery();
        }
    }

    fn dht_send_get_peers(&mut self) {
        let Some(d) = self.dht.discovery.as_ref() else {
            return;
        };
        let (wanted, starved) = (d.wanted, d.starved);
        let me = self.node_id();
        let mut targets: Vec<PeerAddr> = self
            .dht
            .holders
            .iter()
            .filter(|g| g.node_id != me)
            .map(|g| g.addr.0)
            .collect();
        if targets.is_empty() {
            targets = self
                .dht
                .guardians
                .iter()
                .filter(|g| g.node_id != me)
                .map(|g| g.addr.0)
                .collect();
        }
        if targets.is_empty() {
            let exclude = BTreeSet::from([self.node_id()]);
            let ks = self.k_s();
            targets = self
                .dht
                .table
                .closest(&ks, self.params.dht_alpha, &exclude)
                .iter()
                .map(|r| r.addr.0)
                .collect();
        }
        targets.shuffle(&mut self.rng);
        targets.truncate(self.params.dht_alpha.max(1));
        if targets.is_empty() {
            self.dht_finish_discovery();
            return;
        }
        let sid = self.stream_id();
        let mut body = Vec::with_capacity(36);
        body.extend_from_slice(sid.as_bytes());
        body.push(starved.0);
        body.push(wanted.0);
        body.push(0);
        body.push(0);
        for to in &targets {
            let vb = self.validation_block(FrameType::GET_PEERS, &body);
            let f = GetPeers {
                validation: vb,
                stream_id: sid,
                starved_trees: starved,
                wanted_trees: wanted,
                want_relay_capable: false,
            };
            self.dht.rpc_sent += 1;
            self.send(
                *to,
                Channel::Udp,
                Frame::GetPeers(GetPeersFrame::Request(f)),
                None,
            );
        }
        if let Some(d) = self.dht.discovery.as_mut() {
            d.asked = targets.len();
            d.deadline = Some(self.now + self.params.dht_rpc_timeout);
        }
    }

    fn dht_on_get_peers_response(&mut self, from: PeerAddr, r: GetPeersResponse) {
        let _ = from;
        let Some(d) = self.dht.discovery.as_mut() else {
            return;
        };
        if d.deadline.is_none() {
            return;
        }
        d.responses.push(r);
        if d.responses.len() >= d.asked {
            self.dht_finish_discovery();
        }
    }

    fn dht_finish_discovery(&mut self) {
        let Some(d) = self.dht.discovery.take() else {
            return;
        };
        let mut seen = BTreeSet::new();
        let mut records = Vec::new();
        let mut sr: Option<StreamRecord> = None;
        for resp in d.responses {
            for r in resp.records {
                if seen.insert(r.node_id) {
                    records.push(r);
                }
            }
            if let Some(s) = resp.stream_record {
                if sr
                    .as_ref()
                    .map(|c| s.manifest_version > c.manifest_version)
                    .unwrap_or(true)
                {
                    sr = Some(s);
                }
            }
        }
        for r in &records {
            self.dht_learn(*r);
        }
        self.on_discovered(records, sr);
    }

    // ------------------------------------------------------------------ registration

    /// Register (or refresh) our record: with the oracle in oracle mode, with the
    /// guardians of `K_s` in DHT mode (Ch2 §2.3.2).
    pub(super) fn register_self(&mut self) {
        let rec = self.peer_record();
        if self.dht.mode == DiscoveryMode::Oracle {
            self.out.push_back(Output::Register(rec));
            return;
        }
        // Refresh every τ_ttl/2 and immediately on a change of class, trees or flags
        // (Ch2 §2.3.2 step 4); an unchanged record inside the window is not re-sent.
        let due = self
            .dht
            .next_register
            .map(|t| self.now >= t)
            .unwrap_or(true);
        if self.dht.registered == Some(rec) && !due {
            return;
        }
        let s = self.register_sample_log2();
        let sampled = in_register_sample(&rec.node_id, s) || rec.flags.source;
        self.dht.registered = Some(rec);
        self.dht.next_register =
            Some(self.now + Duration::from_micros(self.params.tau_ttl.as_micros() / 2));
        if !sampled {
            return;
        }
        // We may be one of the guardians ourselves.
        if self.serves_dht() {
            self.dht.store.register(rec, self.now);
        }
        let exclude = BTreeSet::from([self.node_id()]);
        let mut targets: Vec<PeerAddr> = self
            .dht
            .guardians
            .iter()
            .filter(|g| g.node_id != rec.node_id)
            .map(|g| g.addr.0)
            .collect();
        if targets.is_empty() {
            let ks = self.k_s();
            targets = self
                .dht
                .table
                .closest(&ks, self.params.dht_k, &exclude)
                .iter()
                .map(|r| r.addr.0)
                .collect();
        }
        if targets.is_empty() {
            // Nobody to tell yet: retry as soon as the first lookup completes.
            self.dht.registered = None;
            return;
        }
        let sid = self.stream_id();
        let port = self.cfg.addr.port();
        let ts = self.now.as_micros();
        let msg = RegisterPeer::signable_bytes(
            &sid,
            &rec.node_id,
            port,
            rec.node_class,
            rec.assigned_trees,
            rec.flags,
            ts,
        );
        let signature = self.identity.sign(&msg);
        let mut body = Vec::with_capacity(72);
        body.extend_from_slice(&port.to_be_bytes());
        body.push(RegisterPeer::PROTOCOL_UDP);
        body.push(rec.node_class as u8);
        body.push(rec.assigned_trees.0);
        body.push(rec.flags.to_byte());
        body.extend_from_slice(&[0, 0]);
        body.extend_from_slice(signature.as_bytes());
        for to in targets {
            let vb =
                self.identity
                    .validation_block(FrameType::REGISTER_PEER, self.dyn_nonce, ts, &body);
            let f = RegisterPeer {
                stream_id: sid,
                validation: vb,
                port,
                node_class: rec.node_class,
                assigned_trees: rec.assigned_trees,
                flags: rec.flags,
                signature,
            };
            self.dht.rpc_sent += 1;
            self.send(to, Channel::Udp, Frame::RegisterPeer(f), None);
        }
    }

    fn register_sample_log2(&self) -> u8 {
        if let Some(p) = self.publisher.as_ref() {
            return p.sample_log2;
        }
        self.dht
            .store
            .record
            .as_ref()
            .map(|r| r.register_sample_log2)
            .unwrap_or(self.known_sample_log2)
    }

    // ------------------------------------------------------------------ publisher STORE_RECORD

    /// DHT-mode `store_record`: fold the guardians' counts into the record, keep a
    /// copy ourselves and write it to the k closest contacts (Ch2 §2.3.2).
    pub(super) fn dht_store_record(&mut self) {
        let s = self.publisher.as_ref().map(|p| p.sample_log2).unwrap_or(0);
        let scale = 1u32 << s.min(16);
        let acks = std::mem::take(&mut self.dht.acks);
        if !acks.is_empty() {
            let mut a: Vec<u32> = acks.iter().map(|x| x.active_count).collect();
            let mut r: Vec<u32> = acks.iter().map(|x| x.relay_count).collect();
            if let (Some(a), Some(r)) = (median(&mut a), median(&mut r)) {
                self.swarm_size = a.saturating_mul(scale).max(1);
                self.relay_count = r.saturating_mul(scale);
            }
        } else if self.serves_dht() {
            let (a, r, _) = self
                .dht
                .store
                .counts(self.now, self.params.registration_active);
            self.swarm_size = a.max(1);
            self.relay_count = r;
        }
        // s = max(0, ceil(log2(N / N_0)))
        let n0 = self.params.n0_register.max(1) as f64;
        let want = if self.swarm_size as f64 > n0 {
            ((self.swarm_size as f64 / n0).log2().ceil() as u8).min(15)
        } else {
            0
        };
        let (n, r, now) = (self.swarm_size, self.relay_count, self.now);
        let Some(p) = self.publisher.as_mut() else {
            return;
        };
        p.sample_log2 = want;
        let rec = p.stream_record(n, r, now);
        self.dht.store.record = Some(rec.clone());
        let exclude = BTreeSet::from([self.node_id()]);
        let ks = self.k_s();
        let targets: Vec<PeerAddr> = self
            .dht
            .table
            .closest(&ks, self.params.dht_k, &exclude)
            .iter()
            .map(|x| x.addr.0)
            .collect();
        let body = bs_wire::Encode::to_vec(&rec);
        for to in targets {
            let vb = self.validation_block(FrameType::STORE_RECORD, &body);
            self.dht.rpc_sent += 1;
            self.send(
                to,
                Channel::Udp,
                Frame::StoreRecord(StoreRecord {
                    validation: vb,
                    record: Box::new(rec.clone()),
                }),
                None,
            );
        }
    }

    // ------------------------------------------------------------------ server side

    /// Dispatch for every DHT frame arriving on plain UDP.
    pub(super) fn on_dht_frame(&mut self, from: PeerAddr, frame: Frame) {
        match frame {
            Frame::FindNode(FindFrame::Request(r)) => {
                self.dht_serve_find(from, r, LookupKind::Node)
            }
            Frame::FindValue(FindFrame::Request(r)) => {
                self.dht_serve_find(from, r, LookupKind::Value)
            }
            Frame::FindNode(FindFrame::Response(r)) | Frame::FindValue(FindFrame::Response(r)) => {
                self.dht_on_find_response(from, r)
            }
            Frame::RegisterPeer(r) => self.dht_serve_register(from, r),
            Frame::GetPeers(GetPeersFrame::Request(g)) => self.dht_serve_get_peers(from, g),
            Frame::GetPeers(GetPeersFrame::Response(r)) => self.dht_on_get_peers_response(from, r),
            Frame::StoreRecord(s) => self.dht_serve_store(from, s),
            Frame::StoreRecordAck(a) if self.publisher.is_some() => self.dht.acks.push(a),
            _ => {}
        }
    }

    fn dht_serve_find(&mut self, from: PeerAddr, r: FindRequest, kind: LookupKind) {
        if !self.serves_dht() {
            return;
        }
        let ty = match kind {
            LookupKind::Node => FrameType::FIND_NODE,
            LookupKind::Value => FrameType::FIND_VALUE,
        };
        if !self.verify_vb(&r.validation, ty, &r.target.0, from, NodeClass::Leaf) {
            return;
        }
        self.dht.rpc_served += 1;
        let result = if kind == LookupKind::Value
            && r.target.0 == self.k_s()
            && self.is_guardian_candidate()
        {
            match self.dht.store.record.clone() {
                Some(sr) => FindResult::Value(Box::new(sr)),
                None => FindResult::Nodes(self.dht_closest_for(&r.target.0, &r.validation.node_id)),
            }
        } else {
            FindResult::Nodes(self.dht_closest_for(&r.target.0, &r.validation.node_id))
        };
        self.emit(Event::DhtServed {
            rpc: match kind {
                LookupKind::Node => "find_node",
                LookupKind::Value => "find_value",
            },
        });
        let resp = FindFrame::Response(FindResponse {
            responder: self.node_id(),
            result,
        });
        let f = match kind {
            LookupKind::Node => Frame::FindNode(resp),
            LookupKind::Value => Frame::FindValue(resp),
        };
        self.send(from, Channel::Udp, f, None);
    }

    /// Whether we are among the k closest contacts to `K_s` that we know of: only
    /// then do we act as a guardian toward lookups (Ch2 §2.3.2 "Who Is a Guardian").
    fn is_guardian_candidate(&self) -> bool {
        let ks = self.k_s();
        let me = crate::dht::distance(&self.node_id().0, &ks);
        let exclude = BTreeSet::from([self.node_id()]);
        let closest = self.dht.table.closest(&ks, self.params.dht_k, &exclude);
        closest.len() < self.params.dht_k
            || closest
                .last()
                .map(|r| crate::dht::distance(&r.node_id.0, &ks) > me)
                .unwrap_or(true)
    }

    fn dht_closest_for(&self, target: &Key, requester: &NodeId) -> Vec<PeerRecord> {
        let exclude = BTreeSet::from([*requester]);
        let mut v = self.dht.table.closest(target, self.params.dht_k, &exclude);
        // We are a contact too: a joiner must be able to learn about the responder's
        // neighbourhood including the responder itself (it already knows our address,
        // but not our record).
        let me = self.peer_record();
        if usable_contact(&me) && v.len() < self.params.dht_k && *requester != me.node_id {
            v.push(me);
        }
        v.truncate(self.params.dht_k);
        v
    }

    fn dht_serve_register(&mut self, from: PeerAddr, r: RegisterPeer) {
        if !self.serves_dht() || r.stream_id != self.stream_id() {
            return;
        }
        let mut body = Vec::with_capacity(72);
        body.extend_from_slice(&r.port.to_be_bytes());
        body.push(RegisterPeer::PROTOCOL_UDP);
        body.push(r.node_class as u8);
        body.push(r.assigned_trees.0);
        body.push(r.flags.to_byte());
        body.extend_from_slice(&[0, 0]);
        body.extend_from_slice(r.signature.as_bytes());
        if !self.verify_vb(
            &r.validation,
            FrameType::REGISTER_PEER,
            &body,
            from,
            r.node_class,
        ) {
            return;
        }
        let msg = RegisterPeer::signable_bytes(
            &r.stream_id,
            &r.validation.node_id,
            r.port,
            r.node_class,
            r.assigned_trees,
            r.flags,
            r.validation.timestamp_us,
        );
        if bs_crypto::Verifier::verify(&r.validation.public_key, &msg, &r.signature).is_err() {
            return;
        }
        let record = PeerRecord {
            node_id: r.validation.node_id,
            node_class: r.node_class,
            assigned_trees: r.assigned_trees,
            flags: r.flags,
            addr: WireAddr(std::net::SocketAddr::new(from.ip(), r.port)),
        };
        self.dht.store.register(record, self.now);
        self.dht.store.registrations += 1;
        self.dht.rpc_served += 1;
        self.emit(Event::DhtServed { rpc: "register" });
        // The registrant is a candidate contact (Ch2 §2.1.3 step 4).
        self.dht_learn(record);
    }

    fn dht_serve_get_peers(&mut self, from: PeerAddr, g: GetPeers) {
        if !self.serves_dht() || g.stream_id != self.stream_id() {
            return;
        }
        let mut body = Vec::with_capacity(36);
        body.extend_from_slice(g.stream_id.as_bytes());
        body.push(g.starved_trees.0);
        body.push(g.wanted_trees.0);
        body.push(g.want_relay_capable as u8);
        body.push(0);
        if !self.verify_vb(
            &g.validation,
            FrameType::GET_PEERS,
            &body,
            from,
            NodeClass::Leaf,
        ) {
            return;
        }
        let now = self.now;
        self.dht.store.expire(now, self.params.tau_ttl);
        self.dht
            .store
            .queriers
            .insert(g.validation.node_id, (now, g.starved_trees));
        self.dht.store.queries += 1;
        self.dht.rpc_served += 1;
        let who = g.validation.node_id;
        let window = self.params.registration_active;
        let membership = g.wanted_trees.is_empty();
        let mut pool: Vec<PeerRecord> = self
            .dht
            .store
            .active(now, window)
            .filter(|r| r.node_id != who)
            .filter(|r| {
                membership
                    || (r.node_class == NodeClass::Relay
                        && !r.assigned_trees.intersection(g.wanted_trees).is_empty())
            })
            .copied()
            .collect();
        // Uniformly random per query, stratified across the wanted trees.
        pool.shuffle(&mut self.rng);
        let max = bs_wire::consts::MAX_GET_PEERS_RECORDS;
        let mut out: Vec<PeerRecord> = Vec::with_capacity(max);
        if !membership {
            let trees: Vec<bs_wire::TreeId> = (1..=8u8)
                .map(bs_wire::TreeId)
                .filter(|t| g.wanted_trees.contains(*t))
                .collect();
            let mut taken = BTreeSet::new();
            'outer: loop {
                let mut progress = false;
                for t in &trees {
                    if let Some(r) = pool
                        .iter()
                        .find(|r| !taken.contains(&r.node_id) && r.assigned_trees.contains(*t))
                    {
                        taken.insert(r.node_id);
                        out.push(*r);
                        progress = true;
                        if out.len() >= max {
                            break 'outer;
                        }
                    }
                }
                if !progress {
                    break;
                }
            }
        } else {
            out.extend(pool.into_iter().take(max));
        }
        self.emit(Event::DhtServed { rpc: "get_peers" });
        let resp = GetPeersResponse {
            guardian: self.node_id(),
            sampled: self.register_sample_log2() > 0,
            records: out,
            stream_record: self.dht.store.record.clone(),
        };
        self.send(
            from,
            Channel::Udp,
            Frame::GetPeers(GetPeersFrame::Response(resp)),
            None,
        );
    }

    fn dht_serve_store(&mut self, from: PeerAddr, s: StoreRecord) {
        if !self.serves_dht() {
            return;
        }
        let body = bs_wire::Encode::to_vec(&*s.record);
        if !self.verify_vb(
            &s.validation,
            FrameType::STORE_RECORD,
            &body,
            from,
            NodeClass::Relay,
        ) {
            return;
        }
        let rec = *s.record;
        let key_ok = bs_crypto::stream_id(&rec.publisher_pubkey) == self.stream_id();
        let sig_ok = bs_crypto::Verifier::verify(
            &rec.publisher_pubkey,
            &rec.signable_bytes(),
            &rec.signature,
        )
        .is_ok();
        let newer = self
            .dht
            .store
            .record
            .as_ref()
            .map(|c| rec.manifest_version >= c.manifest_version)
            .unwrap_or(true);
        let accepted = key_ok && sig_ok && newer;
        if accepted {
            self.dht.store.record = Some(rec);
            // Viewers learn the matrix and live edge from the record they hold as guardians.
            if self.publisher.is_none() && self.matrix.is_none() {
                if let Some(r) = self.dht.store.record.clone() {
                    self.on_discovered(Vec::new(), Some(r));
                }
            }
        }
        let now = self.now;
        self.dht.store.expire(now, self.params.tau_ttl);
        let (active, relay, per_tree) = self.dht.store.counts(now, self.params.registration_active);
        let (query_count, starved) = self.dht.store.query_counts();
        self.dht.rpc_served += 1;
        self.emit(Event::DhtServed { rpc: "store" });
        let ack = StoreRecordAck {
            guardian: self.node_id(),
            accepted,
            resp_flags: 0,
            query_count,
            active_count: active,
            relay_count: relay,
            per_tree_relay: per_tree,
            starved,
        };
        self.send(from, Channel::Udp, Frame::StoreRecordAck(ack), None);
    }

    // ------------------------------------------------------------------ timers

    pub(super) fn dht_next_timer(&self) -> Option<Instant> {
        if self.dht.mode != DiscoveryMode::Dht {
            return None;
        }
        let mut t: Option<Instant> = None;
        let mut m = |x: Option<Instant>| {
            if let Some(x) = x {
                t = Some(t.map_or(x, |c| c.min(x)));
            }
        };
        let to = self.params.dht_rpc_timeout;
        if !self.dht.lookups.is_empty() {
            m(Some(self.now + to));
        }
        if let Some(d) = self.dht.discovery.as_ref() {
            m(d.deadline);
        }
        m(self.dht.next_register);
        m(self.dht.next_bootstrap);
        m(self.dht.next_discovery_retry);
        if let Some((_, t0)) = self.dht.pending_pings.values().min_by_key(|(_, t)| *t) {
            m(Some(*t0 + to * 2));
        }
        t
    }

    pub(super) fn dht_timers(&mut self) {
        if self.dht.mode != DiscoveryMode::Dht || self.state == Lifecycle::Terminated {
            return;
        }
        let now = self.now;
        let to = self.params.dht_rpc_timeout;
        // Pings that went unanswered: the contact is not reachable (or not there).
        let dead: Vec<PeerAddr> = self
            .dht
            .pending_pings
            .iter()
            .filter(|(_, (_, t))| now.duration_since(*t) >= to * 2)
            .map(|(a, _)| *a)
            .collect();
        for a in dead {
            if let Some((Some(rec), _)) = self.dht.pending_pings.remove(&a) {
                self.dht.table.remove(&rec.node_id);
            }
        }
        // Bootstrap retry while the table is empty (seed down or answer lost).
        if let Some(t) = self.dht.next_bootstrap {
            if now >= t {
                if self.dht.table.is_empty() {
                    if self.dht.lookups.is_empty() {
                        let me = self.node_id().0;
                        self.start_lookup(me, LookupKind::Node, Purpose::SelfLookup, 0);
                    }
                    self.dht.next_bootstrap = Some(now + Duration::from_secs(2));
                } else {
                    self.dht.next_bootstrap = None;
                }
            }
        }
        if !self.dht.lookups.is_empty() {
            self.dht_drive_lookups();
        }
        if let Some(d) = self.dht.discovery.as_ref() {
            if d.deadline.map(|t| now >= t).unwrap_or(false) {
                self.dht_finish_discovery();
            }
        }
        if let Some(t) = self.dht.next_register {
            if now >= t {
                self.register_self();
            }
        }
        // A viewer without a Stream Record (lost GET_PEERS answer, empty guardian)
        // re-queries; nothing else would, since join rounds need the matrix.
        if self.state == Lifecycle::Discovery && self.matrix.is_none() && self.publisher.is_none() {
            match self.dht.next_discovery_retry {
                None => self.dht.next_discovery_retry = Some(now + Duration::from_secs(1)),
                Some(t) if now >= t => {
                    self.dht.next_discovery_retry = Some(now + Duration::from_secs(1));
                    if self.dht.discovery.is_none() {
                        self.discovery_inflight = false;
                        self.request_discovery(TreeSet::EMPTY);
                    }
                }
                _ => {}
            }
        } else {
            self.dht.next_discovery_retry = None;
        }
    }
}
