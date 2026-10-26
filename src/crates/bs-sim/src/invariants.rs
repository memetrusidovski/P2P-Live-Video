//! Protocol invariants checked at every step. A hard violation halts the run
//! and the report carries the offending node's recent events.

use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;

use bs_core::{Channel, Event, Instant, Node};
use bs_wire::Frame;
use serde::{Deserialize, Serialize};

use crate::report::EventLog;
use crate::sim::SimNode;

/// One violation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Violation {
    /// When.
    pub t_us: u64,
    /// Node index.
    pub node: usize,
    /// Rule name.
    pub rule: String,
    /// Details.
    pub detail: String,
    /// Hard (halts) or soft (counted).
    pub hard: bool,
}

/// The checker.
pub struct Invariants {
    d_max: u8,
    /// Violations so far.
    pub violations: Vec<Violation>,
    halted: bool,
    /// Halt on the first hard violation.
    pub halt_on_hard: bool,
    manifest_sent: HashSet<(usize, SocketAddr, u32, u8)>,
    live_edge_prev: HashMap<usize, u32>,
    soft_counts: HashMap<String, u64>,
}

impl Invariants {
    /// New checker.
    pub fn new(d_max: u8) -> Self {
        Self {
            d_max,
            violations: Vec::new(),
            halted: false,
            halt_on_hard: true,
            manifest_sent: HashSet::new(),
            live_edge_prev: HashMap::new(),
            soft_counts: HashMap::new(),
        }
    }
    /// Whether a hard violation halted the run.
    pub fn halted(&self) -> bool {
        self.halted
    }
    /// Soft violation counts by rule.
    pub fn soft_counts(&self) -> &HashMap<String, u64> {
        &self.soft_counts
    }
    /// Record a violation.
    pub fn violation(
        &mut self,
        now: Instant,
        node: usize,
        rule: &str,
        detail: String,
        hard: bool,
        log: &mut EventLog,
    ) {
        if !hard {
            *self.soft_counts.entry(rule.to_string()).or_insert(0) += 1;
            if self.soft_counts[rule] > 20 {
                return; // keep the report readable
            }
        }
        log.note(now, node, &format!("INVARIANT {rule}: {detail}"));
        self.violations.push(Violation {
            t_us: now.as_micros(),
            node,
            rule: rule.into(),
            detail,
            hard,
        });
        if hard && self.halt_on_hard {
            self.halted = true;
        }
    }

    /// Check what a node is about to send.
    pub fn on_send(
        &mut self,
        now: Instant,
        i: usize,
        node: &Node,
        frame: &Frame,
        _channel: Channel,
        to: SocketAddr,
        log: &mut EventLog,
    ) {
        match frame {
            Frame::BlockProof(bp) => {
                if !node.swarm().is_verified(bp.segment, bp.block) {
                    self.violation(
                        now,
                        i,
                        "verify_before_forward",
                        format!(
                            "BLOCK_PROOF for unverified block {} of {}",
                            bp.block, bp.segment
                        ),
                        true,
                        log,
                    );
                }
                if !self
                    .manifest_sent
                    .contains(&(i, to, bp.segment.0, bp.block.chunk()))
                {
                    self.violation(
                        now,
                        i,
                        "manifest_before_blocks",
                        format!(
                            "BLOCK_PROOF {} {} to {to} before its MANIFEST",
                            bp.segment, bp.block
                        ),
                        false,
                        log,
                    );
                }
            }
            Frame::RaptorQSymbol(s) => {
                if !node.swarm().is_verified(s.segment, s.block) {
                    self.violation(
                        now,
                        i,
                        "verify_before_forward",
                        format!("symbol for unverified block {} of {}", s.block, s.segment),
                        true,
                        log,
                    );
                }
            }
            Frame::Manifest(m) => {
                self.manifest_sent
                    .insert((i, to, m.body.segment.0, m.body.chunk_index));
                if self.manifest_sent.len() > 200_000 {
                    let floor = m.body.segment.0.saturating_sub(20);
                    self.manifest_sent.retain(|(_, _, s, _)| *s >= floor);
                }
            }
            _ => {}
        }
    }

    /// Check an event.
    pub fn on_event(&mut self, now: Instant, i: usize, e: &Event, log: &mut EventLog) {
        if let Event::ParentAttached { depth, tree, .. } = e {
            if *depth > self.d_max {
                self.violation(
                    now,
                    i,
                    "depth_bound",
                    format!(
                        "attached at depth {depth} > D_max {} in tree {tree}",
                        self.d_max
                    ),
                    true,
                    log,
                );
            }
        }
    }

    /// Structural checks over the whole swarm.
    pub fn snapshot(
        &mut self,
        now: Instant,
        nodes: &[SimNode],
        index: &HashMap<SocketAddr, usize>,
        log: &mut EventLog,
    ) {
        // (parent, child) edges per tree; non-source parents must be distinct across trees.
        let mut edges: HashMap<(SocketAddr, SocketAddr), Vec<u8>> = HashMap::new();
        for (i, n) in nodes.iter().enumerate() {
            if !n.alive {
                continue;
            }
            // Live edge monotone.
            let le = n.node.swarm().live_edge.0;
            let prev = *self.live_edge_prev.get(&i).unwrap_or(&0);
            if le < prev {
                self.violation(
                    now,
                    i,
                    "live_edge_monotone",
                    format!("live edge went from {prev} to {le}"),
                    true,
                    log,
                );
            }
            self.live_edge_prev.insert(i, le);
            // DHT: only reachable relay-class contacts live in k-buckets (Ch2 §2.3.2 rule 1).
            for c in n.node.dht_contact_records() {
                if c.node_class != bs_wire::NodeClass::Relay
                    || c.flags.reachability == bs_wire::Reachability::Symmetric
                {
                    self.violation(
                        now,
                        i,
                        "dht_contact_class",
                        format!(
                            "routing table holds {:?} {:?} contact {}",
                            c.node_class,
                            c.flags.reachability,
                            c.node_id.short()
                        ),
                        true,
                        log,
                    );
                }
            }
            // HyParView: the Active Set never exceeds its target (Ch3 §3.1.3 hard cap).
            if n.node.active_set_len() > n.node.active_set_target() {
                self.violation(
                    now,
                    i,
                    "active_set_cap",
                    format!(
                        "active set {} > target {}",
                        n.node.active_set_len(),
                        n.node.active_set_target()
                    ),
                    false,
                    log,
                );
            }
            for t in n.node.trees() {
                // K_avail never exceeds K_v (children ≤ K_v) — the source may exceed
                // only via the base-layer reserve (M3), so it is hard for everyone in M1.
                let draining = t
                    .children
                    .values()
                    .filter(|c| c.draining_until.is_some() || c.pending)
                    .count();
                if (t.children.len() - draining) as u16 > t.k_v {
                    self.violation(
                        now,
                        i,
                        "slot_bound",
                        format!(
                            "tree {} has {} children ({} draining) > K_v {}",
                            t.id,
                            t.children.len(),
                            draining,
                            t.k_v
                        ),
                        true,
                        log,
                    );
                }
                if let Some(p) = &t.parent {
                    // A link the parent drains is on its way out (a relay that moves at
                    // a resize). It is not a second edge to the same child.
                    let draining = index
                        .get(&p.addr)
                        .and_then(|&pi| nodes[pi].node.trees().get(t.id.index()))
                        .and_then(|pt| pt.children.get(&n.addr))
                        .map(|c| c.draining_until.is_some())
                        .unwrap_or(false);
                    if !draining {
                        edges.entry((p.addr, n.addr)).or_default().push(t.id.0);
                    }
                    // Parent must relay the tree.
                    if let Some(&pi) = index.get(&p.addr) {
                        let pn = &nodes[pi];
                        if pn.alive {
                            let pt = &pn.node.trees()[t.id.index()];
                            // A bridge is out-degree one in a tree the relay is not
                            // assigned to (Ch6 §6.3.1). A parent that already dropped
                            // the link has a DISCONNECT in flight: not a violation.
                            if !pt.assigned
                                && !pn.node.is_bridge_child(n.addr, t.id)
                                && pt.children.contains_key(&n.addr)
                            {
                                self.violation(
                                    now,
                                    i,
                                    "parent_relays_tree",
                                    format!("parent {} is not assigned to tree {}", pn.label, t.id),
                                    true,
                                    log,
                                );
                            }
                            // Depth consistency: child depth = parent depth + 1 once blocks flow.
                            // An ingress relay is hop 0 like the source it fronts.
                            if t.depth != u8::MAX
                                && pt.depth != u8::MAX
                                && t.depth != pt.depth + 1
                                && !n.node.is_ingress()
                                && p.first_block_at.is_some()
                            {
                                self.violation(
                                    now,
                                    i,
                                    "depth_consistency",
                                    format!(
                                        "depth {} but parent depth {} in tree {}",
                                        t.depth, pt.depth, t.id
                                    ),
                                    false,
                                    log,
                                );
                            }
                        }
                    }
                    // Depth bound: propagated depth may exceed D_max while a branch unwinds
                    // (Ch1 §1.2.2 Depth Propagation drains it); admission above D_max is the
                    // hard rule, checked on ParentAttached.
                    if t.depth != u8::MAX && t.depth > self.d_max {
                        self.violation(
                            now,
                            i,
                            "depth_bound_propagated",
                            format!("depth {} > D_max in tree {}", t.depth, t.id),
                            false,
                            log,
                        );
                    }
                }
            }
        }
        for ((p, c), trees) in &edges {
            if trees.len() > 1 {
                // An ingress relay is a root like the source it fronts (Ch6 §6.3.3).
                let is_source = index
                    .get(p)
                    .map(|&pi| nodes[pi].node.is_source() || nodes[pi].node.is_ingress())
                    .unwrap_or(false);
                // A peer bridged through one relay in several trees has correlated
                // parents by design (Ch6 §6.3.1).
                let bridged = index
                    .get(c)
                    .map(|&ci| nodes[ci].node.is_bridged())
                    .unwrap_or(false);
                if !is_source && !bridged {
                    let ci = index.get(c).copied().unwrap_or(0);
                    self.violation(
                        now,
                        ci,
                        "edge_disjoint",
                        format!("edge {p}→{c} used by trees {trees:?}"),
                        true,
                        log,
                    );
                }
            }
        }
        // Cycles: follow parents per tree.
        for (i, n) in nodes.iter().enumerate() {
            if !n.alive {
                continue;
            }
            for t in n.node.trees() {
                let mut cur = i;
                let mut hops = 0;
                let mut seen = HashSet::new();
                let mut chain: Vec<String> = vec![format!("{i}")];
                loop {
                    let node = &nodes[cur];
                    if node.node.is_source() {
                        break;
                    }
                    let Some(p) = node.node.trees()[t.id.index()].parent.as_ref() else {
                        break;
                    };
                    let Some(&pi) = index.get(&p.addr) else { break };
                    chain.push(format!(
                        "{pi}(d{})",
                        nodes[pi].node.trees()[t.id.index()].depth
                    ));
                    if !seen.insert(pi) || hops > nodes.len() {
                        self.violation(
                            now,
                            i,
                            "tree_cycle",
                            format!("tree {} parent chain cycles: {}", t.id, chain.join(" -> ")),
                            true,
                            log,
                        );
                        break;
                    }
                    cur = pi;
                    hops += 1;
                }
            }
        }
    }
}
