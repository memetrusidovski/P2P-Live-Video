//! The peer orchestrator: dispatches inputs to the forest, join, swarm and
//! publisher components and emits outputs. Section references are to the
//! protocol specification.

use std::collections::{HashMap, HashSet, VecDeque};

use bs_crypto::pow::Difficulty;
use bs_crypto::{rendezvous, Identity, Signer};
use bs_media::fec::FecEncoder;
use bs_media::{Ladder, LayeredChunk, SlicingMatrix};
use bs_wire::frames::*;
use bs_wire::Encode;
use bs_wire::{
    DisconnectReason, Frame, FrameType, NodeClass, NodeId, PeerFlags, PeerRecord, PublicKeyBytes,
    SegmentSeq, StreamId, TreeId, TreeSet, ValidationBlock, WireAddr,
};
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

use crate::forest::{ChildLink, ParentLink, TreeState};
use crate::io::{Channel, Command, Event, Input, Output, PeerAddr};
use crate::join::{Candidate, JoinState, JoinStats, ProbeResult};
use crate::params::Params;
use crate::publisher::Publisher;
use crate::swarm::{ManifestVerdict, SwarmBuffer, VerifiedBlock};
use crate::time::{Duration, Instant};
use crate::Lifecycle;

/// What this node is.
#[derive(Debug, Clone)]
pub enum Role {
    /// The broadcaster: root of every tree.
    Publisher {
        /// Layer ladder.
        ladder: Ladder,
        /// Forest size M.
        forest_size: u8,
    },
    /// A viewer (relay or leaf class) subscribing to layers `0..=top_layer`.
    Viewer {
        /// Highest layer wanted.
        top_layer: u8,
    },
}

/// Static configuration.
#[derive(Debug, Clone)]
pub struct NodeConfig {
    /// Role.
    pub role: Role,
    /// Self-declared class (publisher is always RELAY).
    pub node_class: NodeClass,
    /// Upload capacity, kbps.
    pub upload_kbps: u32,
    /// Our external address (as peers will observe it).
    pub addr: PeerAddr,
    /// Stream to join (viewers). Publishers derive it from their key.
    pub stream_id: StreamId,
    /// Parameters.
    pub params: Params,
    /// RNG seed.
    pub seed: u64,
}

#[derive(Debug)]
struct Session {
    node_id: Option<NodeId>,
    open: bool,
    last_rx: Instant,
    ping_sent_at: Option<Instant>,
}

/// A peer.
pub struct Node {
    cfg: NodeConfig,
    identity: Identity,
    params: Params,
    #[allow(dead_code)]
    rng: ChaCha8Rng,
    state: Lifecycle,
    publisher: Option<Publisher>,
    publisher_key: Option<PublicKeyBytes>,
    matrix: Option<SlicingMatrix>,
    swarm_size: u32,
    relay_count: u32,
    trees: Vec<TreeState>,
    join_stats: Vec<JoinStats>,
    top_layer: u8,
    shed_retry_at: Option<Instant>,
    swarm: SwarmBuffer,
    sessions: HashMap<PeerAddr, Session>,
    passive: HashMap<NodeId, PeerRecord>,
    /// Peers that delivered blocks failing verification; never chosen again this run.
    bad_peers: HashSet<NodeId>,
    source_record: Option<PeerRecord>,
    discovery_inflight: bool,
    out: VecDeque<Output>,
    dyn_nonce: u64,
    next_liveness: Instant,
    started_at: Option<Instant>,
    first_ok_chunk: bool,
    manifests_forwarded: HashSet<(u32, u8, PeerAddr)>,
    /// STREAM_END received: terminate once the playout buffer is drained.
    ending: bool,
    /// The verified STREAM_DESCRIPTOR in force (publisher: its own signed copy).
    descriptor: Option<StreamDescriptor>,
    /// Children already sent the current descriptor version.
    descriptor_sent: HashSet<(u32, PeerAddr)>,
    now: Instant,
}

impl Node {
    /// Create a node. Solves the static puzzle at `params.c1`.
    pub fn new(cfg: NodeConfig) -> Self {
        let difficulty = Difficulty {
            c1: cfg.params.c1,
            c2_min: cfg.params.c2_min,
        };
        let mut seed = [0u8; 32];
        seed[..8].copy_from_slice(&cfg.seed.to_le_bytes());
        seed[8..16].copy_from_slice(&cfg.addr.port().to_le_bytes()[..].repeat(4)[..8]);
        let identity = Identity::from_seed(seed, difficulty);
        let params = cfg.params.clone();
        let swarm = SwarmBuffer::new(&params);
        let node_class = match cfg.role {
            Role::Publisher { .. } => NodeClass::Relay,
            Role::Viewer { .. } => cfg.node_class,
        };
        let mut cfg = cfg;
        cfg.node_class = node_class;
        let top_layer = match cfg.role {
            Role::Viewer { top_layer } => top_layer,
            Role::Publisher { ref ladder, .. } => ladder.len().saturating_sub(1) as u8,
        };
        Self {
            rng: ChaCha8Rng::seed_from_u64(cfg.seed),
            identity,
            params,
            state: Lifecycle::Bootstrap,
            publisher: None,
            publisher_key: None,
            matrix: None,
            swarm_size: 0,
            relay_count: 0,
            trees: Vec::new(),
            join_stats: Vec::new(),
            top_layer,
            shed_retry_at: None,
            swarm,
            sessions: HashMap::new(),
            passive: HashMap::new(),
            bad_peers: HashSet::new(),
            source_record: None,
            discovery_inflight: false,
            out: VecDeque::new(),
            dyn_nonce: 0,
            next_liveness: Instant::ZERO,
            started_at: None,
            first_ok_chunk: false,
            manifests_forwarded: HashSet::new(),
            ending: false,
            descriptor: None,
            descriptor_sent: HashSet::new(),
            now: Instant::ZERO,
            cfg,
        }
    }

    // ------------------------------------------------------------------ accessors

    /// NodeID.
    pub fn node_id(&self) -> NodeId {
        self.identity.node_id()
    }
    /// Address.
    pub fn addr(&self) -> PeerAddr {
        self.cfg.addr
    }
    /// Lifecycle state.
    pub fn state(&self) -> Lifecycle {
        self.state
    }
    /// Stream id.
    pub fn stream_id(&self) -> StreamId {
        match self.cfg.role {
            Role::Publisher { .. } => bs_crypto::stream_id(&self.identity.public_key()),
            Role::Viewer { .. } => self.cfg.stream_id,
        }
    }
    /// Publisher public key (publishers know their own).
    pub fn publisher_key(&self) -> Option<PublicKeyBytes> {
        self.publisher_key
    }
    /// Whether this is the source.
    pub fn is_source(&self) -> bool {
        self.publisher.is_some()
    }
    /// Per-tree state (read-only, for drivers and invariant checkers).
    pub fn trees(&self) -> &[TreeState] {
        &self.trees
    }
    /// The swarm buffer.
    pub fn swarm(&self) -> &SwarmBuffer {
        &self.swarm
    }
    /// Highest subscribed layer.
    pub fn top_layer(&self) -> u8 {
        self.top_layer
    }
    /// Matrix in force.
    pub fn matrix(&self) -> Option<&SlicingMatrix> {
        self.matrix.as_ref()
    }
    /// Node class.
    pub fn node_class(&self) -> NodeClass {
        self.cfg.node_class
    }
    /// Our Peer Record as we would register it.
    pub fn peer_record(&self) -> PeerRecord {
        let mut assigned = TreeSet::EMPTY;
        for t in &self.trees {
            if t.assigned {
                assigned.insert(t.id);
            }
        }
        PeerRecord {
            node_id: self.node_id(),
            node_class: self.cfg.node_class,
            assigned_trees: assigned,
            flags: PeerFlags {
                source: self.is_source(),
                ..PeerFlags::PUBLIC
            },
            addr: WireAddr(self.cfg.addr),
        }
    }

    /// Drain the next output.
    pub fn poll_output(&mut self) -> Option<Output> {
        self.out.pop_front()
    }

    /// When the driver must next call `handle(Input::Tick)`.
    pub fn next_timer(&self) -> Option<Instant> {
        if self.state == Lifecycle::Terminated {
            return None;
        }
        let mut t = Some(self.next_liveness);
        let mut m = |x: Option<Instant>| {
            if let Some(x) = x {
                t = Some(t.map_or(x, |cur| cur.min(x)));
            }
        };
        m(self.swarm.next_deadline());
        m(self.shed_retry_at);
        for tree in &self.trees {
            match &tree.join {
                JoinState::Probing { deadline, .. } => m(Some(*deadline)),
                JoinState::Requesting { deadline, .. } => m(Some(*deadline)),
                JoinState::Backoff { until } => m(Some(*until)),
                _ => {}
            }
        }
        t
    }

    // ------------------------------------------------------------------ input

    /// Feed one input at time `now`.
    pub fn handle(&mut self, input: Input, now: Instant) {
        self.now = now;
        if self.state == Lifecycle::Terminated {
            return;
        }
        match input {
            Input::Cmd(cmd) => self.handle_cmd(cmd),
            Input::SessionOpened { peer, node_id } => self.on_session_opened(peer, node_id),
            Input::SessionClosed { peer } => self.on_session_closed(peer, "session closed"),
            Input::Frame {
                from,
                channel,
                frame,
            } => self.on_frame(from, channel, frame),
            Input::Tick => {}
        }
        self.run_timers();
    }

    fn emit(&mut self, e: Event) {
        self.out.push_back(Output::Event(e));
    }

    fn set_state(&mut self, to: Lifecycle) {
        if self.state != to {
            let from = self.state;
            self.state = to;
            self.emit(Event::State { from, to });
        }
    }

    fn handle_cmd(&mut self, cmd: Command) {
        match cmd {
            Command::Start => self.start(),
            Command::Discovered {
                records,
                stream_record,
            } => self.on_discovered(records, stream_record),
            Command::PublishChunk(chunk) => self.publish_chunk(chunk),
            Command::EndStream => self.end_stream(),
            Command::SetDescriptor(d) => self.set_descriptor(d),
            Command::Quit => self.quit(),
        }
    }

    fn start(&mut self) {
        self.started_at = Some(self.now);
        self.next_liveness = self.now;
        let d = Difficulty {
            c1: self.params.c1,
            c2_min: self.params.c2_min,
        };
        let c2 = d.dynamic_required(self.swarm_size.max(2), self.cfg.node_class, false);
        self.dyn_nonce = self.identity.solve_dynamic(self.cfg.addr.ip(), c2);
        match self.cfg.role.clone() {
            Role::Publisher {
                ladder,
                forest_size,
            } => {
                let p = Publisher::new(self.identity.clone(), ladder, forest_size)
                    .expect("valid ladder");
                self.publisher_key = Some(self.identity.public_key());
                self.matrix = Some(p.matrix.clone());
                self.publisher = Some(p);
                self.setup_trees();
                for t in &mut self.trees {
                    t.assigned = true;
                    t.subscribed = true;
                    t.depth = 0;
                }
                self.recompute_slots();
                let rec = self.peer_record();
                self.out.push_back(Output::Register(rec));
                self.store_record();
                self.set_state(Lifecycle::Active);
            }
            Role::Viewer { .. } => {
                self.set_state(Lifecycle::Discovery);
                self.request_discovery(TreeSet::EMPTY);
            }
        }
    }

    fn store_record(&mut self) {
        let (n, r, now) = (self.swarm_size, self.relay_count, self.now);
        if let Some(p) = self.publisher.as_mut() {
            let rec = p.stream_record(n, r, now);
            self.out.push_back(Output::StoreRecord(rec));
        }
    }

    fn setup_trees(&mut self) {
        let m = self.matrix.as_ref().map(|x| x.m()).unwrap_or(0);
        self.trees = (1..=m).map(|i| TreeState::new(TreeId(i))).collect();
        self.join_stats = (0..m).map(|_| JoinStats::default()).collect();
    }

    /// K_v(m) per assigned tree from upload, tree bitrate and parity (Ch1 §1.2.1 §1.3).
    fn recompute_slots(&mut self) {
        let Some(matrix) = self.matrix.clone() else {
            return;
        };
        let assigned_count = self.trees.iter().filter(|t| t.assigned).count() as u32;
        let parity = self.params.default_parity as f64;
        for t in &mut self.trees {
            if !t.assigned {
                t.k_v = 0;
                continue;
            }
            let b = matrix.tree(t.id).map(|i| i.bitrate_kbps).unwrap_or(0);
            t.k_v = self
                .params
                .slots(self.cfg.upload_kbps, assigned_count, b, parity);
        }
    }

    fn request_discovery(&mut self, wanted: TreeSet) {
        if self.discovery_inflight {
            return;
        }
        self.discovery_inflight = true;
        let mut starved = TreeSet::EMPTY;
        for (i, t) in self.trees.iter().enumerate() {
            if t.wants_parent() && t.parent.is_none() && self.join_stats[i].failed_rounds > 0 {
                starved.insert(t.id);
            }
        }
        self.out.push_back(Output::Discover {
            wanted_trees: wanted,
            starved_trees: starved,
        });
    }

    fn on_discovered(
        &mut self,
        records: Vec<PeerRecord>,
        stream_record: Option<bs_wire::StreamRecord>,
    ) {
        self.discovery_inflight = false;
        for r in records {
            if r.node_id == self.node_id() {
                continue;
            }
            if r.flags.source {
                self.source_record = Some(r);
            }
            self.passive.insert(r.node_id, r);
        }
        if let Some(sr) = stream_record {
            if self.publisher.is_none() {
                let ok = bs_crypto::Verifier::verify(
                    &sr.publisher_pubkey,
                    &sr.signable_bytes(),
                    &sr.signature,
                )
                .is_ok()
                    && bs_crypto::stream_id(&sr.publisher_pubkey) == self.cfg.stream_id;
                if !ok {
                    self.emit(Event::Note {
                        text: "stream record rejected (signature/stream id)".into(),
                    });
                } else {
                    self.publisher_key = Some(sr.publisher_pubkey);
                    self.swarm_size = sr.swarm_size;
                    self.relay_count = sr.relay_count;
                    let layer_count = sr.trees.iter().map(|t| t.layer + 1).max().unwrap_or(1);
                    let m = SlicingMatrix::from_entries(
                        sr.manifest_version.min(255) as u8,
                        &sr.trees,
                        layer_count,
                    );
                    // The manifest's SlicingMatrixVersion is the publisher's matrix version,
                    // which is 1 for the initial matrix in M1.
                    let m = SlicingMatrix { version: 1, ..m };
                    if self.matrix.is_none() {
                        self.matrix = Some(m);
                        self.setup_trees();
                        self.configure_tree_roles();
                        self.recompute_slots();
                        let rec = self.peer_record();
                        self.out.push_back(Output::Register(rec));
                    }
                }
            }
        }
        if self.state == Lifecycle::Discovery && self.matrix.is_some() {
            self.set_state(Lifecycle::Connecting);
        }
        // Advance every tree waiting on discovery, and start joins for detached trees.
        let ids: Vec<TreeId> = self.trees.iter().map(|t| t.id).collect();
        for id in ids {
            let t = &self.trees[id.index()];
            if !t.wants_parent() || t.parent.is_some() {
                continue;
            }
            match t.join {
                JoinState::Discovering { .. } | JoinState::Idle => self.begin_probing(id),
                _ => {}
            }
        }
    }

    /// Decide assigned (relay duty) and subscribed (rendered) trees (Ch1 §1.2.5, §1.3.1).
    fn configure_tree_roles(&mut self) {
        let Some(matrix) = self.matrix.as_ref() else {
            return;
        };
        let m = matrix.m();
        let subscribed =
            matrix.trees_for_layer_prefix(self.top_layer.min(matrix.layer_count.saturating_sub(1)));
        let assigned = if self.cfg.node_class == NodeClass::Relay && !self.is_source() {
            TreeSet::single(rendezvous::assigned_tree(&self.node_id(), m))
        } else {
            TreeSet::EMPTY
        };
        for t in &mut self.trees {
            t.subscribed = subscribed.contains(t.id);
            t.assigned = assigned.contains(t.id);
        }
    }

    // ------------------------------------------------------------------ join (Ch1 §1.2.2 §2.2)

    fn candidates_for(&self, tree: TreeId) -> Vec<PeerRecord> {
        let my_parents: HashSet<NodeId> = self
            .trees
            .iter()
            .filter_map(|t| t.parent.as_ref().map(|p| p.node_id))
            .collect();
        let my_children: HashSet<NodeId> = self.trees[tree.index()]
            .children
            .values()
            .map(|c| c.node_id)
            .collect();
        let mut v: Vec<PeerRecord> = self
            .passive
            .values()
            .filter(|r| r.relays(tree))
            .filter(|r| r.flags.reachability != bs_wire::Reachability::Symmetric)
            .filter(|r| r.node_id != self.node_id())
            .filter(|r| !self.bad_peers.contains(&r.node_id))
            .filter(|r| !my_parents.contains(&r.node_id) || r.flags.source)
            .filter(|r| !my_children.contains(&r.node_id))
            .copied()
            .collect();
        if v.is_empty() {
            if let Some(s) = self.source_record {
                if !my_parents.contains(&s.node_id) || self.trees[tree.index()].parent.is_none() {
                    v.push(s);
                }
            }
        }
        v.sort_by_key(|r| r.node_id);
        v
    }

    fn begin_probing(&mut self, tree: TreeId) {
        if self.is_source() {
            return;
        }
        let cands = self.candidates_for(tree);
        if cands.len() < 2 && !self.discovery_inflight {
            // Thin pool: ask discovery for this tree, keep going with what we have.
            self.request_discovery(TreeSet::single(tree));
            if cands.is_empty() {
                self.trees[tree.index()].join = JoinState::Discovering { since: self.now };
                return;
            }
        }
        let deadline = self.now + self.params.probe_timeout;
        let mut candidates = Vec::with_capacity(cands.len());
        let sid = self.stream_id();
        for r in cands {
            let body = {
                let mut b = Vec::with_capacity(33);
                b.extend_from_slice(sid.as_bytes());
                b.push(tree.0);
                b
            };
            let vb = self.validation_block(FrameType::PROBE, &body);
            self.send(
                r.addr.0,
                Channel::Udp,
                Frame::Probe(Probe {
                    validation: vb,
                    stream_id: sid,
                    tree_id: tree,
                }),
                None,
            );
            candidates.push(Candidate {
                record: r,
                probed_at: self.now,
                response: None,
            });
        }
        self.join_stats[tree.index()].rounds += 1;
        self.join_stats[tree.index()].saw_warming = false;
        self.trees[tree.index()].join = JoinState::Probing {
            candidates,
            deadline,
        };
    }

    fn on_probe_response(&mut self, from: PeerAddr, pr: ProbeResponse) {
        // Which tree? Match the responder against probing candidates; the response
        // names no tree, so a responder probed for two trees at once is ambiguous —
        // we probe one tree at a time per candidate (candidates_for excludes current
        // parents), and fall back to the first match.
        let mut matched: Option<TreeId> = None;
        for t in &self.trees {
            if let JoinState::Probing { candidates, .. } = &t.join {
                if candidates
                    .iter()
                    .any(|c| c.record.node_id == pr.sender && c.response.is_none())
                {
                    matched = Some(t.id);
                    break;
                }
            }
        }
        let Some(tree) = matched else { return };
        let params = self.params.clone();
        let now = self.now;
        let mut all_answered = false;
        if let JoinState::Probing { candidates, .. } = &mut self.trees[tree.index()].join {
            if let Some(c) = candidates
                .iter_mut()
                .find(|c| c.record.node_id == pr.sender && c.response.is_none())
            {
                let rtt_ms = now.duration_since(c.probed_at).as_millis_f64();
                let score = params.score(pr.k_avail, pr.reliability_f32(), rtt_ms, pr.hop_count);
                c.response = Some(ProbeResult {
                    k_avail: pr.k_avail,
                    reliability: pr.reliability_f32(),
                    hop: pr.hop_count,
                    tree_state: pr.tree_state,
                    rtt_ms,
                    score,
                });
                c.record.addr = WireAddr(from);
                // Refresh the pool with what the responder says about itself.
                self.passive.entry(pr.sender).and_modify(|r| {
                    r.assigned_trees = pr.assigned_trees;
                    r.node_class = pr.node_class;
                });
            }
            all_answered = candidates.iter().all(|c| c.response.is_some());
        }
        if all_answered {
            self.finish_probing(tree);
        }
    }

    fn finish_probing(&mut self, tree: TreeId) {
        let JoinState::Probing { candidates, .. } =
            std::mem::replace(&mut self.trees[tree.index()].join, JoinState::Idle)
        else {
            return;
        };
        let d_max = self.params.d_max;
        let we_relay_this_tree = self.trees[tree.index()].assigned;
        let mut scored: Vec<(PeerRecord, f64, u8, f64)> = Vec::new();
        let mut saturated: Vec<(PeerRecord, f64, u8, f64)> = Vec::new();
        let mut saw_warming = false;
        let mut saturated_seen = 0usize;
        let n = candidates.len();
        for c in candidates {
            let Some(r) = c.response else { continue };
            match r.tree_state {
                bs_wire::TreeState::Warming => saw_warming = true,
                bs_wire::TreeState::Unparented => saturated_seen += 1,
                bs_wire::TreeState::Serving => {
                    if r.hop >= d_max {
                        saturated_seen += 1;
                        continue;
                    }
                    if r.k_avail == 0 {
                        saturated_seen += 1;
                    }
                    if r.k_avail > 0 {
                        scored.push((c.record, r.score, r.hop, r.rtt_ms));
                    } else if we_relay_this_tree {
                        // ISSUE-061: a relay of this tree may displace a pure subscriber,
                        // so it asks saturated parents too, after every free one.
                        saturated.push((c.record, r.score, r.hop, r.rtt_ms));
                    }
                }
            }
        }
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        saturated.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.extend(saturated);
        self.join_stats[tree.index()].saw_warming = saw_warming;
        if scored.is_empty() {
            let _ = n;
            self.join_round_failed(tree, saturated_seen);
            return;
        }
        self.request_next(tree, scored, 0);
    }

    fn request_next(
        &mut self,
        tree: TreeId,
        mut queue: Vec<(PeerRecord, f64, u8, f64)>,
        saturated: usize,
    ) {
        if queue.is_empty() {
            self.join_round_failed(tree, saturated);
            return;
        }
        let (rec, _score, hop, rtt_ms) = queue.remove(0);
        let addr = rec.addr.0;
        let open = self.sessions.get(&addr).map(|s| s.open).unwrap_or(false);
        if !open {
            self.open_session(addr);
        }
        self.trees[tree.index()].join = JoinState::Requesting {
            queue,
            current: addr,
            current_id: rec.node_id,
            current_hop: hop,
            current_rtt_ms: rtt_ms,
            awaiting_session: !open,
            deadline: self.now + self.params.probe_timeout * 3,
            saturated,
        };
        if open {
            self.send_neighbor(tree, addr);
        }
    }

    fn send_neighbor(&mut self, tree: TreeId, addr: PeerAddr) {
        let repairing = self.trees[tree.index()].repairing;
        let f = Neighbor {
            sender: self.node_id(),
            priority: if repairing {
                Priority::High
            } else {
                Priority::Low
            },
            tree_id: tree,
            node_class: self.cfg.node_class,
            assigned_trees: self.peer_record().assigned_trees,
        };
        self.send(addr, Channel::Control, Frame::Neighbor(f), None);
    }

    /// `saturated` counts candidates that *refused* (saturated / depth) or advertised
    /// no slots; a round of pure timeouts is a discovery failure, not a shed signal
    /// (App D §D.4.3b), and does not advance the shed counter.
    fn join_round_failed(&mut self, tree: TreeId, saturated: usize) {
        let idx = tree.index();
        let candidates = saturated;
        if !self.join_stats[idx].saw_warming && saturated > 0 {
            self.join_stats[idx].failed_rounds += 1;
        }
        let failed = self.join_stats[idx].failed_rounds;
        self.emit(Event::JoinRoundFailed {
            tree: tree.0,
            failed_rounds: failed,
            candidates,
        });
        // Shed rule (Ch1 §1.1.5 §5.3): after N consecutive failed rounds on a
        // non-base, non-assigned tree, drop that layer and everything above.
        let layer = self
            .matrix
            .as_ref()
            .and_then(|m| m.tree(tree))
            .map(|t| t.layer)
            .unwrap_or(0);
        let subscribed = self.trees[idx].subscribed;
        if failed >= self.params.shed_after_failed_rounds && layer > 0 && subscribed {
            // Shed the layer (subscription); an assigned tree keeps retrying below.
            self.shed_to(layer - 1);
            if !self.trees[idx].wants_parent() {
                return;
            }
        }
        // Bounded exponential backoff, capped at 4x; a repair or a round that saw a
        // warming candidate retries at the base cadence, since capacity is imminent
        // and the drain window is only five segments.
        let repairing = self.trees[idx].repairing
            || self.trees[idx].parent.is_none() && self.trees[idx].lost_at.is_some();
        let steps = if repairing || self.join_stats[idx].saw_warming {
            0
        } else {
            failed.min(2)
        };
        let backoff = self.params.join_backoff * (1u64 << steps);
        self.trees[idx].join = JoinState::Backoff {
            until: self.now + backoff,
        };
        // Fresh sample next round (every GET_PEERS draws a new sample).
        self.request_discovery(TreeSet::single(tree));
    }

    fn shed_to(&mut self, top: u8) {
        self.top_layer = top;
        self.emit(Event::LayerShed { top_layer: top });
        let Some(matrix) = self.matrix.as_ref() else {
            return;
        };
        let subscribed = matrix.trees_for_layer_prefix(top);
        let ids: Vec<TreeId> = self.trees.iter().map(|t| t.id).collect();
        for id in ids {
            let t = &mut self.trees[id.index()];
            t.subscribed = subscribed.contains(id);
            if !t.wants_parent() {
                t.join = JoinState::Idle;
                // Leaving the tree is not a repair: clear the repair bookkeeping so a
                // later re-subscription is not counted as a 10 s "repair".
                t.repairing = false;
                t.lost_at = None;
                self.join_stats[id.index()].failed_rounds = 0;
                if let Some(p) = t.detach_parent(self.now) {
                    t.lost_at = None;
                    let old = t.old_parent.take();
                    let f = Disconnect {
                        sender: self.identity.node_id(),
                        reason: DisconnectReason::Quit,
                        tree_id: id,
                    };
                    self.send(p.addr, Channel::Tree(id), Frame::Disconnect(f), None);
                    self.maybe_close_session(p.addr);
                    if let Some(op) = old {
                        let f = Disconnect {
                            sender: self.identity.node_id(),
                            reason: DisconnectReason::Quit,
                            tree_id: id,
                        };
                        self.send(op.addr, Channel::Tree(id), Frame::Disconnect(f), None);
                        self.maybe_close_session(op.addr);
                    }
                }
            }
        }
        self.shed_retry_at = Some(self.now + self.params.shed_hysteresis);
        self.check_active();
    }

    fn unshed_attempt(&mut self) {
        let Some(matrix) = self.matrix.as_ref() else {
            return;
        };
        let max_layer = matrix.layer_count.saturating_sub(1);
        let wanted = match &self.cfg.role {
            Role::Viewer { top_layer } => (*top_layer).min(max_layer),
            _ => max_layer,
        };
        if self.top_layer >= wanted {
            self.shed_retry_at = None;
            return;
        }
        self.top_layer += 1;
        let subscribed = matrix.trees_for_layer_prefix(self.top_layer);
        let ids: Vec<TreeId> = self.trees.iter().map(|t| t.id).collect();
        for id in ids {
            let t = &mut self.trees[id.index()];
            t.subscribed = subscribed.contains(id);
            if t.wants_parent() && t.parent.is_none() && !t.join.is_active() {
                self.begin_probing(id);
            }
        }
        self.shed_retry_at = None;
    }

    fn on_accepted(&mut self, from: PeerAddr, a: Accepted) {
        if a.tree_id == TreeId::NONE {
            return; // membership accept (M3)
        }
        let idx = a.tree_id.index();
        if idx >= self.trees.len() {
            return;
        }
        let JoinState::Requesting {
            current,
            current_id,
            current_rtt_ms,
            ..
        } = self.trees[idx].join.clone()
        else {
            return;
        };
        if current != from {
            return;
        }
        let repair = self.trees[idx].repairing;
        let srtt = Duration::from_secs_f64((current_rtt_ms.max(1.0)) / 1000.0);
        let old_parent = self.trees[idx].parent.take();
        self.trees[idx].parent = Some(ParentLink {
            addr: from,
            node_id: current_id,
            parent_depth: a.hop_depth,
            last_rx: self.now,
            pinged: false,
            srtt,
            rttvar: srtt.div_by(2),
            attached_at: self.now,
            first_block_at: None,
            pending: a.pending,
            rejected_blocks: 0,
            attached_as_repair: repair,
        });
        self.trees[idx].depth = a.hop_depth.saturating_add(1);
        self.trees[idx].join = JoinState::Idle;
        self.join_stats[idx].failed_rounds = 0;
        // A warm repair (drain) leaves an old parent: keep taking its blocks until
        // the new parent delivers, then release it (spec step 3).
        if let Some(op) = old_parent {
            if op.addr != from {
                self.trees[idx].old_parent = Some(op);
            }
        }
        self.emit(Event::ParentAttached {
            tree: a.tree_id.0,
            parent: current_id,
            depth: self.trees[idx].depth,
            repair,
        });
        if repair && self.trees[idx].lost_at.is_none() {
            self.trees[idx].repairing = false;
        }
        self.check_active();
        self.reregister_if_needed();
    }

    fn on_rejected(&mut self, from: PeerAddr, d: &Disconnect) {
        let tree = d.tree_id;
        if !tree.is_tree() {
            return;
        }
        let idx = tree.index();
        if idx >= self.trees.len() {
            return;
        }
        let JoinState::Requesting {
            queue,
            current,
            current_id,
            saturated,
            ..
        } = self.trees[idx].join.clone()
        else {
            return;
        };
        if current != from {
            return;
        }
        let mut saturated = saturated;
        match d.reason {
            DisconnectReason::RejectedNotAssigned => {
                if let Some(r) = self.passive.get_mut(&current_id) {
                    r.assigned_trees.remove(tree);
                }
            }
            DisconnectReason::RejectedSaturated | DisconnectReason::RejectedDepth => saturated += 1,
            _ => {}
        }
        self.maybe_close_session(from);
        self.request_next(tree, queue, saturated);
    }

    fn check_active(&mut self) {
        if self.is_source() {
            return;
        }
        let all_subscribed = self
            .trees
            .iter()
            .filter(|t| t.subscribed)
            .all(|t| t.parent.is_some());
        let any_parent = self.trees.iter().any(|t| t.parent.is_some());
        match self.state {
            Lifecycle::Connecting if all_subscribed && any_parent => {
                self.set_state(Lifecycle::Active)
            }
            Lifecycle::Active if !any_parent => {
                self.set_state(Lifecycle::Discovery);
                self.request_discovery(TreeSet::EMPTY);
                if self.matrix.is_some() {
                    self.set_state(Lifecycle::Connecting);
                }
            }
            _ => {}
        }
    }

    // ------------------------------------------------------------------ admission (parent side)

    fn on_neighbor(&mut self, from: PeerAddr, n: Neighbor) {
        if let Some(s) = self.sessions.get_mut(&from) {
            s.node_id.get_or_insert(n.sender);
        }
        if n.tree_id == TreeId::NONE {
            // Membership promotion (HyParView, M3): accept for now.
            let f = Accepted {
                sender: self.node_id(),
                accepted_type: AcceptedType::Neighbor,
                tree_id: TreeId::NONE,
                hop_depth: 0,
                pending: false,
            };
            self.send(from, Channel::Control, Frame::Accepted(f), None);
            return;
        }
        let idx = n.tree_id.index();
        let reject = |reason: DisconnectReason| Disconnect {
            sender: NodeId::ZERO,
            reason,
            tree_id: n.tree_id,
        };
        let verdict: Result<u8, DisconnectReason> =
            if idx >= self.trees.len() || !self.trees[idx].assigned {
                Err(DisconnectReason::RejectedNotAssigned)
            } else {
                let t = &self.trees[idx];
                if !t.depth_allows_children(self.params.d_max) {
                    Err(DisconnectReason::RejectedDepth)
                } else if t.k_avail(self.is_source()) == 0 && !t.children.contains_key(&from) {
                    // ISSUE-061: a relay *assigned* to this tree displaces a pure subscriber
                    // (a child that does not relay the tree) through the drain path, so the
                    // tree can grow interior nodes. Verified against the rendezvous rule, not
                    // the self-declared bit alone. One displacement in progress per tree.
                    let m = self.matrix.as_ref().map(|x| x.m()).unwrap_or(0);
                    let requester_relays_m = n.node_class == NodeClass::Relay
                        && n.assigned_trees.contains(n.tree_id)
                        && rendezvous::assigned_tree(&n.sender, m) == n.tree_id;
                    let draining = t.children.values().any(|c| c.draining_until.is_some());
                    let victim = if requester_relays_m && !draining {
                        t.children
                            .values()
                            .filter(|c| {
                                !c.assigned_trees.contains(n.tree_id)
                                    || c.node_class != NodeClass::Relay
                            })
                            .filter(|c| c.draining_until.is_none())
                            .max_by_key(|c| c.admitted_at)
                            .map(|c| c.addr)
                    } else {
                        None
                    };
                    match victim {
                        Some(v) => {
                            let deadline =
                                SegmentSeq(self.swarm.live_edge.0 + self.params.tau_drain_segments);
                            let d = DrainNotice {
                                tree_id: n.tree_id,
                                reason: DisconnectReason::Displaced,
                                scope: DrainScope::ThisChild,
                                deadline,
                            };
                            self.send(v, Channel::Tree(n.tree_id), Frame::DrainNotice(d), None);
                            if let Some(c) = self.trees[idx].children.get_mut(&v) {
                                c.draining_until = Some(deadline);
                            }
                            Ok(self.trees[idx].depth)
                        }
                        None => Err(DisconnectReason::RejectedSaturated),
                    }
                } else {
                    Ok(t.depth)
                }
            };
        match verdict {
            Ok(depth) => {
                // Handover budget (Ch1 §1.2.2): serving K_v + 1 during a drain is charged to
                // the PULL reserve, which can hold one extra child only if the uplink is at
                // least 10 · B_m · Ω. Below that the handover is sequential: ACCEPTED.PENDING,
                // and pushing starts when the drained child releases its slot.
                let displacing = self.trees[idx]
                    .children
                    .values()
                    .any(|c| c.draining_until.is_some())
                    && self.trees[idx].children.len() as u16 >= self.trees[idx].k_v;
                let b_m = self
                    .matrix
                    .as_ref()
                    .and_then(|m| m.tree(n.tree_id))
                    .map(|t| t.bitrate_kbps as f64)
                    .unwrap_or(0.0);
                let omega = (1.0
                    + self.params.default_parity as f64 / bs_wire::consts::K_BLOCK as f64)
                    * (1.0 + self.params.f_frame);
                let pending = displacing && (self.cfg.upload_kbps as f64) < 10.0 * b_m * omega;
                let t = &mut self.trees[idx];
                t.children.insert(
                    from,
                    ChildLink {
                        addr: from,
                        node_id: n.sender,
                        node_class: n.node_class,
                        assigned_trees: n.assigned_trees,
                        last_tx: self.now,
                        last_rx: self.now,
                        admitted_at: self.now,
                        parity: self.params.default_parity,
                        draining_until: None,
                        pending,
                    },
                );
                let (children, k_v) = (t.children.len(), t.k_v);
                let f = Accepted {
                    sender: self.node_id(),
                    accepted_type: AcceptedType::Neighbor,
                    tree_id: n.tree_id,
                    hop_depth: depth,
                    pending,
                };
                self.send(from, Channel::Control, Frame::Accepted(f), None);
                if !pending {
                    self.send_recent_manifests(from);
                }
                self.emit(Event::ChildAdmitted {
                    tree: n.tree_id.0,
                    child: n.sender,
                    children,
                    k_v,
                });
                self.reregister_if_needed();
            }
            Err(reason) => {
                let mut f = reject(reason);
                f.sender = self.node_id();
                self.send(from, Channel::Control, Frame::Disconnect(f), None);
                self.emit(Event::ChildRejected {
                    tree: n.tree_id.0,
                    child: n.sender,
                    reason: format!("{reason:?}"),
                });
            }
        }
    }

    /// Hand a child the manifests of the segments still in flight so the blocks we
    /// push next verify at once (late joiners backfill the rest with
    /// MANIFEST_REQUEST in M3).
    fn send_recent_manifests(&mut self, to: PeerAddr) {
        self.send_descriptor_to(to);
        let from_seg = SegmentSeq(self.swarm.live_edge.0.saturating_sub(1).max(1));
        for m in self.swarm.manifests_from(from_seg) {
            if self
                .manifests_forwarded
                .insert((m.body.segment.0, m.body.chunk_index, to))
            {
                self.send(to, Channel::Control, Frame::Manifest(m), None);
            }
        }
    }

    /// A drained child left tree `tree`: start serving one pending child, if any.
    fn release_pending(&mut self, tree: TreeId) {
        let idx = tree.index();
        let next = self.trees[idx]
            .children
            .values_mut()
            .find(|c| c.pending)
            .map(|c| {
                c.pending = false;
                c.addr
            });
        if let Some(addr) = next {
            self.send_recent_manifests(addr);
            // A pending joiner keeps any old parent until our first block; the
            // ACCEPTED it holds already carries our depth.
        }
    }

    /// Push the descriptor in force to one peer, once per version (App D §D.4.20).
    fn send_descriptor_to(&mut self, to: PeerAddr) {
        let Some(d) = self.descriptor.clone() else {
            return;
        };
        if self.descriptor_sent.insert((d.version, to)) {
            self.send(to, Channel::Control, Frame::StreamDescriptor(d), None);
        }
    }

    /// Publisher: install a new descriptor and push it down every tree.
    fn set_descriptor(&mut self, d: StreamDescriptor) {
        let Some(p) = self.publisher.as_mut() else {
            return;
        };
        let signed = p.set_descriptor(d);
        self.descriptor = Some(signed);
        let children: HashSet<PeerAddr> = self
            .trees
            .iter()
            .flat_map(|t| t.children.values().filter(|c| !c.pending).map(|c| c.addr))
            .collect();
        for c in children {
            self.send_descriptor_to(c);
        }
        self.store_record();
    }

    /// A descriptor arrived: verify against the pinned publisher key, keep the
    /// newest version, forward to children, and hand it to the application once.
    fn on_descriptor(&mut self, _from: PeerAddr, d: StreamDescriptor) {
        let Some(pk) = self.publisher_key else { return };
        if d.stream_id != self.stream_id()
            || bs_crypto::Verifier::verify(&pk, &d.signable_bytes(), &d.signature).is_err()
        {
            self.emit(Event::Note {
                text: "stream descriptor rejected (signature/stream id)".into(),
            });
            return;
        }
        if self
            .descriptor
            .as_ref()
            .map(|c| c.version >= d.version)
            .unwrap_or(false)
        {
            return;
        }
        self.descriptor = Some(d.clone());
        let children: HashSet<PeerAddr> = self
            .trees
            .iter()
            .flat_map(|t| t.children.values().filter(|c| !c.pending).map(|c| c.addr))
            .collect();
        for c in children {
            self.send_descriptor_to(c);
        }
        self.out.push_back(Output::Descriptor(d));
    }

    /// Serve manifests or the descriptor from the retained window (App D §D.4.16).
    fn on_manifest_request(&mut self, from: PeerAddr, r: ManifestRequest) {
        match r.selector {
            ManifestSelector::Descriptor => {
                if let Some(d) = self.descriptor.clone() {
                    self.send(from, Channel::Control, Frame::StreamDescriptor(d), None);
                }
            }
            ManifestSelector::Chunk(c) => {
                if let Some(m) = self
                    .swarm
                    .manifests_from(r.segment)
                    .into_iter()
                    .find(|m| m.body.segment == r.segment && m.body.chunk_index == c)
                {
                    self.send(from, Channel::Control, Frame::Manifest(m), None);
                }
            }
            ManifestSelector::AllChunks => {
                for m in self
                    .swarm
                    .manifests_from(r.segment)
                    .into_iter()
                    .filter(|m| m.body.segment == r.segment)
                {
                    self.send(from, Channel::Control, Frame::Manifest(m), None);
                }
            }
            ManifestSelector::PendingUpdate => {}
        }
    }

    fn remove_child(&mut self, addr: PeerAddr, reason: &str) {
        for t in 0..self.trees.len() {
            if let Some(c) = self.trees[t].children.remove(&addr) {
                let tree = self.trees[t].id;
                self.emit(Event::ChildRemoved {
                    tree: tree.0,
                    child: c.node_id,
                    reason: reason.into(),
                });
                if c.draining_until.is_some() {
                    self.release_pending(tree);
                }
            }
        }
        self.reregister_if_needed();
    }

    fn reregister_if_needed(&mut self) {
        // Registration carries class and assigned trees only; K_avail is probed.
        // Re-register when we first become able to serve (warm-up ended) so the
        // oracle/DHT can return us.
        let rec = self.peer_record();
        self.out.push_back(Output::Register(rec));
    }

    // ------------------------------------------------------------------ sessions

    fn open_session(&mut self, addr: PeerAddr) {
        let s = self.sessions.entry(addr).or_insert(Session {
            node_id: None,
            open: false,
            last_rx: self.now,
            ping_sent_at: None,
        });
        if !s.open {
            self.out.push_back(Output::OpenSession(addr));
        }
    }

    fn on_session_opened(&mut self, peer: PeerAddr, node_id: NodeId) {
        let s = self.sessions.entry(peer).or_insert(Session {
            node_id: None,
            open: false,
            last_rx: self.now,
            ping_sent_at: None,
        });
        s.open = true;
        s.node_id = Some(node_id);
        s.last_rx = self.now;
        let ids: Vec<TreeId> = self.trees.iter().map(|t| t.id).collect();
        for id in ids {
            if let JoinState::Requesting {
                current,
                awaiting_session,
                ..
            } = &mut self.trees[id.index()].join
            {
                if *current == peer && *awaiting_session {
                    *awaiting_session = false;
                    self.send_neighbor(id, peer);
                }
            }
        }
    }

    fn on_session_closed(&mut self, peer: PeerAddr, reason: &str) {
        self.sessions.remove(&peer);
        self.remove_child(peer, reason);
        let ids: Vec<TreeId> = self.trees.iter().map(|t| t.id).collect();
        for id in ids {
            let idx = id.index();
            if self.trees[idx].parent.as_ref().map(|p| p.addr) == Some(peer) {
                self.lose_parent(id, reason);
            }
            if self.trees[idx].old_parent.as_ref().map(|p| p.addr) == Some(peer) {
                self.trees[idx].old_parent = None;
            }
            if let JoinState::Requesting {
                current,
                queue,
                saturated,
                ..
            } = self.trees[idx].join.clone()
            {
                if current == peer {
                    self.request_next(id, queue, saturated);
                }
            }
        }
        self.check_active();
    }

    fn maybe_close_session(&mut self, addr: PeerAddr) {
        let used = self.trees.iter().any(|t| {
            t.parent.as_ref().map(|p| p.addr) == Some(addr)
                || t.old_parent.as_ref().map(|p| p.addr) == Some(addr)
                || t.children.contains_key(&addr)
                || matches!(&t.join, JoinState::Requesting { current, .. } if *current == addr)
        });
        if !used && self.sessions.remove(&addr).is_some() {
            self.out.push_back(Output::CloseSession(addr));
        }
    }

    fn lose_parent(&mut self, tree: TreeId, reason: &str) {
        let idx = tree.index();
        let Some(p) = self.trees[idx].detach_parent(self.now) else {
            return;
        };
        self.emit(Event::ParentLost {
            tree: tree.0,
            parent: p.node_id,
            reason: reason.into(),
        });
        self.maybe_close_session(p.addr);
        if self.ending {
            return; // the stream is over; nothing to repair
        }
        self.trees[idx].repairing = true;
        if self.trees[idx].wants_parent() {
            self.begin_probing(tree);
        }
        // Children of a tree whose parent we lost keep being served from whatever
        // we still hold; depth is unknown until re-attached, so we stop admitting.
    }

    // ------------------------------------------------------------------ frames

    fn on_frame(&mut self, from: PeerAddr, channel: Channel, frame: Frame) {
        if let Some(s) = self.sessions.get_mut(&from) {
            s.last_rx = self.now;
        }
        // Any packet from a parent counts as liveness (Ch3 §3.3.2).
        for t in &mut self.trees {
            if let Some(p) = t.parent.as_mut() {
                if p.addr == from {
                    p.last_rx = self.now;
                    p.pinged = false;
                }
            }
            if let Some(p) = t.old_parent.as_mut() {
                if p.addr == from {
                    p.last_rx = self.now;
                }
            }
            if let Some(c) = t.children.get_mut(&from) {
                c.last_rx = self.now;
            }
        }
        match frame {
            Frame::Ping(p) => self.on_ping(from, p),
            Frame::Pong(p) => self.on_pong(from, p),
            Frame::Probe(p) => self.on_probe(from, p),
            Frame::ProbeResponse(pr) => self.on_probe_response(from, pr),
            Frame::Neighbor(n) => self.on_neighbor(from, n),
            Frame::Accepted(a) => self.on_accepted(from, a),
            Frame::Disconnect(d) => {
                if d.reason.is_rejection() {
                    self.on_rejected(from, &d);
                } else {
                    // App D §D.4.3b: TreeID 0 ends the whole connection, m > 0 only tree m.
                    let scope = if d.tree_id.is_tree() {
                        Some(d.tree_id)
                    } else {
                        None
                    };
                    self.on_disconnect(from, d, scope);
                }
            }
            Frame::DrainNotice(d) => self.on_drain(from, d),
            Frame::Manifest(m) => self.on_manifest(from, m),
            Frame::BlockProof(bp) => self.on_block_proof(from, channel, bp),
            Frame::RaptorQSymbol(s) => self.on_symbol(from, s),
            Frame::StreamEnd(e) => self.on_stream_end(from, e),
            Frame::StreamDescriptor(d) => self.on_descriptor(from, d),
            Frame::ManifestRequest(r) => self.on_manifest_request(from, r),
            Frame::Join(_)
            | Frame::ChokeState(_)
            | Frame::PullRequest(_)
            | Frame::ManifestUpdate(_)
            | Frame::BlockTransmission(_) => {
                // M3.
            }
            Frame::RegisterPeer(_) | Frame::GetPeers(_) => {
                // Discovery frames are answered by the guardian in the driver (M2) or the
                // DHT module (M3), never by the peer core.
            }
            Frame::Raw { .. } => {}
        }
    }

    fn validation_block(&self, ty: FrameType, body: &[u8]) -> ValidationBlock {
        self.identity
            .validation_block(ty, self.dyn_nonce, self.now.as_micros(), body)
    }

    fn verify_vb(
        &self,
        vb: &ValidationBlock,
        ty: FrameType,
        body: &[u8],
        from: PeerAddr,
        class: NodeClass,
    ) -> bool {
        let d = Difficulty {
            c1: self.params.c1,
            c2_min: self.params.c2_min,
        };
        let c2 = d.dynamic_accepted(self.swarm_size.max(2), class, false);
        Identity::verify_validation_block(vb, ty, body, from.ip(), d, c2).is_ok()
    }

    fn on_ping(&mut self, from: PeerAddr, p: Ping) {
        if !self.verify_vb(
            &p.validation,
            FrameType::PING,
            p.stream_id.as_bytes(),
            from,
            NodeClass::Leaf,
        ) {
            return;
        }
        let reflected = WireAddr(from);
        let vb = self.validation_block(FrameType::PONG, &reflected.to_vec());
        self.send(
            from,
            Channel::Udp,
            Frame::Pong(Pong {
                validation: vb,
                reflected,
            }),
            None,
        );
    }

    fn on_pong(&mut self, from: PeerAddr, _p: Pong) {
        if let Some(s) = self.sessions.get_mut(&from) {
            if let Some(t0) = s.ping_sent_at.take() {
                let sample = self.now.duration_since(t0);
                for t in &mut self.trees {
                    if let Some(p) = t.parent.as_mut() {
                        if p.addr == from {
                            // RFC 6298 style smoothing.
                            let diff = if sample > p.srtt {
                                sample - p.srtt
                            } else {
                                p.srtt - sample
                            };
                            p.rttvar = Duration((p.rttvar.0 * 3 + diff.0) / 4);
                            p.srtt = Duration((p.srtt.0 * 7 + sample.0) / 8);
                        }
                    }
                }
            }
        }
    }

    fn on_probe(&mut self, from: PeerAddr, p: Probe) {
        let mut body = Vec::with_capacity(33);
        body.extend_from_slice(p.stream_id.as_bytes());
        body.push(p.tree_id.0);
        if !self.verify_vb(
            &p.validation,
            FrameType::PROBE,
            &body,
            from,
            NodeClass::Leaf,
        ) {
            return;
        }
        if p.stream_id != self.stream_id() {
            return;
        }
        let idx = p.tree_id.index();
        let (k_avail, hop, state, live_edge) = match self.trees.get(idx) {
            Some(t) => (
                t.k_avail(self.is_source()),
                t.advertised_depth(),
                t.probe_state(self.is_source()),
                t.live_edge,
            ),
            None => (0, u8::MAX, bs_wire::TreeState::Unparented, SegmentSeq::NONE),
        };
        let rec = self.peer_record();
        let f = ProbeResponse {
            sender: self.node_id(),
            k_avail,
            reliability: ProbeResponse::reliability_from_f32(1.0),
            hop_count: hop,
            node_class: self.cfg.node_class,
            flags: rec.flags,
            tree_state: state,
            assigned_trees: rec.assigned_trees,
            live_edge,
        };
        self.send(from, Channel::Udp, Frame::ProbeResponse(f), None);
    }

    fn on_disconnect(&mut self, from: PeerAddr, d: Disconnect, scope: Option<TreeId>) {
        // From a parent: we were released or evicted. From a child: it left.
        let reason = format!("disconnect:{:?}", d.reason);
        let ids: Vec<TreeId> = self.trees.iter().map(|t| t.id).collect();
        for id in ids {
            if scope.is_some_and(|s| s != id) {
                continue;
            }
            if self.trees[id.index()].parent.as_ref().map(|p| p.addr) == Some(from) {
                self.lose_parent(id, &reason);
            }
            if self.trees[id.index()].old_parent.as_ref().map(|p| p.addr) == Some(from) {
                self.trees[id.index()].old_parent = None;
            }
            if let Some(c) = self.trees[id.index()].children.remove(&from) {
                self.emit(Event::ChildRemoved {
                    tree: id.0,
                    child: c.node_id,
                    reason: reason.clone(),
                });
                if c.draining_until.is_some() {
                    self.release_pending(id);
                }
            }
        }
        self.maybe_close_session(from);
        self.check_active();
    }

    fn on_drain(&mut self, from: PeerAddr, d: DrainNotice) {
        // Warm repair: keep the parent, look for a new one now (Ch1 §1.2.2 The Drain Path).
        let idx = d.tree_id.index();
        if idx >= self.trees.len() {
            return;
        }
        if self.trees[idx].parent.as_ref().map(|p| p.addr) != Some(from) {
            return;
        }
        if !self.trees[idx].join.is_active() {
            self.trees[idx].repairing = true;
            self.begin_probing(d.tree_id);
        }
    }

    fn on_stream_end(&mut self, _from: PeerAddr, e: StreamEnd) {
        let Some(pk) = self.publisher_key else { return };
        if bs_crypto::Verifier::verify(&pk, &e.signable_bytes(), &e.signature).is_err() {
            return;
        }
        let children: Vec<PeerAddr> = self
            .trees
            .iter()
            .flat_map(|t| t.children.keys().copied())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        for c in children {
            self.send(c, Channel::Control, Frame::StreamEnd(e.clone()), None);
        }
        // The overlay role ends now; playback finishes what is buffered. Ch1 §1.3.1 says
        // TERMINATED "immediately", read here as "stop relaying": a viewer that discards
        // 3 s of verified video would lose the end of every stream.
        self.ending = true;
        if self.swarm.next_deadline().is_none() {
            self.set_state(Lifecycle::Terminated);
        }
    }

    fn on_manifest(&mut self, from: PeerAddr, m: Manifest) {
        let (Some(pk), Some(matrix)) = (self.publisher_key, self.matrix.clone()) else {
            return;
        };
        let verdict = self.swarm.accept_manifest(&m, &pk, &matrix, self.now);
        match verdict {
            ManifestVerdict::Accepted => {
                self.emit(Event::ManifestAccepted {
                    segment: m.body.segment.0,
                    chunk: m.body.chunk_index,
                    blocks: m.body.block_count(),
                });
                self.forward_manifest(&m, Some(from));
                // Blocks that were waiting for this manifest may now be verifiable.
                let n = m.body.block_count();
                for j in 0..n {
                    let idx = bs_wire::BlockIndex::new(m.body.chunk_index, j).unwrap();
                    self.try_verify(m.body.segment, idx, None);
                }
            }
            ManifestVerdict::Duplicate => {}
            other => self.emit(Event::ManifestRejected {
                segment: m.body.segment.0,
                chunk: m.body.chunk_index,
                reason: format!("{other:?}"),
            }),
        }
    }

    fn forward_manifest(&mut self, m: &Manifest, except: Option<PeerAddr>) {
        let children: HashSet<PeerAddr> = self
            .trees
            .iter()
            .flat_map(|t| t.children.values().filter(|c| !c.pending).map(|c| c.addr))
            .collect();
        let key = (m.body.segment.0, m.body.chunk_index);
        for c in children {
            if Some(c) == except {
                continue;
            }
            if self.manifests_forwarded.insert((key.0, key.1, c)) {
                self.send(c, Channel::Control, Frame::Manifest(m.clone()), None);
            }
        }
        if self.manifests_forwarded.len() > 4096 {
            let floor = self.swarm.live_edge.0.saturating_sub(16);
            self.manifests_forwarded.retain(|(s, _, _)| *s >= floor);
        }
    }

    fn on_block_proof(&mut self, from: PeerAddr, channel: Channel, bp: BlockProof) {
        let Channel::Tree(tree) = channel else { return };
        let idx = tree.index();
        if idx >= self.trees.len() {
            return;
        }
        // Only the tree's parent (or the one it is draining from) may push on the tree stream.
        let is_current = self.trees[idx].parent.as_ref().map(|p| p.addr) == Some(from);
        let is_old = self.trees[idx].old_parent.as_ref().map(|p| p.addr) == Some(from);
        if !is_current && !is_old {
            return;
        }
        let sender = if is_current {
            // Depth propagation (Ch1 §1.2.2): h_self = SenderHopDepth + 1 on every block.
            let t = &mut self.trees[idx];
            t.depth = bp.sender_hop_depth.saturating_add(1);
            let p = t.parent.as_mut().unwrap();
            p.parent_depth = bp.sender_hop_depth;
            let id = p.node_id;
            if p.first_block_at.is_none() {
                // "When the new parent begins delivering" (Ch1 §1.2.2 The Drain Path, step 3):
                // release the old parent and close the repair.
                p.first_block_at = Some(self.now);
                let was_repair = p.attached_as_repair;
                let old = t.old_parent.take();
                let mut done: Option<Duration> = None;
                if t.repairing && was_repair {
                    t.repairing = false;
                    done = t.lost_at.take().map(|l| self.now.duration_since(l));
                }
                if let Some(op) = old {
                    let f = Disconnect {
                        sender: self.identity.node_id(),
                        reason: DisconnectReason::Choke,
                        tree_id: tree,
                    };
                    self.send(op.addr, Channel::Tree(tree), Frame::Disconnect(f), None);
                    self.maybe_close_session(op.addr);
                }
                if let Some(d) = done {
                    self.emit(Event::RepairDone {
                        tree: tree.0,
                        duration_us: d.as_micros(),
                    });
                }
            }
            id
        } else {
            self.trees[idx].old_parent.as_ref().unwrap().node_id
        };
        let ready = self.swarm.add_proof(
            bp.segment,
            bp.block,
            bp.siblings,
            bp.sender_hop_depth,
            sender,
            self.now,
        );
        if ready {
            self.try_verify(bp.segment, bp.block, Some(from));
        }
    }

    fn on_symbol(&mut self, from: PeerAddr, s: RaptorQSymbol) {
        if self.swarm.is_verified(s.segment, s.block) {
            return;
        }
        let ready = self
            .swarm
            .add_symbol(s.segment, s.block, s.esi, s.payload, self.now);
        if ready {
            self.try_verify(s.segment, s.block, Some(from));
        }
    }

    fn try_verify(&mut self, seg: SegmentSeq, idx: bs_wire::BlockIndex, from: Option<PeerAddr>) {
        match self.swarm.try_verify(seg, idx) {
            Ok(Some(vb)) => self.on_verified(vb, from),
            Ok(None) => {}
            Err((_e, who)) => {
                let tree = self
                    .swarm
                    .chunk(seg, idx.chunk())
                    .and_then(|c| c.tree_of.get(idx.j() as usize).copied())
                    .unwrap_or(TreeId::NONE);
                self.emit(Event::BlockRejected {
                    tree: tree.0,
                    segment: seg.0,
                    block: idx.0,
                    from: who.unwrap_or(NodeId::ZERO),
                });
                // Ch4 §4.1.2: the delivering parent is flagged. Three failed blocks from
                // one parent and it is evicted from this tree and excluded from future
                // candidate lists (the audit gossip of Ch5 §5.3 arrives in M3).
                if let Some(bad) = who {
                    if tree.is_tree() && tree.index() < self.trees.len() {
                        let idx_t = tree.index();
                        let evict = match self.trees[idx_t].parent.as_mut() {
                            Some(p) if p.node_id == bad => {
                                p.rejected_blocks += 1;
                                p.rejected_blocks >= 3
                            }
                            _ => false,
                        };
                        if evict {
                            self.bad_peers.insert(bad);
                            self.lose_parent(tree, "poisoned blocks");
                        }
                    }
                }
            }
        }
    }

    fn on_verified(&mut self, vb: VerifiedBlock, _from: Option<PeerAddr>) {
        let idx = vb.tree.index();
        if idx < self.trees.len() && vb.segment > self.trees[idx].last_segment_seen {
            self.trees[idx].last_segment_seen = vb.segment;
        }
        let forwarded = self.forward_block(&vb);
        let depth = self.trees.get(idx).map(|t| t.depth).unwrap_or(0);
        self.emit(Event::BlockVerified {
            tree: vb.tree.0,
            segment: vb.segment.0,
            block: vb.index.0,
            depth,
            decoded: vb.decoded,
            forwarded_to: forwarded,
        });
        // Warm-up accounting: a segment counts once every block of this tree in all
        // four chunks is verified (Ch1 §1.2.2 warm-up gating).
        if idx < self.trees.len()
            && vb.segment > self.trees[idx].live_edge
            && self.swarm.segment_complete_for_tree(vb.segment, vb.tree)
        {
            let t = &mut self.trees[idx];
            t.live_edge = vb.segment;
            t.verified_segments += 1;
            if t.verified_segments == 1 && t.assigned {
                self.reregister_if_needed();
            }
        }
    }

    /// Verify-then-forward: BLOCK_PROOF on the tree stream, then symbols as datagrams.
    fn forward_block(&mut self, vb: &VerifiedBlock) -> usize {
        let idx = vb.tree.index();
        if idx >= self.trees.len() {
            return 0;
        }
        let depth = self.trees[idx].depth;
        let children: Vec<(PeerAddr, u16)> = self.trees[idx]
            .children
            .values()
            .filter(|c| !c.pending)
            .map(|c| (c.addr, c.parity))
            .collect();
        if children.is_empty() {
            return 0;
        }
        let enc = FecEncoder::new(vb.segment, vb.index, vb.bytes.clone());
        let proof = BlockProof {
            segment: vb.segment,
            block: vb.index,
            sender_hop_depth: depth,
            siblings: vb.siblings.clone(),
        };
        let not_before = self.pacing_slot(vb);
        for (addr, parity) in &children {
            self.send(
                *addr,
                Channel::Tree(vb.tree),
                Frame::BlockProof(proof.clone()),
                not_before,
            );
            for s in enc.push_symbols(*parity) {
                self.send(
                    *addr,
                    Channel::Datagram,
                    Frame::RaptorQSymbol(s),
                    not_before,
                );
            }
        }
        children.len()
    }

    /// Source pacing (Ch3 §3.3.2): spread a chunk's blocks over ≤ ½ chunk period.
    /// Relays forward immediately.
    fn pacing_slot(&self, vb: &VerifiedBlock) -> Option<Instant> {
        if !self.is_source() {
            return None;
        }
        let n = self
            .swarm
            .chunk(vb.segment, vb.index.chunk())
            .map(|c| c.blocks.len())
            .unwrap_or(1)
            .max(1) as u64;
        let window = self
            .params
            .chunk_period
            .mul_f64(self.params.source_pacing_fraction);
        Some(self.now + Duration(window.0 * vb.index.j() as u64 / n))
    }

    // ------------------------------------------------------------------ publisher

    fn publish_chunk(&mut self, chunk: LayeredChunk) {
        let Some(matrix) = self.matrix.clone() else {
            return;
        };
        let built = match self.publisher.as_mut().map(|p| p.build(&chunk)) {
            Some(Ok(b)) => b,
            _ => return,
        };
        self.swarm.insert_built(&built, &matrix, self.now);
        self.emit(Event::ManifestAccepted {
            segment: chunk.segment.0,
            chunk: chunk.chunk_index,
            blocks: built.block_count(),
        });
        self.forward_manifest(&built.manifest, None);
        for j in 0..built.block_count() {
            let idx = bs_wire::BlockIndex::new(chunk.chunk_index, j).unwrap();
            let tree = matrix
                .tree_for_block(&built.manifest.body, j)
                .unwrap_or(TreeId::BASE);
            let (layer, _) = built.manifest.body.layer_of(j).unwrap_or((0, 0));
            let vb = VerifiedBlock {
                segment: chunk.segment,
                index: idx,
                tree,
                layer,
                bytes: built.blocks[j as usize].clone(),
                siblings: built.tree.proof(j as usize),
                sender_depth: 0,
                decoded: false,
            };
            let n = self.forward_block(&vb);
            self.emit(Event::BlockVerified {
                tree: tree.0,
                segment: chunk.segment.0,
                block: idx.0,
                depth: 0,
                decoded: false,
                forwarded_to: n,
            });
        }
        // Track live edge per tree for probes.
        for t in &mut self.trees {
            t.live_edge = chunk.segment;
            t.verified_segments += 1;
        }
        if chunk.chunk_index == 0 {
            self.store_record();
        }
    }

    fn end_stream(&mut self) {
        let Some(p) = self.publisher.as_ref() else {
            return;
        };
        let mut e = StreamEnd {
            stream_id: p.stream_id(),
            final_segment: p.last_segment,
            timestamp_us: self.now.as_micros(),
            signature: bs_wire::SignatureBytes::ZERO,
        };
        e.signature = self.identity.sign(&e.signable_bytes());
        let children: HashSet<PeerAddr> = self
            .trees
            .iter()
            .flat_map(|t| t.children.keys().copied())
            .collect();
        for c in children {
            self.send(c, Channel::Control, Frame::StreamEnd(e.clone()), None);
        }
        self.set_state(Lifecycle::Terminated);
    }

    fn quit(&mut self) {
        let peers: HashSet<PeerAddr> = self
            .trees
            .iter()
            .flat_map(|t| {
                t.children
                    .keys()
                    .copied()
                    .chain(t.parent.as_ref().map(|p| p.addr))
            })
            .collect();
        for p in peers {
            let f = Disconnect {
                sender: self.node_id(),
                reason: DisconnectReason::Quit,
                tree_id: TreeId::NONE,
            };
            self.send(p, Channel::Control, Frame::Disconnect(f), None);
            self.out.push_back(Output::CloseSession(p));
        }
        self.set_state(Lifecycle::Terminated);
    }

    // ------------------------------------------------------------------ timers

    fn run_timers(&mut self) {
        let now = self.now;
        // Join deadlines.
        let ids: Vec<TreeId> = self.trees.iter().map(|t| t.id).collect();
        for id in ids {
            let idx = id.index();
            match self.trees[idx].join.clone() {
                JoinState::Probing { deadline, .. } if now >= deadline => self.finish_probing(id),
                JoinState::Requesting {
                    deadline,
                    queue,
                    saturated,
                    current,
                    ..
                } if now >= deadline => {
                    self.maybe_close_session(current);
                    self.request_next(id, queue, saturated);
                }
                JoinState::Backoff { until } if now >= until => {
                    let t = &self.trees[idx];
                    if t.wants_parent() && (t.parent.is_none() || t.repairing) {
                        self.begin_probing(id);
                    } else {
                        self.trees[idx].join = JoinState::Idle;
                    }
                }
                JoinState::Idle
                    if !self.is_source()
                        && self.trees[idx].wants_parent()
                        && (self.trees[idx].parent.is_none() || self.trees[idx].repairing)
                        && self.matrix.is_some()
                        && self.state != Lifecycle::Discovery =>
                {
                    self.begin_probing(id);
                }
                _ => {}
            }
        }
        if let Some(t) = self.shed_retry_at {
            if now >= t {
                self.unshed_attempt();
            }
        }
        // Liveness (Ch3 §3.3): heartbeat on idle, RTT-scaled eviction.
        if now >= self.next_liveness {
            self.next_liveness = now + self.params.tau_ping.div_by(4);
            self.liveness();
        }
        // Playout.
        if self
            .swarm
            .next_deadline()
            .map(|d| d <= now)
            .unwrap_or(false)
        {
            let played = self.swarm.play_due(now);
            let subscribed = self.top_layer + 1;
            for p in played {
                let ok = p.layers_complete >= 1;
                let (seg, c) = (p.chunk.segment.0, p.chunk.chunk_index);
                self.emit(Event::ChunkPlayed {
                    segment: seg,
                    chunk: c,
                    layers_complete: p.layers_complete,
                    layers_subscribed: subscribed.min(p.layers_total),
                    ok,
                    missing_blocks: p.missing,
                });
                if ok && !self.first_ok_chunk {
                    self.first_ok_chunk = true;
                    self.emit(Event::FirstChunk {
                        segment: seg,
                        chunk: c,
                    });
                }
                self.out.push_back(Output::Deliver {
                    chunk: p.chunk,
                    layers_complete: p.layers_complete,
                    layers_subscribed: subscribed.min(p.layers_total),
                });
            }
            self.swarm.gc(now);
        }
        if self.ending
            && self.swarm.next_deadline().is_none()
            && self.state != Lifecycle::Terminated
        {
            self.set_state(Lifecycle::Terminated);
        }
    }

    fn liveness(&mut self) {
        let now = self.now;
        let tau_ping = self.params.tau_ping;
        // Parents.
        let mut to_ping: Vec<PeerAddr> = Vec::new();
        let mut to_evict: Vec<TreeId> = Vec::new();
        for t in &mut self.trees {
            let Some(p) = t.parent.as_mut() else { continue };
            let silence = now.duration_since(p.last_rx);
            let tau_evict = self.params.tau_evict(p.srtt, p.rttvar);
            if silence > tau_evict {
                to_evict.push(t.id);
            } else if silence > tau_ping && !p.pinged {
                p.pinged = true;
                to_ping.push(p.addr);
            }
        }
        for addr in to_ping {
            self.send_ping(addr);
        }
        for id in to_evict {
            self.lose_parent(id, "timeout");
        }
        // Drains past their deadline (Ch1 §1.2.2 The Drain Path): DISCONNECT(reason).
        let live = self.swarm.live_edge;
        let mut expired: Vec<(PeerAddr, TreeId, NodeId)> = Vec::new();
        for t in &self.trees {
            for c in t.children.values() {
                if let Some(d) = c.draining_until {
                    if live >= d {
                        expired.push((c.addr, t.id, c.node_id));
                    }
                }
            }
        }
        for (addr, tree, id) in expired {
            self.trees[tree.index()].children.remove(&addr);
            self.release_pending(tree);
            // Sent on the tree's stream: scoped to this tree, the session stays up.
            let f = Disconnect {
                sender: self.identity.node_id(),
                reason: DisconnectReason::Displaced,
                tree_id: tree,
            };
            self.send(addr, Channel::Tree(tree), Frame::Disconnect(f), None);
            self.emit(Event::ChildRemoved {
                tree: tree.0,
                child: id,
                reason: "drain deadline".into(),
            });
            self.maybe_close_session(addr);
        }
        // Children: heartbeat when idle.
        let mut hb: Vec<(TreeId, PeerAddr, NodeId)> = Vec::new();
        for t in &mut self.trees {
            for c in t.children.values_mut() {
                if now.duration_since(c.last_tx) >= tau_ping {
                    c.last_tx = now;
                    hb.push((t.id, c.addr, c.node_id));
                }
            }
        }
        let mut sent: HashSet<PeerAddr> = HashSet::new();
        for (tree, addr, id) in hb {
            if sent.insert(addr) {
                self.send_ping(addr);
                self.emit(Event::Heartbeat {
                    tree: tree.0,
                    to: id,
                });
            }
        }
        self.check_active();
    }

    fn send_ping(&mut self, addr: PeerAddr) {
        let sid = self.stream_id();
        let vb = self.validation_block(FrameType::PING, sid.as_bytes());
        if let Some(s) = self.sessions.get_mut(&addr) {
            s.ping_sent_at = Some(self.now);
        }
        self.send(
            addr,
            Channel::Udp,
            Frame::Ping(Ping {
                validation: vb,
                stream_id: sid,
            }),
            None,
        );
    }

    fn send(&mut self, to: PeerAddr, channel: Channel, frame: Frame, not_before: Option<Instant>) {
        // Track last_tx toward children for the heartbeat rule.
        for t in &mut self.trees {
            if let Some(c) = t.children.get_mut(&to) {
                c.last_tx = not_before.unwrap_or(self.now).max(c.last_tx);
            }
        }
        self.out.push_back(Output::Send {
            to,
            channel,
            frame,
            not_before,
        });
    }
}
