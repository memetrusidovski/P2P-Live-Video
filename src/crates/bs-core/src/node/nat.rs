//! NAT traversal and emergent relays (Ch6).
//!
//! Reachability from two PONG reflections (Ch2 §2.1.3), hole punching through the
//! referrer that named a CONE candidate (App D §D.4.14), the keep-alive a CONE
//! registrant owes its guardians, and the RELAY_PROPOSAL / RELAY_BIND bridge that a
//! peer without a reachable parent asks a RELAY_CAPABLE node for (Ch6 §6.3).

use std::collections::{BTreeMap, BTreeSet};

use bs_wire::frames::{
    PunchForward, PunchFrame, PunchRequest, RelayBind, RelayProposal, BIND_REJECTED,
    BIND_SOURCE_INGRESS,
};
use bs_wire::{
    Frame, FrameType, NodeClass, NodeId, PeerFlags, PeerRecord, Reachability, TreeId, TreeSet,
    WireAddr,
};

use super::Node;
use crate::forest::{ChildLink, ParentLink};
use crate::io::{Channel, Event, PeerAddr};
use crate::join::JoinState;
use crate::time::{Duration, Instant};

/// A bridge this node serves: one NAT-blocked child in one tree.
#[derive(Debug, Clone)]
pub struct Bridge {
    /// The bridged peer.
    pub client: NodeId,
    /// Upload the bridge costs, kbps.
    pub kbps: u32,
    /// When the bind was sent; the slot expires if the child never joins.
    pub bound_at: Instant,
}

/// A bridge this node asked for.
#[derive(Debug, Clone)]
struct BridgeRequest {
    relay: PeerRecord,
    sent: bool,
    since: Instant,
}

/// NAT state on a node.
#[derive(Debug, Default)]
pub struct NatState {
    /// Reflected addresses by responder, until reachability is known.
    reflections: BTreeMap<PeerAddr, WireAddr>,
    /// Who named each record to us (guardian or gossip neighbour).
    referrers: BTreeMap<NodeId, PeerAddr>,
    /// Punches sent, by target, so one candidate is punched once per round.
    punched: BTreeMap<NodeId, Instant>,
    /// Next keep-alive PING to the guardians (CONE registrants).
    next_keepalive: Option<Instant>,
    /// The RELAY_CAPABLE bit last registered, so a change re-registers.
    advertised_relay_capable: bool,
    /// Bridges served, by (child address, tree).
    bridges: BTreeMap<(PeerAddr, u8), Bridge>,
    /// Proposals accepted whose tree we still have to join: (client, address, tree).
    pending_bridges: Vec<(NodeId, PeerAddr, TreeId)>,
    /// Bridges asked for, by tree.
    requests: BTreeMap<u8, BridgeRequest>,
    /// Relays that declined or timed out.
    tried_relays: BTreeSet<NodeId>,
    /// Relays we are bridged through, by tree.
    bridged_parent: BTreeMap<u8, PeerAddr>,
    /// The next discovery asks for RELAY_CAPABLE records.
    want_relay: bool,
    /// Source behind a NAT: ingress relays bound (Ch6 §6.3.3).
    ingress: BTreeSet<PeerAddr>,
    /// Source: proposals out, by relay address, with the record and when.
    ingress_requests: BTreeMap<PeerAddr, (PeerRecord, bool, Instant)>,
    /// Source: relays that declined or did not answer.
    ingress_tried: BTreeSet<NodeId>,
    /// Relay: we are an ingress relay for the source at this address.
    ingress_source: Option<PeerAddr>,
    /// Source: when the guardians were last asked for relays.
    ingress_query_at: Option<Instant>,
    /// Counters.
    pub punches_sent: u64,
    /// Counters.
    pub punches_forwarded: u64,
    /// Counters.
    pub bridges_bound: u64,
    /// Counters.
    pub bridges_rejected: u64,
}

impl Node {
    // ------------------------------------------------------------------ reachability

    /// The class this node acts as: a SYMMETRIC peer is leaf-class (Ch1 §1.2.5).
    pub fn effective_class(&self) -> NodeClass {
        if self.reach == Reachability::Symmetric {
            NodeClass::Leaf
        } else {
            self.cfg.node_class
        }
    }

    /// Whether this node meets the emergent-relay criteria (Ch6 §6.3.1).
    pub fn relay_capable(&self) -> bool {
        let uptime = self
            .started_at
            .map(|t| self.now.duration_since(t))
            .unwrap_or(Duration::ZERO);
        !self.is_source()
            && self.reach == Reachability::Public
            && self.cfg.node_class == NodeClass::Relay
            && self.cfg.upload_kbps >= self.params.relay_capable_kbps
            && uptime >= self.params.relay_capable_uptime
    }

    /// Flags for our own Peer Record.
    pub(super) fn peer_flags(&self) -> PeerFlags {
        PeerFlags {
            reachability: self.reach,
            relay_capable: self.relay_capable(),
            source: self.is_source(),
            source_ingress: self.is_ingress(),
        }
    }

    /// Whether this node is an ingress relay: hop 0 of every tree for a source
    /// behind a NAT (Ch6 §6.3.3).
    pub fn is_ingress(&self) -> bool {
        self.nat.ingress_source.is_some()
    }

    /// Ingress relays bound by this source.
    pub fn ingress_count(&self) -> usize {
        self.nat.ingress.len()
    }

    // ------------------------------------------------------------------ source behind a NAT

    /// Source: bind ingress relays while fewer than wanted are bound.
    pub(super) fn ingress_timers(&mut self) {
        if !self.is_source() || self.reach == Reachability::Public {
            return;
        }
        let timeout = self.params.bridge_timeout;
        let stale: Vec<PeerAddr> = self
            .nat
            .ingress_requests
            .iter()
            .filter(|(_, (_, _, t))| self.now.duration_since(*t) > timeout)
            .map(|(a, _)| *a)
            .collect();
        for a in stale {
            if let Some((rec, _, _)) = self.nat.ingress_requests.remove(&a) {
                self.nat.ingress_tried.insert(rec.node_id);
            }
        }
        // RULE: An ingress relay is a child in every tree (Ch6 §6.3.3). The source
        //       binds no more of them than its weakest tree holds.
        let slots = self.trees.iter().map(|t| t.k_v as usize).min().unwrap_or(0);
        let wanted = (self.params.ingress_relays as usize).min(slots);
        if self.nat.ingress.len() + self.nat.ingress_requests.len() >= wanted {
            return;
        }
        let me = self.node_id();
        let mut pool: Vec<PeerRecord> = self
            .passive
            .values()
            .copied()
            .chain(self.membership.active.values().map(|a| a.record))
            .chain(self.dht_relay_capable_records())
            .filter(|r| r.flags.relay_capable && r.node_id != me)
            .filter(|r| !self.nat.ingress_tried.contains(&r.node_id))
            .filter(|r| !self.nat.ingress.contains(&r.addr.0))
            .filter(|r| !self.nat.ingress_requests.contains_key(&r.addr.0))
            .collect();
        pool.sort_by_key(|r| r.node_id);
        pool.dedup_by_key(|r| r.node_id);
        if pool.is_empty() {
            // Ask the guardians for RELAY_CAPABLE records, as any blocked peer does.
            let due = self
                .nat
                .ingress_query_at
                .map(|t| self.now.duration_since(t) >= Duration::from_secs(2))
                .unwrap_or(true);
            if due && !self.discovery_inflight {
                self.nat.ingress_query_at = Some(self.now);
                self.nat.want_relay = true;
                self.request_discovery(TreeSet::EMPTY);
            }
            return;
        }
        if let Some(rec) = pool.into_iter().next() {
            let addr = rec.addr.0;
            let open = self.sessions.get(&addr).map(|s| s.open).unwrap_or(false);
            self.nat
                .ingress_requests
                .insert(addr, (rec, false, self.now));
            if open {
                self.send_ingress_proposal(addr);
            } else {
                self.open_session(addr);
            }
        }
    }

    fn send_ingress_proposal(&mut self, addr: PeerAddr) {
        if let Some(r) = self.nat.ingress_requests.get_mut(&addr) {
            r.1 = true;
        }
        let p = RelayProposal {
            target: NodeId::ZERO,
            target_port: 0,
            tree_id: TreeId::NONE,
            bind_flags: BIND_SOURCE_INGRESS,
        };
        self.send(addr, Channel::Control, Frame::RelayProposal(p), None);
    }

    /// Source: an ingress relay answered.
    fn on_ingress_bind(&mut self, from: PeerAddr, b: RelayBind) {
        let Some((rec, _, _)) = self.nat.ingress_requests.remove(&from) else {
            return;
        };
        if b.rejected() {
            self.nat.ingress_tried.insert(rec.node_id);
            return;
        }
        // The relay is a child in every tree: the source pushes all M slices to it.
        let all = TreeSet::first_n(self.trees.len() as u8);
        for t in &mut self.trees {
            t.children.insert(
                from,
                ChildLink {
                    addr: from,
                    node_id: rec.node_id,
                    node_class: NodeClass::Relay,
                    assigned_trees: all,
                    last_tx: self.now,
                    last_rx: self.now,
                    admitted_at: self.now,
                    parity: self.params.default_parity,
                    draining_until: None,
                    pending: false,
                    rank: 0.0,
                },
            );
        }
        self.nat.ingress.insert(from);
        self.send_recent_manifests(from);
        self.emit(Event::IngressBound { relay: rec.node_id });
    }

    /// Relay: the source proposes that we become its ingress (Ch6 §6.3.3).
    fn on_ingress_proposal(&mut self, from: PeerAddr) {
        let Some(peer) = self.sessions.get(&from).and_then(|s| s.node_id) else {
            return;
        };
        // SECURITY: Only the stream's publisher can bind an ingress relay: the session
        //           identity must be the one the Stream Record names.
        let is_publisher = self
            .source_record
            .map(|r| r.node_id == peer && r.flags.source)
            .unwrap_or(false);
        let Some(matrix) = self.matrix.clone() else {
            return;
        };
        let m = matrix.m() as u32;
        let parity = self.mean_child_parity();
        // RULE: An ingress relay holds at least two slots in every tree (Ch6 §6.3.3).
        let enough = matrix.trees.iter().all(|t| {
            self.params
                .slots(self.cfg.upload_kbps, m, t.bitrate_kbps, parity)
                >= 2
        });
        let accept = is_publisher && self.relay_capable() && enough && !self.is_ingress();
        let me = self.node_id();
        if !accept {
            let b = RelayBind {
                client_a: peer,
                client_b: me,
                tree_id: TreeId::NONE,
                bind_flags: BIND_SOURCE_INGRESS | BIND_REJECTED,
            };
            self.send(from, Channel::Control, Frame::RelayBind(b), None);
            return;
        }
        self.nat.ingress_source = Some(from);
        // Hop 0 of every tree: the source is our parent everywhere, and we relay all.
        for t in &mut self.trees {
            t.assigned = true;
            t.join = JoinState::Idle;
            t.repairing = false;
            t.lost_at = None;
            t.old_parent = None;
            t.parent = Some(ParentLink {
                addr: from,
                node_id: peer,
                parent_depth: 0,
                last_rx: self.now,
                pinged: false,
                srtt: Duration::from_millis(50),
                rttvar: Duration::from_millis(25),
                attached_at: self.now,
                first_block_at: None,
                pending: false,
                rejected_blocks: 0,
                attached_as_repair: false,
            });
            t.depth = 0;
        }
        self.recompute_slots();
        self.dht.registered = None;
        self.register_self();
        self.emit(Event::IngressServing { source: peer });
        let b = RelayBind {
            client_a: peer,
            client_b: me,
            tree_id: TreeId::NONE,
            bind_flags: BIND_SOURCE_INGRESS,
        };
        self.send(from, Channel::Control, Frame::RelayBind(b), None);
        self.check_active();
    }

    /// A PONG reflected our address: decide reachability after two responders.
    pub(super) fn on_reflection(&mut self, from: PeerAddr, reflected: WireAddr) {
        if self.cfg.reachability.is_some() {
            return;
        }
        self.nat.reflections.insert(from, reflected);
        if self.nat.reflections.len() < 2 {
            return;
        }
        let mut seen: Vec<WireAddr> = self.nat.reflections.values().copied().collect();
        seen.sort_by_key(|a| (a.0.ip(), a.0.port()));
        seen.dedup();
        // RULE: One mapping equal to our own address is no NAT. One shared mapping is
        //       a cone. A mapping per responder is symmetric (Ch2 §2.1.3).
        let decided = if seen.len() == 1 && seen[0].0 == self.cfg.addr {
            Reachability::Public
        } else if seen.len() == 1 {
            Reachability::Cone
        } else {
            Reachability::Symmetric
        };
        if decided != self.reach {
            self.reach = decided;
            self.emit(Event::ReachabilityDetected { reach: decided });
            self.configure_tree_roles();
            self.recompute_slots();
            self.dht.registered = None;
            self.register_self();
        }
    }

    // ------------------------------------------------------------------ punching

    /// Remember who named these records.
    pub(super) fn note_referrer(&mut self, records: &[PeerRecord], from: PeerAddr) {
        for r in records {
            self.nat.referrers.insert(r.node_id, from);
        }
    }

    /// Ask a CONE candidate's referrer to relay a punch request before the PROBE.
    pub(super) fn punch_if_needed(&mut self, candidate: &PeerRecord) {
        if candidate.flags.reachability != Reachability::Cone {
            return;
        }
        let open = self
            .sessions
            .get(&candidate.addr.0)
            .map(|s| s.open)
            .unwrap_or(false);
        if open {
            return;
        }
        let recent = self
            .nat
            .punched
            .get(&candidate.node_id)
            .map(|t| self.now.duration_since(*t) < self.params.probe_timeout)
            .unwrap_or(false);
        if recent {
            return;
        }
        let Some(referrer) = self.nat.referrers.get(&candidate.node_id).copied() else {
            return;
        };
        self.nat.punched.insert(candidate.node_id, self.now);
        let sid = self.stream_id();
        let mut body = Vec::with_capacity(64);
        body.extend_from_slice(&candidate.node_id.0);
        body.extend_from_slice(sid.as_bytes());
        let vb = self.validation_block(FrameType::PUNCH_REQUEST, &body);
        let f = PunchFrame::Request(PunchRequest {
            validation: vb,
            target: candidate.node_id,
            stream_id: sid,
        });
        self.nat.punches_sent += 1;
        self.emit(Event::PunchSent {
            target: candidate.node_id,
        });
        self.send(referrer, Channel::Udp, Frame::PunchRequest(f), None);
    }

    /// A PUNCH_REQUEST in either form.
    pub(super) fn on_punch(&mut self, from: PeerAddr, f: PunchFrame) {
        match f {
            PunchFrame::Request(r) => {
                let mut body = Vec::with_capacity(64);
                body.extend_from_slice(&r.target.0);
                body.extend_from_slice(r.stream_id.as_bytes());
                if !self.verify_vb(
                    &r.validation,
                    FrameType::PUNCH_REQUEST,
                    &body,
                    from,
                    NodeClass::Leaf,
                ) || r.stream_id != self.stream_id()
                {
                    return;
                }
                let Some(target) = self.resolve_addr_full(&r.target) else {
                    return;
                };
                // SECURITY: The forwarded record carries the address we observed, never
                //           one the requester declared (App D §D.4.14).
                let known = self.known_record(&r.validation.node_id);
                let requester = PeerRecord {
                    node_id: r.validation.node_id,
                    node_class: known.map(|k| k.node_class).unwrap_or(NodeClass::Leaf),
                    assigned_trees: known.map(|k| k.assigned_trees).unwrap_or(TreeSet::EMPTY),
                    flags: known.map(|k| k.flags).unwrap_or(PeerFlags::PUBLIC),
                    addr: WireAddr(from),
                };
                let mut fbody = Vec::with_capacity(90);
                fbody.extend_from_slice(&requester.node_id.0);
                fbody.extend_from_slice(r.stream_id.as_bytes());
                let vb = self.validation_block(FrameType::PUNCH_REQUEST, &fbody);
                let fwd = PunchFrame::Forward(PunchForward {
                    validation: vb,
                    requester,
                    stream_id: r.stream_id,
                });
                self.nat.punches_forwarded += 1;
                self.send(target, Channel::Udp, Frame::PunchRequest(fwd), None);
            }
            PunchFrame::Forward(fw) => {
                let mut body = Vec::with_capacity(90);
                body.extend_from_slice(&fw.requester.node_id.0);
                body.extend_from_slice(fw.stream_id.as_bytes());
                if !self.verify_vb(
                    &fw.validation,
                    FrameType::PUNCH_REQUEST,
                    &body,
                    from,
                    NodeClass::Leaf,
                ) || fw.stream_id != self.stream_id()
                {
                    return;
                }
                // The PING opens our mapping toward the requester; its PROBE follows.
                self.send_ping(fw.requester.addr.0);
            }
        }
    }

    /// A PING from a candidate under probe: the punch landed, PROBE again.
    pub(super) fn reprobe_after_punch(&mut self, from: PeerAddr) {
        let mut again: Vec<(TreeId, PeerRecord)> = Vec::new();
        for t in &mut self.trees {
            if let JoinState::Probing { candidates, .. } = &mut t.join {
                for c in candidates.iter_mut() {
                    if c.record.addr.0 == from && c.response.is_none() {
                        c.probed_at = self.now;
                        again.push((t.id, c.record));
                    }
                }
            }
        }
        for (tree, rec) in again {
            self.send_probe(tree, rec.addr.0);
        }
    }

    /// The address we hold for `id`: a session, a membership record, the routing
    /// table, or the guardian store.
    fn resolve_addr_full(&self, id: &NodeId) -> Option<PeerAddr> {
        if let Some((addr, _)) = self.sessions.iter().find(|(_, s)| s.node_id == Some(*id)) {
            return Some(*addr);
        }
        self.known_record(id).map(|r| r.addr.0)
    }

    /// A record we hold for `id`, from membership, the routing table or the store.
    fn known_record(&self, id: &NodeId) -> Option<PeerRecord> {
        if let Some(a) = self.membership.active.values().find(|a| a.node_id == *id) {
            return Some(a.record);
        }
        if let Some(r) = self.passive.get(id) {
            return Some(*r);
        }
        if let Some(c) = self.dht.table.get(id) {
            return Some(c.record);
        }
        self.dht_stored_record(id)
    }

    /// CONE registrants keep their guardian mappings alive every τ_nat (App D §D.4.14).
    pub(super) fn nat_timers(&mut self) {
        // WHY: The uptime criterion flips RELAY_CAPABLE on after the first
        //      registration. The guardians must hold the new bit to name us.
        let capable = self.relay_capable();
        if capable != self.nat.advertised_relay_capable && self.dht.registered.is_some() {
            self.nat.advertised_relay_capable = capable;
            self.dht.registered = None;
            self.register_self();
        }
        if self.reach == Reachability::Cone && self.dht.registered.is_some() {
            let due = self
                .nat
                .next_keepalive
                .map(|t| self.now >= t)
                .unwrap_or(true);
            if due {
                self.nat.next_keepalive = Some(self.now + self.params.tau_nat);
                let guardians = self.dht_registered_addrs();
                for g in guardians {
                    self.send_ping(g);
                }
            }
        }
        // A bound bridge whose child never joined releases its budget.
        let now = self.now;
        let timeout = self.params.bridge_timeout;
        let unused: Vec<(PeerAddr, u8)> = self
            .nat
            .bridges
            .iter()
            .filter(|((addr, t), b)| {
                now.duration_since(b.bound_at) > timeout
                    && !self
                        .trees
                        .get(TreeId(*t).index())
                        .map(|x| x.children.contains_key(addr))
                        .unwrap_or(false)
            })
            .map(|(k, _)| *k)
            .collect();
        for (addr, t) in unused {
            if let Some(client) = self.nat.bridges.get(&(addr, t)).map(|b| b.client) {
                self.emit(Event::BridgeExpired { tree: t, client });
            }
            self.nat_child_removed(addr, TreeId(t));
        }
        // A proposal without an answer: try the next relay.
        let stale: Vec<u8> = self
            .nat
            .requests
            .iter()
            .filter(|(_, r)| self.now.duration_since(r.since) > timeout)
            .map(|(t, _)| *t)
            .collect();
        for t in stale {
            if let Some(r) = self.nat.requests.remove(&t) {
                self.nat.tried_relays.insert(r.relay.node_id);
                self.nat.bridges_rejected += 1;
                self.request_bridge(TreeId(t));
            }
        }
        if self.nat.punched.len() > 256 {
            let now = self.now;
            self.nat
                .punched
                .retain(|_, t| now.duration_since(*t) < Duration::from_secs(10));
        }
    }

    // ------------------------------------------------------------------ bridges: the blocked peer

    /// Whether tree `tree` is served to us through a bridge.
    pub fn bridged_parent(&self, tree: TreeId) -> Option<PeerAddr> {
        self.nat.bridged_parent.get(&tree.0).copied()
    }

    /// No reachable parent for `tree`: ask a RELAY_CAPABLE peer for a bridge.
    pub(super) fn request_bridge(&mut self, tree: TreeId) {
        if self.nat.requests.contains_key(&tree.0) || self.is_source() {
            return;
        }
        // A candidate we already hold and have not tried is asked at once.
        let known: Vec<PeerRecord> = self
            .passive
            .values()
            .chain(self.membership.active.values().map(|a| &a.record))
            .filter(|r| r.flags.relay_capable && !self.nat.tried_relays.contains(&r.node_id))
            .filter(|r| r.node_id != self.node_id())
            .copied()
            .collect();
        // Distinct relays per tree where the sample offers them (Ch1 §1.2.1).
        let used: BTreeSet<PeerAddr> = self.nat.bridged_parent.values().copied().collect();
        let pick = known
            .iter()
            .filter(|r| !used.contains(&r.addr.0))
            .min_by_key(|r| r.node_id)
            .or_else(|| known.iter().min_by_key(|r| r.node_id))
            .copied();
        if let Some(rec) = pick {
            self.propose_bridge(tree, rec);
            return;
        }
        if !self.discovery_inflight {
            self.nat.want_relay = true;
            self.request_discovery(TreeSet::single(tree));
        }
    }

    /// Whether the next GET_PEERS asks for relays, and clears the wish.
    pub(super) fn take_want_relay(&mut self) -> bool {
        std::mem::take(&mut self.nat.want_relay)
    }

    /// Discovery answered: send proposals for trees that wait on a relay.
    pub(super) fn on_relay_records(&mut self, records: &[PeerRecord]) {
        let waiting: Vec<TreeId> = self
            .trees
            .iter()
            .filter(|t| {
                t.wants_parent()
                    && t.parent.is_none()
                    && !self.nat.requests.contains_key(&t.id.0)
                    && self.join_stats[t.id.index()].failed_rounds
                        >= self.params.bridge_after_failed_rounds
            })
            .map(|t| t.id)
            .collect();
        for tree in waiting {
            let pick = records
                .iter()
                .filter(|r| r.flags.relay_capable && !self.nat.tried_relays.contains(&r.node_id))
                .min_by_key(|r| r.node_id)
                .copied();
            if let Some(rec) = pick {
                self.propose_bridge(tree, rec);
            }
        }
    }

    fn propose_bridge(&mut self, tree: TreeId, relay: PeerRecord) {
        let addr = relay.addr.0;
        let open = self.sessions.get(&addr).map(|s| s.open).unwrap_or(false);
        self.nat.requests.insert(
            tree.0,
            BridgeRequest {
                relay,
                sent: false,
                since: self.now,
            },
        );
        self.passive_insert(relay);
        if open {
            self.send_proposal(tree, addr);
        } else {
            self.open_session(addr);
        }
    }

    fn send_proposal(&mut self, tree: TreeId, addr: PeerAddr) {
        if let Some(r) = self.nat.requests.get_mut(&tree.0) {
            r.sent = true;
        }
        let p = RelayProposal {
            target: NodeId::ZERO,
            target_port: 0,
            tree_id: tree,
            bind_flags: 0,
        };
        self.emit(Event::BridgeProposed { tree: tree.0 });
        self.send(addr, Channel::Control, Frame::RelayProposal(p), None);
    }

    /// A session opened: proposals waiting on it go out.
    pub(super) fn nat_session_opened(&mut self, peer: PeerAddr) {
        if self
            .nat
            .ingress_requests
            .get(&peer)
            .map(|(_, sent, _)| !*sent)
            .unwrap_or(false)
        {
            self.send_ingress_proposal(peer);
        }
        let trees: Vec<u8> = self
            .nat
            .requests
            .iter()
            .filter(|(_, r)| !r.sent && r.relay.addr.0 == peer)
            .map(|(t, _)| *t)
            .collect();
        for t in trees {
            self.send_proposal(TreeId(t), peer);
        }
    }

    /// RELAY_BIND from a relay we proposed to.
    pub(super) fn on_relay_bind(&mut self, from: PeerAddr, b: RelayBind) {
        if b.bind_flags & BIND_SOURCE_INGRESS != 0 {
            self.on_ingress_bind(from, b);
            return;
        }
        let tree = b.tree_id;
        let Some(req) = self.nat.requests.remove(&tree.0) else {
            return;
        };
        if req.relay.addr.0 != from || b.client_a != self.node_id() {
            return;
        }
        if b.rejected() {
            self.nat.tried_relays.insert(req.relay.node_id);
            self.nat.bridges_rejected += 1;
            self.emit(Event::BridgeRejected { tree: tree.0 });
            self.request_bridge(tree);
            return;
        }
        if self.trees[tree.index()].parent.is_some() {
            return;
        }
        self.nat.bridged_parent.insert(tree.0, from);
        self.nat.bridges_bound += 1;
        self.emit(Event::BridgeBound {
            tree: tree.0,
            relay: req.relay.node_id,
        });
        // An ordinary join to the relay: it holds a bridge slot for us.
        self.trees[tree.index()].join = JoinState::Idle;
        self.request_next(tree, vec![(req.relay, 0.0, 0, 0.0)], 0);
    }

    /// Our parent in `tree` went away: forget the bridge.
    pub(super) fn nat_parent_lost(&mut self, tree: TreeId, addr: PeerAddr) {
        if self.nat.bridged_parent.get(&tree.0) == Some(&addr) {
            self.nat.bridged_parent.remove(&tree.0);
        }
    }

    // ------------------------------------------------------------------ bridges: the relay

    /// Upload committed to bridges, kbps.
    pub(super) fn bridge_load_kbps(&self) -> u32 {
        self.nat.bridges.values().map(|b| b.kbps).sum()
    }

    /// Upload a bridge in `tree` costs, kbps: the stripe times Ω.
    fn bridge_cost_kbps(&self, tree: TreeId) -> u32 {
        let b_m = self
            .tree_info(tree)
            .map(|t| t.bitrate_kbps as u32)
            .unwrap_or(0);
        let omega = (1.0 + self.params.default_parity as f64 / bs_wire::consts::K_BLOCK as f64)
            * (1.0 + self.params.f_frame);
        (b_m as f64 * omega) as u32
    }

    /// Upload promised to proposals accepted but not yet bound, kbps.
    fn pending_bridge_kbps(&self) -> u32 {
        self.nat
            .pending_bridges
            .iter()
            .map(|(_, _, t)| self.bridge_cost_kbps(*t))
            .sum()
    }

    /// Whether one more bridge in `tree` fits the bridge budget (Ch6 §6.3.1).
    fn bridge_affordable(&self, tree: TreeId) -> bool {
        let budget = self.params.bridge_budget_share
            * (1.0 - self.params.r_pull)
            * self.cfg.upload_kbps as f64;
        let committed = self.bridge_load_kbps() + self.pending_bridge_kbps();
        (committed + self.bridge_cost_kbps(tree)) as f64 <= budget
    }

    /// Bridges served in `tree`.
    pub(super) fn bridge_count(&self, tree: TreeId) -> u16 {
        self.nat
            .bridges
            .keys()
            .filter(|(_, t)| *t == tree.0)
            .count() as u16
    }

    /// Whether `from` holds a bridge slot in `tree`.
    pub fn is_bridge_child(&self, from: PeerAddr, tree: TreeId) -> bool {
        self.nat.bridges.contains_key(&(from, tree.0))
    }

    /// Whether this node is bridged in any tree.
    pub fn is_bridged(&self) -> bool {
        !self.nat.bridged_parent.is_empty()
    }

    /// RELAY_PROPOSAL from a NAT-blocked peer (Ch6 §6.3.1).
    pub(super) fn on_relay_proposal(&mut self, from: PeerAddr, p: RelayProposal) {
        if p.bind_flags & BIND_SOURCE_INGRESS != 0 {
            self.on_ingress_proposal(from);
            return;
        }
        let Some(client) = self.sessions.get(&from).and_then(|s| s.node_id) else {
            return;
        };
        let tree = p.tree_id;
        let idx = tree.index();
        let cost = self.bridge_cost_kbps(tree);
        // RULE: Bridges take at most half of the tree budget (Ch6 §6.3.1). A relay
        //       without a slot in the tree binds nothing.
        let affordable = self.relay_capable()
            && idx < self.trees.len()
            && tree.is_tree()
            && cost > 0
            && self.bridge_affordable(tree);
        if !affordable {
            self.send_bind(from, client, tree, true);
            return;
        }
        if self.trees[idx].parent.is_some() && self.trees[idx].verified_segments > 0 {
            self.bind_bridge(from, client, tree, cost);
            return;
        }
        // We do not hold this tree yet: join it, then bind.
        self.nat.pending_bridges.push((client, from, tree));
        if !self.trees[idx].bridging {
            self.trees[idx].bridging = true;
            if !self.trees[idx].join.is_active() && self.trees[idx].parent.is_none() {
                self.begin_probing(tree);
            }
        }
    }

    fn bind_bridge(&mut self, from: PeerAddr, client: NodeId, tree: TreeId, kbps: u32) {
        self.nat.bridges.insert(
            (from, tree.0),
            Bridge {
                client,
                kbps,
                bound_at: self.now,
            },
        );
        self.recompute_slots();
        self.emit(Event::BridgeServed {
            tree: tree.0,
            client,
        });
        self.send_bind(from, client, tree, false);
    }

    fn send_bind(&mut self, to: PeerAddr, client: NodeId, tree: TreeId, reject: bool) {
        let parent = self
            .trees
            .get(tree.index())
            .and_then(|t| t.parent.as_ref().map(|p| p.node_id));
        let b = RelayBind {
            client_a: client,
            client_b: parent.unwrap_or(NodeId::ZERO),
            tree_id: tree,
            bind_flags: if reject { BIND_REJECTED } else { 0 },
        };
        self.send(to, Channel::Control, Frame::RelayBind(b), None);
    }

    /// We attached in `tree`: bind the proposals that waited for it.
    pub(super) fn nat_attached(&mut self, tree: TreeId) {
        let pending: Vec<(NodeId, PeerAddr, TreeId)> = self
            .nat
            .pending_bridges
            .iter()
            .filter(|(_, _, t)| *t == tree)
            .copied()
            .collect();
        self.nat.pending_bridges.retain(|(_, _, t)| *t != tree);
        for (client, addr, t) in pending {
            // RULE: Check the budget again. Proposals queued while we had no parent
            //       can exceed it by the time the tree is held.
            if self.bridge_affordable(t) {
                let cost = self.bridge_cost_kbps(t);
                self.bind_bridge(addr, client, t, cost);
            } else {
                self.send_bind(addr, client, t, true);
            }
        }
    }

    /// A bridge child left: release its budget.
    pub(super) fn nat_child_removed(&mut self, addr: PeerAddr, tree: TreeId) {
        if self.nat.bridges.remove(&(addr, tree.0)).is_some() {
            if self.bridge_count(tree) == 0 && !self.trees[tree.index()].subscribed {
                self.trees[tree.index()].bridging = false;
            }
            self.recompute_slots();
        }
    }

    /// Ask for a bridge in `tree` now (a test hook; silent rounds decide this in production).
    #[doc(hidden)]
    pub fn force_bridge(&mut self, tree: TreeId) {
        self.request_bridge(tree);
    }

    /// Counters for reports.
    pub fn nat_counters(&self) -> (u64, u64, u64, u64) {
        (
            self.nat.punches_sent,
            self.nat.punches_forwarded,
            self.nat.bridges_bound,
            self.nat.bridges_rejected,
        )
    }

    /// Our reachability.
    pub fn reachability(&self) -> Reachability {
        self.reach
    }
}
