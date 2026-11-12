//! Balance grant (Ch1 §1.2.1 §1.3): a relay takes its second-ranked tree when that
//! tree is short of relays.
//!
//! Rendezvous assignment gives each tree a binomial share of the relays. In a swarm
//! of tens of relays one tree can hold half its share, and its viewers then find no
//! free slot (ISSUE-089). The relays whose second-ranked tree is the short one fill
//! the gap, each with the probability the gap calls for.

use bs_crypto::rendezvous;
use bs_wire::{NodeClass, Reachability, TreeId};

use super::{Lifecycle, Node};
use crate::time::{Duration, Instant};

/// Time between two evaluations of the grant.
const BALANCE_PERIOD: Duration = Duration::from_secs(2);
/// Relay records a node must know before it judges the balance of the trees.
const MIN_SAMPLE: usize = 12;
/// Mean relays per tree a node must know before it judges one tree short.
const MIN_MEAN: f64 = 6.0;
/// Time after a forest resize in which no grant is taken.
const RESIZE_QUIET: Duration = Duration::from_secs(60);
/// A tree is short when its relay count is below this share of the mean.
const SHORT_SHARE: f64 = 0.6;
/// A grant with no child is kept at least this long, so a tree is not left and
/// taken again on every evaluation.
const GRANT_HOLD: Duration = Duration::from_secs(30);

/// Balance-grant state of a node.
#[derive(Debug)]
pub struct BalanceState {
    /// The second-ranked tree this relay serves beside its first-ranked one.
    pub(super) granted: Option<TreeId>,
    granted_at: Instant,
    pub(super) next_round: Instant,
    /// Forest size at the last evaluation.
    last_m: u8,
    /// No grant is taken before this instant: the forest was resized.
    quiet_until: Instant,
}

impl Default for BalanceState {
    fn default() -> Self {
        Self {
            granted: None,
            granted_at: Instant::ZERO,
            next_round: Instant::ZERO,
            last_m: 0,
            quiet_until: Instant::ZERO,
        }
    }
}

impl Node {
    /// The tree this relay serves under a balance grant, if any.
    pub fn granted_tree(&self) -> Option<TreeId> {
        self.balance.granted
    }

    /// Relay records this node knows, itself included, per tree: index `m - 1`.
    fn known_relays_per_tree(&self, m: u8) -> (Vec<usize>, usize) {
        let mut counts = vec![0usize; m as usize];
        let mut relays = 0;
        let own = self.peer_record();
        let records = self
            .passive
            .values()
            .chain(self.membership.active.values().map(|a| &a.record))
            .filter(|r| r.node_id != own.node_id)
            .chain(std::iter::once(&own));
        let mut seen = std::collections::BTreeSet::new();
        for r in records {
            if r.node_class != NodeClass::Relay
                || r.flags.source
                || r.flags.reachability == Reachability::Symmetric
                || !seen.insert(r.node_id)
            {
                continue;
            }
            relays += 1;
            for (i, c) in counts.iter_mut().enumerate() {
                if r.assigned_trees.contains(TreeId(i as u8 + 1)) {
                    *c += 1;
                }
            }
        }
        (counts, relays)
    }

    /// Take or release the balance grant. Runs every `BALANCE_PERIOD`.
    pub(super) fn balance_round(&mut self) {
        if self.now < self.balance.next_round {
            return;
        }
        self.balance.next_round = self.now + BALANCE_PERIOD;
        let Some(matrix) = self.matrix.clone() else {
            return;
        };
        let m = matrix.m();
        if m < 2
            || !self.params.balance_grant
            || self.state != Lifecycle::Active
            || self.is_source()
            || self.is_ingress()
            || self.effective_class() != NodeClass::Relay
        {
            return;
        }
        // RULE: No grant around a resize. The forest ladder sizes M to the relay
        //       count and reassigns relays itself. A grant taken under one M is
        //       undone by the next, and each change costs repairs.
        if self.balance.last_m != m {
            if self.balance.last_m != 0 {
                self.balance.quiet_until = self.now + RESIZE_QUIET;
            }
            self.balance.last_m = m;
        }
        let quiet = self.now < self.balance.quiet_until || self.pending_update.is_some();
        let rank = rendezvous::rendezvous_rank(&self.node_id(), m);
        let (first, second) = (rank[0], rank[1]);
        let (counts, relays) = self.known_relays_per_tree(m);
        let mean = counts.iter().sum::<usize>() as f64 / m as f64;
        let in_second = counts[second.index()] as f64;

        if let Some(g) = self.balance.granted {
            // RULE: The grant lapses when the granted tree holds no child of ours and
            //       is no longer short (Ch1 §1.2.1 §1.3).
            let idle = self.trees[g.index()].children.is_empty()
                && self.now.duration_since(self.balance.granted_at) >= GRANT_HOLD;
            if g != second || (idle && relays >= MIN_SAMPLE && in_second >= mean) {
                self.set_grant(None);
            } else {
                // Slots move to the granted tree as the first tree's children leave.
                self.recompute_slots();
            }
            return;
        }
        // RULE: Trees of a few relays each are uneven by chance alone. Below
        //       `MIN_MEAN` relays per tree the count says nothing about a shortage.
        if quiet || relays < MIN_SAMPLE || mean < MIN_MEAN {
            return;
        }
        // RULE: Only a relay of a tree at or above the mean gives half its slots away.
        if in_second >= SHORT_SHARE * mean || (counts[first.index()] as f64) < mean {
            return;
        }
        // The relay must receive the tree it is to relay.
        let t = &self.trees[second.index()];
        if !t.subscribed || t.parent.is_none() || t.verified_segments == 0 {
            return;
        }
        // Capacity gate: one slot at least in each of the two trees, and spare
        // capacity now for one slot in the granted tree.
        let parity = self.mean_child_parity();
        let slots = |tree: TreeId, trees: u32| {
            matrix
                .tree(tree)
                .map(|i| {
                    self.params
                        .slots(self.cfg.upload_kbps, trees, i.bitrate_kbps, parity)
                })
                .unwrap_or(0)
        };
        if slots(first, 2) < 1 || slots(second, 2) < 1 {
            return;
        }
        let whole = slots(first, 1).max(1);
        let held = self.trees[first.index()]
            .children
            .values()
            .filter(|c| c.draining_until.is_none())
            .count() as u16;
        let spare = 1.0 - held.clamp(slots(first, 2), whole) as f64 / whole as f64;
        if (slots(second, 1) as f64 * spare) < 1.0 {
            return;
        }
        // RULE: Each candidate volunteers with the probability the gap calls for, drawn
        //       from its NodeID. The draw is the same at every evaluation, so the set
        //       of volunteers is stable and a filled gap attracts no further relay.
        let share = 1.0 - in_second / mean;
        let mut seedb = self.node_id().0.to_vec();
        seedb.extend_from_slice(b"balance-grant");
        let h = bs_crypto::blake3_hash(&seedb);
        let mut first8 = [0u8; 8];
        first8.copy_from_slice(&h.0[..8]);
        let draw = u64::from_be_bytes(first8) as f64 / u64::MAX as f64;
        if draw < share {
            self.set_grant(Some(second));
        }
    }

    /// Split the slots of a relay that holds a balance grant between its
    /// first-ranked tree and the granted tree.
    ///
    /// RULE: A grant spends spare capacity only. The first-ranked tree keeps every
    ///       child it has, and its slot count falls to the half share as children
    ///       leave. A grant that drained children would trade one repair per child
    ///       for the balance it buys.
    pub(super) fn balance_split_slots(
        &mut self,
        matrix: &bs_media::SlicingMatrix,
        upload_kbps: u32,
        parity: f64,
        bridge_counts: &[u16],
    ) {
        let Some(granted) = self.balance.granted else {
            return;
        };
        let Some(first) = self
            .trees
            .iter()
            .find(|t| t.assigned && t.id != granted)
            .map(|t| t.id)
        else {
            return;
        };
        let rate = |tree: TreeId| matrix.tree(tree).map(|i| i.bitrate_kbps).unwrap_or(0);
        let (b_first, b_granted) = (rate(first), rate(granted));
        if b_first == 0 || b_granted == 0 {
            return;
        }
        let whole = self.params.slots(upload_kbps, 1, b_first, parity);
        let half = self.params.slots(upload_kbps, 2, b_first, parity);
        let held = self.trees[first.index()]
            .children
            .values()
            .filter(|c| c.draining_until.is_none())
            .count() as u16;
        let own = held.saturating_sub(bridge_counts[first.index()]);
        let k_first = own.clamp(half, whole.max(half));
        // The share of the upload the first tree does not use buys granted slots.
        let spare = 1.0 - k_first as f64 / whole.max(1) as f64;
        let k_granted =
            (self.params.slots(upload_kbps, 1, b_granted, parity) as f64 * spare).floor() as u16;
        self.trees[first.index()].k_v = k_first + bridge_counts[first.index()];
        self.trees[granted.index()].k_v = k_granted + bridge_counts[granted.index()];
    }

    /// Apply a grant change: roles, slots and the registered record follow.
    fn set_grant(&mut self, tree: Option<TreeId>) {
        self.balance.granted = tree;
        self.balance.granted_at = self.now;
        self.configure_tree_roles();
        self.recompute_slots();
        self.reregister_if_needed();
    }
}
