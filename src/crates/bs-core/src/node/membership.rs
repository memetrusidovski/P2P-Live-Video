//! HyParView membership (Ch3 §3.1–3.2): the Active Set of gossip sessions, the
//! Passive Set of standby Peer Records indexed by tree, the JOIN / FORWARD_JOIN
//! random walk, SHUFFLE walks, and HMAC-authenticated GOSSIP_EXCHANGE.

use std::collections::{BTreeMap, BTreeSet};

use bs_wire::frames::{
    Accepted, AcceptedType, ForwardJoin, GossipExchange, Join, Neighbor, Priority, Shuffle,
};
use bs_wire::{
    Frame, FrameType, Hash, NodeClass, NodeId, PeerFlags, PeerRecord, TreeId, TreeSet, WireAddr,
};
use rand::seq::SliceRandom;
use rand::Rng;

use super::Node;
use crate::io::{Channel, Event, PeerAddr};
use crate::time::{Duration, Instant};
use crate::Lifecycle;

/// One Active Set member.
#[derive(Debug, Clone)]
pub struct ActivePeer {
    /// Identity.
    pub node_id: NodeId,
    /// What we know about it (class and trees may be unknown until a record arrives).
    pub record: PeerRecord,
    /// Since when.
    pub since: Instant,
    /// Highest GOSSIP_EXCHANGE sequence accepted from it.
    pub seq_in_max: u32,
    /// Trees it last told us it wants relays for.
    pub wants: TreeSet,
}

/// Membership state.
#[derive(Debug, Default)]
pub struct MembershipState {
    /// Active Set: peers we gossip with over open sessions.
    pub active: BTreeMap<PeerAddr, ActivePeer>,
    /// Contacts we sent JOIN to and are waiting on.
    pub join_contacts: BTreeSet<PeerAddr>,
    /// Joiners that sent us JOIN and whose session is not open yet.
    pub pending_joiners: BTreeMap<PeerAddr, Join>,
    /// Peers we are opening a session to in order to send NEIGHBOR(TreeID = 0).
    pub pending_neighbor: BTreeSet<PeerAddr>,
    /// When JOINING began.
    pub joining_since: Option<Instant>,
    /// Next GOSSIP_EXCHANGE round.
    pub next_gossip: Instant,
    /// Next SHUFFLE walk.
    pub next_shuffle: Instant,
    /// Last GET_PEERS re-query per tree.
    pub last_pool_query: BTreeMap<u8, Instant>,
    /// Outgoing GOSSIP_EXCHANGE sequence.
    pub seq_out: u32,
    /// Counters.
    pub forward_joins: u64,
    /// Shuffles handled.
    pub shuffles: u64,
    /// Exchanges accepted.
    pub exchanges: u64,
    /// Exchanges dropped (HMAC or replay).
    pub exchanges_rejected: u64,
}

/// Session key for gossip HMAC: `Blake3(key ‖ bytes)` keyed mode.
pub fn gossip_hmac(key: &[u8; 32], bytes: &[u8]) -> Hash {
    Hash(*blake3::keyed_hash(key, bytes).as_bytes())
}

impl Node {
    // ------------------------------------------------------------------ sizing

    /// `c_a_eff = min(64, max(c_a, ⌊K_v/10⌋))` over the node's largest tree slot count.
    pub(super) fn active_target(&self) -> usize {
        let k_v = self.trees.iter().map(|t| t.k_v as usize).max().unwrap_or(0);
        (self.params.c_a.max(k_v / 10)).min(64)
    }

    /// Per-tree candidate pool `P_m`: passive records that relay `m` and are reachable.
    pub(super) fn pool(&self, tree: TreeId) -> Vec<PeerRecord> {
        self.passive
            .values()
            .filter(|r| r.relays(tree) && r.flags.reachability != bs_wire::Reachability::Symmetric)
            .copied()
            .collect()
    }

    /// Trees whose pool is below the floor of 3.
    pub(super) fn wanted_trees(&self) -> TreeSet {
        let mut w = TreeSet::EMPTY;
        for t in &self.trees {
            if t.wants_parent() && self.pool(t.id).len() < 3 {
                w.insert(t.id);
            }
        }
        w
    }

    /// Insert a record into the Passive Set, evicting a random one past `c_p`.
    pub(super) fn passive_insert(&mut self, r: PeerRecord) {
        if r.node_id == self.node_id() {
            return;
        }
        // RULE: An ingress relay stands in for a source behind a NAT (Ch6 §6.3.3).
        //       A reachable source record is never replaced by an unreachable one.
        if r.flags.source_ingress
            || (r.flags.source
                && (r.flags.reachability != bs_wire::Reachability::Symmetric
                    || self.source_record.is_none()))
        {
            self.source_record = Some(r);
        }
        let known_active = self
            .membership
            .active
            .values()
            .any(|a| a.node_id == r.node_id);
        self.passive.insert(r.node_id, r);
        if !known_active && self.passive.len() > self.params.c_p {
            let keys: Vec<NodeId> = self.passive.keys().copied().collect();
            // Never evict the source record or a current parent's record.
            let protected: BTreeSet<NodeId> = self
                .trees
                .iter()
                .filter_map(|t| t.parent.as_ref().map(|p| p.node_id))
                .chain(self.source_record.map(|s| s.node_id))
                .collect();
            let victims: Vec<NodeId> = keys
                .into_iter()
                .filter(|k| !protected.contains(k))
                .collect();
            if !victims.is_empty() {
                let i = self.rng.random_range(0..victims.len());
                self.passive.remove(&victims[i]);
            }
        }
    }

    /// Our own record for gossip.
    fn self_record(&self) -> PeerRecord {
        self.peer_record()
    }

    /// Records we may hand out: active peers' records plus a random slice of the
    /// passive set, steered toward `wanted` trees first.
    fn records_for(&mut self, wanted: TreeSet, max: usize, exclude: &NodeId) -> Vec<PeerRecord> {
        let mut out: Vec<PeerRecord> = Vec::new();
        let mut seen: BTreeSet<NodeId> = BTreeSet::new();
        seen.insert(*exclude);
        for t in wanted.iter() {
            for r in self.pool(t) {
                if out.len() >= max {
                    break;
                }
                if seen.insert(r.node_id) {
                    out.push(r);
                }
            }
        }
        let mut rest: Vec<PeerRecord> = self
            .membership
            .active
            .values()
            .map(|a| a.record)
            .chain(self.passive.values().copied())
            .filter(|r| {
                !seen.contains(&r.node_id) && r.node_class != NodeClass::Leaf || r.flags.source
            })
            .filter(|r| !r.assigned_trees.is_empty() || r.flags.source)
            .collect();
        rest.shuffle(&mut self.rng);
        for r in rest {
            if out.len() >= max {
                break;
            }
            if seen.insert(r.node_id) {
                out.push(r);
            }
        }
        out
    }

    // ------------------------------------------------------------------ timers

    /// Earliest membership timer.
    pub(super) fn membership_next_timer(&self) -> Option<Instant> {
        self.matrix.as_ref()?;
        let mut t = self
            .membership
            .next_gossip
            .min(self.membership.next_shuffle);
        if let Some(j) = self.membership.joining_since {
            t = t.min(j + Duration::from_millis(1000));
        }
        Some(t)
    }

    /// Run due membership timers.
    pub(super) fn membership_timers(&mut self) {
        let now = self.now;
        if self.matrix.is_none() {
            return;
        }
        // JOINING → CONNECTING when the Active Set reaches θ_join, or after 1 s regardless.
        if self.state == Lifecycle::Joining {
            let theta = self.params.theta_join(self.swarm_size.max(2));
            let waited = self
                .membership
                .joining_since
                .map(|j| now.duration_since(j) >= Duration::from_millis(1000))
                .unwrap_or(true);
            if self.membership.active.len() >= theta || waited {
                self.membership.joining_since = None;
                self.set_state(Lifecycle::Connecting);
                self.start_tree_joins();
            }
        }
        if now >= self.membership.next_gossip {
            self.membership.next_gossip = now + self.params.tau_gossip;
            self.gossip_round();
            self.refill_pools();
        }
        if now >= self.membership.next_shuffle {
            self.membership.next_shuffle = now + self.params.tau_gossip * 5;
            self.shuffle_walk();
        }
    }

    /// Begin tree joins for every wanted tree without a parent.
    pub(super) fn start_tree_joins(&mut self) {
        let ids: Vec<TreeId> = self.trees.iter().map(|t| t.id).collect();
        for id in ids {
            let t = &self.trees[id.index()];
            if t.wants_parent() && t.parent.is_none() && !t.join.is_active() {
                self.begin_probing(id);
            }
        }
    }

    // ------------------------------------------------------------------ JOIN (Ch3 §3.1.2)

    /// JOINING: send JOIN to up to θ_join contacts from discovery and open sessions
    /// to them (the plain-UDP → QUIC upgrade of App D §D.4.2).
    pub(super) fn begin_joining(&mut self) {
        let theta = self.params.theta_join(self.swarm_size.max(2));
        let mut contacts: Vec<PeerRecord> = self
            .passive
            .values()
            .filter(|r| r.node_id != self.node_id())
            .filter(|r| r.flags.reachability != bs_wire::Reachability::Symmetric)
            .copied()
            .collect();
        contacts.shuffle(&mut self.rng);
        if let Some(s) = self.source_record {
            if !contacts.iter().any(|c| c.node_id == s.node_id) {
                contacts.push(s);
            }
        }
        self.membership.joining_since = Some(self.now);
        let sid = self.stream_id();
        for c in contacts.into_iter().take(theta) {
            let mut body = Vec::with_capacity(36);
            body.extend_from_slice(sid.as_bytes());
            body.push(self.cfg.node_class as u8);
            body.extend_from_slice(&[0, 0, 0]);
            let vb = self.validation_block(FrameType::JOIN, &body);
            self.send(
                c.addr.0,
                Channel::Udp,
                Frame::Join(Join {
                    validation: vb,
                    stream_id: sid,
                    node_class: self.cfg.node_class,
                }),
                None,
            );
            self.membership.join_contacts.insert(c.addr.0);
            self.open_session(c.addr.0);
        }
    }

    /// Contact side: a JOIN arrived on UDP. Admit once the joiner's session is open.
    pub(super) fn on_join(&mut self, from: PeerAddr, j: Join) {
        let mut body = Vec::with_capacity(36);
        body.extend_from_slice(j.stream_id.as_bytes());
        body.push(j.node_class as u8);
        body.extend_from_slice(&[0, 0, 0]);
        if j.stream_id != self.stream_id()
            || !self.verify_vb(&j.validation, FrameType::JOIN, &body, from, j.node_class)
        {
            return;
        }
        let open = self.sessions.get(&from).map(|s| s.open).unwrap_or(false);
        if open {
            self.admit_joiner(from, &j);
        } else {
            self.membership.pending_joiners.insert(from, j);
        }
    }

    /// Session to a pending joiner opened.
    pub(super) fn membership_session_opened(&mut self, peer: PeerAddr) {
        if let Some(j) = self.membership.pending_joiners.remove(&peer) {
            self.admit_joiner(peer, &j);
        }
    }

    fn admit_joiner(&mut self, from: PeerAddr, j: &Join) {
        let node_id = j.validation.node_id;
        let record = PeerRecord {
            node_id,
            node_class: j.node_class,
            assigned_trees: TreeSet::EMPTY,
            flags: PeerFlags::PUBLIC,
            addr: WireAddr(from),
        };
        self.active_add(from, record, true);
        let f = Accepted {
            sender: self.node_id(),
            accepted_type: AcceptedType::Join,
            tree_id: TreeId::NONE,
            hop_depth: 0,
            pending: false,
        };
        self.send(from, Channel::Control, Frame::Accepted(f), None);
        // Random walk: FORWARD_JOIN to every other active neighbour.
        let peers: Vec<PeerAddr> = self
            .membership
            .active
            .keys()
            .copied()
            .filter(|p| *p != from)
            .collect();
        let fj = ForwardJoin {
            origin: record,
            ttl: self.params.ttl_active,
            pr_gossip: self.params.pr_gossip,
        };
        for p in peers {
            self.send(p, Channel::Control, Frame::ForwardJoin(fj.clone()), None);
        }
        self.membership.forward_joins += 1;
    }

    /// Add a peer to the Active Set, evicting a random member with no tree role if
    /// the set is full and `force` (JOIN, FORWARD_JOIN leaf, HIGH priority) applies.
    pub(super) fn active_add(&mut self, addr: PeerAddr, record: PeerRecord, force: bool) -> bool {
        if addr == self.cfg.addr || record.node_id == self.node_id() {
            return false;
        }
        if let Some(a) = self.membership.active.get_mut(&addr) {
            if a.record.assigned_trees.is_empty() {
                a.record = record;
            }
            return true;
        }
        let target = self.active_target();
        if self.membership.active.len() >= target {
            if !force {
                return false;
            }
            let tree_peers = self.tree_peer_addrs();
            let victims: Vec<PeerAddr> = self
                .membership
                .active
                .keys()
                .copied()
                .filter(|p| !tree_peers.contains(p))
                .collect();
            if victims.is_empty() {
                return false;
            }
            let v = victims[self.rng.random_range(0..victims.len())];
            self.active_remove(v, "active set full");
            let f = bs_wire::frames::Disconnect {
                sender: self.node_id(),
                reason: bs_wire::DisconnectReason::Quit,
                tree_id: TreeId::NONE,
            };
            self.send(v, Channel::Control, Frame::Disconnect(f), None);
            self.out.push_back(crate::io::Output::CloseSession(v));
            self.sessions.remove(&v);
        }
        self.membership.active.insert(
            addr,
            ActivePeer {
                node_id: record.node_id,
                record,
                since: self.now,
                seq_in_max: 0,
                wants: TreeSet::EMPTY,
            },
        );
        self.emit(Event::Note {
            text: format!(
                "active set +{} ({} members)",
                record.node_id.short(),
                self.membership.active.len()
            ),
        });
        true
    }

    /// Remove from the Active Set (demote to Passive if we hold a usable record).
    pub(super) fn active_remove(&mut self, addr: PeerAddr, _reason: &str) {
        if let Some(a) = self.membership.active.remove(&addr) {
            if !a.record.assigned_trees.is_empty() || a.record.flags.source {
                self.passive_insert(a.record);
            }
        }
    }

    /// Addresses with a tree relationship (never evicted from the Active Set for room).
    pub(super) fn tree_peer_addrs(&self) -> BTreeSet<PeerAddr> {
        let mut s = BTreeSet::new();
        for t in &self.trees {
            if let Some(p) = &t.parent {
                s.insert(p.addr);
            }
            if let Some(p) = &t.old_parent {
                s.insert(p.addr);
            }
            for c in t.children.keys() {
                s.insert(*c);
            }
        }
        s
    }

    /// FORWARD_JOIN random walk step (Ch3 §3.1.2).
    pub(super) fn on_forward_join(&mut self, from: PeerAddr, fj: ForwardJoin) {
        if fj.origin.node_id == self.node_id() {
            return;
        }
        let hops = self.params.ttl_active.saturating_sub(fj.ttl);
        if hops >= fj.pr_gossip {
            self.passive_insert(fj.origin);
        }
        let others: Vec<PeerAddr> = self
            .membership
            .active
            .keys()
            .copied()
            .filter(|p| *p != from && *p != fj.origin.addr.0)
            .collect();
        if fj.ttl == 0 || others.is_empty() {
            // Leaf of the walk: connect to the origin and ask for membership.
            if !self.membership.active.contains_key(&fj.origin.addr.0) {
                self.open_session(fj.origin.addr.0);
                self.membership.pending_neighbor.insert(fj.origin.addr.0);
            }
            return;
        }
        let next = others[self.rng.random_range(0..others.len())];
        let fwd = ForwardJoin {
            origin: fj.origin,
            ttl: fj.ttl - 1,
            pr_gossip: fj.pr_gossip,
        };
        self.send(next, Channel::Control, Frame::ForwardJoin(fwd), None);
    }

    /// A session we opened for membership is up: send NEIGHBOR(TreeID = 0, HIGH).
    pub(super) fn membership_send_neighbor(&mut self, peer: PeerAddr) {
        let f = Neighbor {
            sender: self.node_id(),
            priority: Priority::High,
            tree_id: TreeId::NONE,
            node_class: self.cfg.node_class,
            assigned_trees: self.peer_record().assigned_trees,
        };
        self.send(peer, Channel::Control, Frame::Neighbor(f), None);
    }

    /// ACCEPTED for a JOIN or NEIGHBOR(0): the peer is now an active neighbour.
    pub(super) fn on_membership_accepted(&mut self, from: PeerAddr, a: &Accepted) {
        self.membership.join_contacts.remove(&from);
        let record = self.passive.get(&a.sender).copied().unwrap_or(PeerRecord {
            node_id: a.sender,
            node_class: NodeClass::Relay,
            assigned_trees: TreeSet::EMPTY,
            flags: PeerFlags::PUBLIC,
            addr: WireAddr(from),
        });
        self.active_add(from, record, true);
    }

    // ------------------------------------------------------------------ SHUFFLE / GOSSIP_EXCHANGE

    fn shuffle_walk(&mut self) {
        let peers: Vec<PeerAddr> = self.membership.active.keys().copied().collect();
        if peers.is_empty() {
            return;
        }
        let to = peers[self.rng.random_range(0..peers.len())];
        let wanted = self.wanted_trees();
        let me = self.node_id();
        let mut records = self.records_for(wanted, 7, &me);
        records.push(self.self_record());
        let f = Shuffle {
            origin: me,
            ttl: 3,
            wanted_trees: wanted,
            records,
        };
        self.send(to, Channel::Control, Frame::Shuffle(f), None);
    }

    /// SHUFFLE: integrate, then forward (ttl > 0) or answer (ttl == 0).
    pub(super) fn on_shuffle(&mut self, from: PeerAddr, s: Shuffle) {
        self.membership.shuffles += 1;
        self.note_referrer(&s.records, from);
        for r in &s.records {
            self.passive_insert(*r);
        }
        if let Some(a) = self.membership.active.get_mut(&from) {
            a.wants = s.wanted_trees;
        }
        if s.ttl == 0 {
            self.send_exchange(from, s.wanted_trees);
            return;
        }
        let others: Vec<PeerAddr> = self
            .membership
            .active
            .keys()
            .copied()
            .filter(|p| *p != from)
            .collect();
        if others.is_empty() {
            return;
        }
        let next = others[self.rng.random_range(0..others.len())];
        let fwd = Shuffle {
            origin: s.origin,
            ttl: s.ttl - 1,
            wanted_trees: s.wanted_trees,
            records: s.records,
        };
        self.send(next, Channel::Control, Frame::Shuffle(fwd), None);
    }

    fn gossip_round(&mut self) {
        let peers: Vec<(PeerAddr, TreeSet)> = self
            .membership
            .active
            .iter()
            .map(|(a, p)| (*a, p.wants))
            .collect();
        for (p, wants) in peers {
            self.send_exchange(p, wants);
        }
    }

    fn send_exchange(&mut self, to: PeerAddr, wants: TreeSet) {
        let Some(key) = self.sessions.get(&to).and_then(|s| s.session_key) else {
            return;
        };
        let exclude = self
            .membership
            .active
            .get(&to)
            .map(|a| a.node_id)
            .unwrap_or(NodeId::ZERO);
        let records = self.records_for(wants, 8, &exclude);
        self.membership.seq_out += 1;
        let mut g = GossipExchange {
            sender: self.node_id(),
            sequence: self.membership.seq_out,
            wanted_trees: self.wanted_trees(),
            records,
            hmac: Hash::ZERO,
        };
        g.hmac = gossip_hmac(&key, &g.authenticated_bytes());
        self.send(to, Channel::Control, Frame::GossipExchange(g), None);
    }

    /// GOSSIP_EXCHANGE: verify HMAC and sequence, then integrate (Ch3 §3.2.2).
    pub(super) fn on_gossip_exchange(&mut self, from: PeerAddr, g: GossipExchange) {
        let Some(key) = self.sessions.get(&from).and_then(|s| s.session_key) else {
            return;
        };
        if gossip_hmac(&key, &g.authenticated_bytes()) != g.hmac {
            self.membership.exchanges_rejected += 1;
            return;
        }
        let Some(a) = self.membership.active.get_mut(&from) else {
            return;
        };
        if g.sequence <= a.seq_in_max {
            self.membership.exchanges_rejected += 1;
            return;
        }
        a.seq_in_max = g.sequence;
        a.wants = g.wanted_trees;
        self.membership.exchanges += 1;
        self.note_referrer(&g.records, from);
        for r in &g.records {
            self.passive_insert(*r);
        }
    }

    /// Pools below the floor for a full gossip round re-query discovery, at most
    /// every 30 s per tree (Ch3 §3.1.1); a direct shuffle request goes to a
    /// surviving neighbour first (tier 3 of Ch3 §3.3.1).
    fn refill_pools(&mut self) {
        let wanted = self.wanted_trees();
        if wanted.is_empty() {
            return;
        }
        if let Some(&to) = self.membership.active.keys().next() {
            let me = self.node_id();
            let f = Shuffle {
                origin: me,
                ttl: 0,
                wanted_trees: wanted,
                records: vec![self.self_record()],
            };
            self.send(to, Channel::Control, Frame::Shuffle(f), None);
        }
        let now = self.now;
        for t in wanted.iter() {
            let last = self.membership.last_pool_query.get(&t.0).copied();
            if last
                .map(|l| now.duration_since(l) >= Duration::from_secs(30))
                .unwrap_or(true)
            {
                self.membership.last_pool_query.insert(t.0, now);
                self.request_discovery(TreeSet::single(t));
            }
        }
    }
}
