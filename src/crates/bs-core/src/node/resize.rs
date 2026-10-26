//! Dynamic forest resize and layer fold (Ch1 §1.2.4 §4.5, Ch1 §1.1.5 §5.6).
//!
//! The source announces a new slicing matrix with `MANIFEST_UPDATE` five segments
//! ahead; every peer applies the announcement at once (new trees join, moving relays
//! drain) and switches its matrix at `EffectiveSegmentSeq`.

use std::collections::VecDeque;

use bs_crypto::{rendezvous, Signer};
use bs_media::{Ladder, SlicingMatrix};
use bs_wire::frames::{
    DrainNotice, DrainScope, ManifestBody, ManifestRequest, ManifestSelector, ManifestUpdate,
    StoreRecordAck,
};
use bs_wire::{
    DisconnectReason, Frame, NodeClass, SegmentSeq, SignatureBytes, TreeId, TreeMappingEntry,
};

use super::Node;
use crate::forest::TreeState;
use crate::io::{Channel, Event, PeerAddr};
use crate::join::JoinStats;
use crate::time::{Duration, Instant};

/// A `MANIFEST_UPDATE` accepted and not yet in force.
#[derive(Debug, Clone)]
pub struct PendingUpdate {
    /// The matrix that takes effect.
    pub matrix: SlicingMatrix,
    /// First segment emitted on the new layout.
    pub effective: SegmentSeq,
    /// The frame, re-sent unchanged to children and on request.
    pub frame: Option<ManifestUpdate>,
    /// Forest size before the switch.
    pub old_m: u8,
}

/// The source's forest control state.
#[derive(Debug, Default)]
pub struct ForestControl {
    /// When the last matrix change was announced.
    pub last_change: Option<Instant>,
    /// Layers removed from the top by folds.
    pub folded: u8,
    /// Relay count when the last fold was announced.
    pub relays_at_fold: u32,
    /// Starved-share estimates per tree from the acknowledgements, with their time.
    starved: VecDeque<(Instant, [f64; 8])>,
    /// Samples before this segment belong to the previous layout and are ignored.
    measure_from: SegmentSeq,
    /// Counters.
    pub updates_announced: u64,
    /// Counters.
    pub switches: u64,
    /// The last update that took effect, for peers that ask for it afterwards.
    pub last_update: Option<ManifestUpdate>,
    /// When we last asked a peer for the update (rate limit).
    pub update_requested_at: Option<Instant>,
}

impl Node {
    // ------------------------------------------------------------------ lookups

    /// Mapping entry for `id` in the matrix in force, else in the pending one.
    pub(super) fn tree_info(&self, id: TreeId) -> Option<bs_media::TreeInfo> {
        self.matrix
            .as_ref()
            .and_then(|m| m.tree(id).copied())
            .or_else(|| {
                self.pending_update
                    .as_ref()
                    .and_then(|p| p.matrix.tree(id).copied())
            })
    }

    /// Ensure `self.trees` has a state for every id up to `m`.
    fn grow_trees(&mut self, m: u8) {
        while self.trees.len() < m as usize {
            let id = TreeId(self.trees.len() as u8 + 1);
            self.trees.push(TreeState::new(id));
            self.join_stats.push(JoinStats::default());
        }
    }

    // ------------------------------------------------------------------ source

    /// Note the guardians' starvation counts from one record store (Ch1 §1.1.5 §5.6).
    pub(super) fn forest_note_acks(&mut self, acks: &[StoreRecordAck]) {
        if acks.is_empty() {
            return;
        }
        let s = self.publisher.as_ref().map(|p| p.sample_log2).unwrap_or(0);
        let mut active: Vec<u32> = acks.iter().map(|a| a.active_count).collect();
        let n_hat = (median_u32(&mut active).unwrap_or(0) as f64) * (1u64 << s.min(16)) as f64;
        // WHY: The spec's median assumes queries spread over the k guardians. Ours go
        //      to the α record holders, so most guardians report zero. The sum over
        //      guardians divided by α counts each querier once either way.
        let alpha = self.params.dht_alpha.max(1) as f64;
        let mut est = [0f64; 8];
        let n_trees = acks[0].starved.len();
        for (m, e) in est.iter_mut().enumerate().take(n_trees) {
            let total: u32 = acks.iter().map(|a| a.starved[m] as u32).sum();
            *e = if n_hat > 0.0 {
                (total as f64 / (alpha * n_hat)).min(1.0)
            } else {
                0.0
            };
        }
        // RULE: A fold is judged on the layout in force after its fill-in window.
        //       Starvation left over from the previous layout is not a second fold.
        if self.swarm.live_edge < self.forest.measure_from {
            return;
        }
        self.forest.starved.push_back((self.now, est));
        let keep = self.params.tau_fold.max(self.params.tau_fold_base);
        while self
            .forest
            .starved
            .front()
            .map(|(t, _)| self.now.duration_since(*t) > keep)
            .unwrap_or(false)
        {
            self.forest.starved.pop_front();
        }
    }

    /// Mean starved share of tree `m` over `window`.
    fn starved_mean(&self, m: usize, window: Duration) -> f64 {
        let pts: Vec<f64> = self
            .forest
            .starved
            .iter()
            .filter(|(t, _)| self.now.duration_since(*t) <= window)
            .map(|(_, e)| e[m])
            .collect();
        if pts.is_empty() {
            0.0
        } else {
            pts.iter().sum::<f64>() / pts.len() as f64
        }
    }

    /// The forest size the ladder wants, with hysteresis (Ch1 §1.2.1 §1.4).
    fn forest_target(&self, m_cur: u8) -> u8 {
        let thr = |m: u8| -> u32 {
            self.params
                .forest_ladder_relays
                .get(m as usize)
                .copied()
                .unwrap_or(u32::MAX)
        };
        let n = self.relay_count;
        let mut m = m_cur.max(2);
        while m < 6 && n >= thr(m + 1) {
            m += 1;
        }
        // RULE: Shrink only below the step's threshold minus 3 relays: half a ladder
        //       step of slack against flapping.
        while m > 2 && n < thr(m).saturating_sub(3) {
            m -= 1;
        }
        m
    }

    /// Source: decide a resize, fold or unfold after a record store.
    pub(super) fn forest_consider(&mut self) {
        if !self.params.dynamic_forest || self.pending_update.is_some() {
            return;
        }
        let Some(p) = self.publisher.as_ref() else {
            return;
        };
        let m_cur = p.matrix.m();
        let layers_total = p.ladder.len() as u8;
        let layers_now = layers_total - self.forest.folded;
        let dwell = self
            .forest
            .last_change
            .map(|t| self.now.duration_since(t) < self.params.tau_forest)
            .unwrap_or(false);
        // Resize by the ladder.
        let target = self.forest_target(m_cur);
        if target != m_cur {
            let next_thr = self
                .params
                .forest_ladder_relays
                .get(m_cur as usize + 1)
                .copied()
                .unwrap_or(u32::MAX);
            // RULE: Growth pre-empts the dwell only when the relay count reached twice
            //       the next threshold (flash crowd). Shrinks never pre-empt it.
            let flash = target > m_cur && self.relay_count >= next_thr.saturating_mul(2);
            if !dwell || flash {
                self.forest_announce(target, layers_now);
                return;
            }
        }
        if dwell {
            return;
        }
        // Fold: a layer below the top starves for τ_fold.
        if layers_now > 1 {
            let top = layers_now - 1;
            let starving = p.matrix.trees.iter().any(|t| {
                t.layer < top && {
                    let window = if t.layer == 0 {
                        self.params.tau_fold_base
                    } else {
                        self.params.tau_fold
                    };
                    self.starved_mean(t.tree_id.index(), window) > self.params.s_fold
                }
            });
            if starving {
                self.forest.folded += 1;
                self.forest.relays_at_fold = self.relay_count;
                self.forest_announce(m_cur, layers_now - 1);
                return;
            }
        }
        // Unfold only on relay growth (Ch1 §1.1.5 §5.6).
        if self.forest.folded > 0
            && (self.relay_count as f64)
                >= self.params.unfold_growth * self.forest.relays_at_fold as f64
        {
            self.forest.folded -= 1;
            self.forest.relays_at_fold = self.relay_count;
            self.forest_announce(m_cur, layers_now + 1);
        }
    }

    /// Source: sign and broadcast a `MANIFEST_UPDATE` for `m` trees over `layers`.
    fn forest_announce(&mut self, m: u8, layers: u8) {
        let Some(p) = self.publisher.as_mut() else {
            return;
        };
        let ladder = Ladder {
            layers: p.ladder.layers[..layers as usize].to_vec(),
        };
        let version = p.matrix.version.wrapping_add(1).max(1);
        let Ok(matrix) = SlicingMatrix::allocate(version, m, &ladder) else {
            return;
        };
        let Some(mut body) = p.last_body.clone() else {
            return;
        };
        body.slicing_matrix_version = version;
        let effective = SegmentSeq(p.last_segment.0 + self.params.resize_window_segments);
        let mut frame = ManifestUpdate {
            body,
            effective_segment: effective,
            trees: matrix.to_entries(),
            signature: SignatureBytes::ZERO,
        };
        frame.signature = self.identity.sign(&frame.signable_bytes());
        p.pending = Some((matrix.to_entries(), effective));
        let old_m = p.matrix.m();
        self.forest.last_change = Some(self.now);
        self.forest.updates_announced += 1;
        self.emit(Event::ForestUpdateAnnounced {
            version,
            m,
            layers,
            effective: effective.0,
        });
        self.pending_update = Some(PendingUpdate {
            matrix,
            effective,
            frame: Some(frame.clone()),
            old_m,
        });
        self.broadcast_update(&frame, None);
        self.apply_announcement();
        self.store_record();
    }

    /// Send the update once to every non-pending child.
    fn broadcast_update(&mut self, frame: &ManifestUpdate, except: Option<PeerAddr>) {
        let children: std::collections::BTreeSet<PeerAddr> = self
            .trees
            .iter()
            .flat_map(|t| t.children.values().filter(|c| !c.pending).map(|c| c.addr))
            .collect();
        for c in children {
            if Some(c) != except {
                self.send(
                    c,
                    Channel::Control,
                    Frame::ManifestUpdate(frame.clone()),
                    None,
                );
            }
        }
    }

    // ------------------------------------------------------------------ receiving

    /// A `MANIFEST_UPDATE` from a parent or neighbour.
    pub(super) fn on_manifest_update(&mut self, from: PeerAddr, u: ManifestUpdate) {
        let Some(pk) = self.publisher_key else { return };
        if u.body.stream_id != self.stream_id()
            || bs_crypto::Verifier::verify(&pk, &u.signable_bytes(), &u.signature).is_err()
        {
            self.emit(Event::Note {
                text: "manifest update rejected (signature/stream id)".into(),
            });
            return;
        }
        let version = u.body.slicing_matrix_version;
        if !self.update_version_is_new(version) {
            return;
        }
        let matrix = matrix_from_entries(version, &u.trees);
        self.install_pending(matrix, u.effective_segment, Some(u.clone()));
        self.broadcast_update(&u, Some(from));
    }

    /// Whether `version` is newer than the matrix in force and any pending one.
    fn update_version_is_new(&self, version: u8) -> bool {
        let cur = self.matrix.as_ref().map(|m| m.version).unwrap_or(0);
        let pend = self.pending_update.as_ref().map(|p| p.matrix.version);
        // RULE: Versions are strictly increasing (replay protection, Ch1 §1.2.4 §4.5).
        version > cur && pend.map(|v| version > v).unwrap_or(true)
    }

    /// A pending matrix learned from the Stream Record (joined inside a window).
    pub(super) fn pending_from_record(&mut self, rec: &bs_wire::StreamRecord) {
        if rec.trees_next.is_empty() || !rec.effective_segment.is_some() {
            return;
        }
        if self.pending_update.is_some() {
            return;
        }
        // The record names no version: 0 until the first manifest cut under it.
        let matrix = matrix_from_entries(0, &rec.trees_next);
        self.install_pending(matrix, rec.effective_segment, None);
    }

    fn install_pending(
        &mut self,
        matrix: SlicingMatrix,
        effective: SegmentSeq,
        frame: Option<ManifestUpdate>,
    ) {
        let old_m = self.matrix.as_ref().map(|m| m.m()).unwrap_or(0);
        self.emit(Event::ForestUpdateAnnounced {
            version: matrix.version,
            m: matrix.m(),
            layers: matrix.layer_count,
            effective: effective.0,
        });
        self.pending_update = Some(PendingUpdate {
            matrix,
            effective,
            frame,
            old_m,
        });
        self.apply_announcement();
        // Already past the switch (late joiner): switch now.
        if self.swarm.live_edge.is_some() && self.swarm.live_edge >= effective {
            self.forest_switch();
        }
    }

    /// Ask `from` for the pending update after a manifest with an unknown version.
    pub(super) fn request_pending_update(&mut self, from: PeerAddr) {
        if self.pending_update.is_some() {
            return;
        }
        if let Some(t) = self.forest.update_requested_at {
            if self.now.duration_since(t) < Duration::from_millis(500) {
                return;
            }
        }
        self.forest.update_requested_at = Some(self.now);
        let r = ManifestRequest {
            segment: SegmentSeq::NONE,
            selector: ManifestSelector::PendingUpdate,
        };
        self.send(from, Channel::Control, Frame::ManifestRequest(r), None);
    }

    /// Answer a `MANIFEST_REQUEST(0xFE)`: the pending update, else the last one in
    /// force, so a peer that missed the announcement still reaches the layout.
    pub(super) fn send_pending_update(&mut self, to: PeerAddr) {
        let f = self
            .pending_update
            .as_ref()
            .and_then(|p| p.frame.clone())
            .or_else(|| self.forest.last_update.clone());
        if let Some(f) = f {
            self.send(to, Channel::Control, Frame::ManifestUpdate(f), None);
        }
    }

    // ------------------------------------------------------------------ window

    /// Apply an announcement: new trees exist and are joined, movers drain.
    fn apply_announcement(&mut self) {
        let Some(p) = self.pending_update.clone() else {
            return;
        };
        let new_m = p.matrix.m();
        let old_m = p.old_m;
        self.grow_trees(new_m.max(old_m));
        let grace = p.effective.0 + self.params.d_max as u32;
        if self.is_source() {
            for t in &mut self.trees {
                if t.id.0 > old_m {
                    t.assigned = true;
                    t.subscribed = true;
                    t.depth = 0;
                }
            }
            self.recompute_slots();
            self.register_self();
            return;
        }
        // Relay reassignment (Ch1 §1.2.4 §4.5 steps 2 and 3).
        if self.cfg.node_class == NodeClass::Relay {
            let current = self.trees.iter().find(|t| t.assigned).map(|t| t.id);
            let next = rendezvous::assigned_tree(&self.node_id(), new_m);
            if current != Some(next) {
                if let Some(old) = current {
                    self.drain_for_reassignment(old, p.effective);
                    self.emit(Event::RelayReassigned {
                        from: old.0,
                        to: next.0,
                    });
                }
                self.trees[next.index()].assigned = true;
                self.join_stats[next.index()].grace_until = grace;
            }
        }
        // Trees newly needed by the subscription are joined from now on.
        let top = self.top_layer.min(p.matrix.layer_count.saturating_sub(1));
        let want = p.matrix.trees_for_layer_prefix(top);
        for t in &mut self.trees {
            if want.contains(t.id) && !t.subscribed {
                t.subscribed = true;
            }
        }
        // RULE: Failed rounds do not count toward a shed in the fill-in window
        //       (Ch1 §1.1.5 §5.3 item 6). Every tree is in the fill-in. A tree with
        //       an unchanged entry still loses parents to relays that move.
        for js in &mut self.join_stats {
            js.grace_until = grace;
        }
        self.recompute_slots();
        self.register_self();
        let ids: Vec<TreeId> = self
            .trees
            .iter()
            .filter(|t| t.wants_parent() && t.parent.is_none() && !t.join.is_active())
            .map(|t| t.id)
            .collect();
        for id in ids {
            self.begin_probing(id);
        }
    }

    /// A moving relay drains its old tree's children until the switch.
    fn drain_for_reassignment(&mut self, old: TreeId, effective: SegmentSeq) {
        let idx = old.index();
        self.trees[idx].reassigning = true;
        let addrs: Vec<PeerAddr> = self.trees[idx]
            .children
            .values()
            .filter(|c| c.draining_until.is_none())
            .map(|c| c.addr)
            .collect();
        for a in addrs {
            let d = DrainNotice {
                tree_id: old,
                reason: DisconnectReason::Reassigned,
                scope: DrainScope::AllChildren,
                deadline: effective,
            };
            self.send(a, Channel::Tree(old), Frame::DrainNotice(d), None);
            if let Some(c) = self.trees[idx].children.get_mut(&a) {
                c.draining_until = Some(effective);
            }
        }
    }

    /// Switch before a manifest cut under the pending matrix is admitted.
    pub(super) fn maybe_switch_for(&mut self, body: &ManifestBody) {
        let due = self
            .pending_update
            .as_ref()
            .map(|p| body.segment >= p.effective)
            .unwrap_or(false);
        if due {
            self.forest_switch();
        }
    }

    /// Switch if the live edge reached the effective segment.
    pub(super) fn forest_timers(&mut self) {
        let due = self
            .pending_update
            .as_ref()
            .map(|p| self.swarm.live_edge.is_some() && self.swarm.live_edge >= p.effective)
            .unwrap_or(false);
        if due {
            self.forest_switch();
        }
    }

    /// The switch (Ch1 §1.2.4 §4.5 step 5): the pending matrix takes effect.
    pub(super) fn forest_switch(&mut self) {
        let Some(p) = self.pending_update.take() else {
            return;
        };
        let new_m = p.matrix.m();
        let layer_count = p.matrix.layer_count;
        self.matrix = Some(p.matrix.clone());
        if let Some(pb) = self.publisher.as_mut() {
            pb.matrix = p.matrix.clone();
            pb.pending = None;
            pb.set_matrix_version(p.matrix.version);
        }
        self.forest.switches += 1;
        if p.frame.is_some() {
            self.forest.last_update = p.frame.clone();
        }
        self.forest.starved.clear();
        self.forest.measure_from = SegmentSeq(p.effective.0 + self.params.d_max as u32);
        self.top_layer = self.top_layer.min(layer_count.saturating_sub(1));
        // Trees that no longer exist close now.
        let gone: Vec<TreeId> = self
            .trees
            .iter()
            .filter(|t| t.id.0 > new_m)
            .map(|t| t.id)
            .collect();
        for id in gone {
            self.leave_tree(id, DisconnectReason::Reassigned);
        }
        self.trees.truncate(new_m as usize);
        self.join_stats.truncate(new_m as usize);
        if self.is_source() {
            for t in &mut self.trees {
                t.assigned = true;
                t.subscribed = true;
                t.depth = 0;
            }
        } else {
            let before: Vec<(bool, bool)> = self
                .trees
                .iter()
                .map(|t| (t.assigned, t.subscribed))
                .collect();
            self.configure_tree_roles();
            for (i, (a, s)) in before.into_iter().enumerate() {
                let t = &mut self.trees[i];
                t.reassigning = false;
                let id = t.id;
                if (a || s) && !t.wants_parent() {
                    self.leave_tree(id, DisconnectReason::Reassigned);
                } else if a && !t.assigned {
                    // Still subscribed, no longer the relay: the children go, the
                    // parent stays.
                    self.disconnect_children(id, DisconnectReason::Reassigned);
                }
            }
        }
        for js in &mut self.join_stats {
            js.failed_rounds = 0;
            js.warming_rounds = 0;
        }
        self.recompute_slots();
        self.register_self();
        self.emit(Event::ForestSwitched {
            version: self.matrix.as_ref().map(|m| m.version).unwrap_or(0),
            m: new_m,
            layers: layer_count,
        });
        if self.is_source() {
            self.store_record();
            return;
        }
        let ids: Vec<TreeId> = self
            .trees
            .iter()
            .filter(|t| t.wants_parent() && t.parent.is_none() && !t.join.is_active())
            .map(|t| t.id)
            .collect();
        for id in ids {
            self.begin_probing(id);
        }
    }

    /// Disconnect every child of `id` with `reason`.
    fn disconnect_children(&mut self, id: TreeId, reason: DisconnectReason) {
        let idx = id.index();
        let children: Vec<PeerAddr> = self.trees[idx].children.keys().copied().collect();
        for c in children {
            let f = bs_wire::frames::Disconnect {
                sender: self.node_id(),
                reason,
                tree_id: id,
            };
            self.send(c, Channel::Tree(id), Frame::Disconnect(f), None);
            self.remove_child_from_tree(id, c, "forest switch");
        }
    }

    /// Leave `id`: release the parent, disconnect the children, forget the join.
    fn leave_tree(&mut self, id: TreeId, reason: DisconnectReason) {
        let idx = id.index();
        if idx >= self.trees.len() {
            return;
        }
        self.disconnect_children(id, reason);
        let t = &mut self.trees[idx];
        t.join = crate::join::JoinState::Idle;
        t.repairing = false;
        t.lost_at = None;
        t.subscribed = false;
        t.assigned = false;
        let parents: Vec<PeerAddr> = t
            .detach_parent(self.now)
            .into_iter()
            .chain(t.old_parent.take())
            .map(|p| p.addr)
            .collect();
        t.lost_at = None;
        self.join_stats[idx] = JoinStats::default();
        for p in parents {
            let f = bs_wire::frames::Disconnect {
                sender: self.node_id(),
                reason: DisconnectReason::Quit,
                tree_id: id,
            };
            self.send(p, Channel::Tree(id), Frame::Disconnect(f), None);
            self.maybe_close_session(p);
        }
    }

    /// Pre-emission (Ch1 §1.2.4 §4.5 step 4).
    ///
    /// A block that the pending layout places on a new tree is also pushed down that
    /// tree, so its relays warm up before the switch.
    pub(super) fn preemit_block(&mut self, vb: &crate::swarm::VerifiedBlock) {
        let Some(p) = self.pending_update.as_ref() else {
            return;
        };
        let Some(body) = self
            .swarm
            .chunk(vb.segment, vb.index.chunk())
            .map(|c| c.manifest.clone())
        else {
            return;
        };
        let Some(t) = p.matrix.tree_for_block(&body, vb.index.j()) else {
            return;
        };
        if t.0 <= p.old_m || t == vb.tree {
            return;
        }
        let mut dup = vb.clone();
        dup.tree = t;
        self.forward_block(&dup);
    }

    /// Whether a failed join round on `tree` is inside the resize grace window.
    pub(super) fn in_resize_grace(&self, tree: TreeId) -> bool {
        let g = self.join_stats[tree.index()].grace_until;
        g > 0 && (self.swarm.live_edge.0 < g)
    }

    /// Announce a layout of `m` trees over the lowest `layers` layers now.
    ///
    /// A test hook: the ladder and the fold rule decide this in production.
    #[doc(hidden)]
    pub fn force_forest(&mut self, m: u8, layers: u8) {
        if self.publisher.is_some() && self.pending_update.is_none() {
            self.forest_announce(m, layers);
        }
    }

    /// Facts for reports: (updates announced, switches).
    pub fn forest_counters(&self) -> (u64, u64) {
        (self.forest.updates_announced, self.forest.switches)
    }

    /// The pending update, if any.
    pub fn pending_update(&self) -> Option<&PendingUpdate> {
        self.pending_update.as_ref()
    }
}

/// A matrix from wire entries; the layer count is the highest layer named plus one.
fn matrix_from_entries(version: u8, entries: &[TreeMappingEntry]) -> SlicingMatrix {
    let layers = entries.iter().map(|t| t.layer + 1).max().unwrap_or(1);
    SlicingMatrix::from_entries(version, entries, layers)
}

fn median_u32(v: &mut [u32]) -> Option<u32> {
    if v.is_empty() {
        return None;
    }
    v.sort_unstable();
    Some(v[v.len() / 2])
}
