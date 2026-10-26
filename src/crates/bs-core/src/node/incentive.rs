//! Incentives (Ch5): PROOF_OF_UPLOAD receipts and their ledger, the receipt deadline,
//! adaptive per-link parity (Ch4 §4.2.2) and the Tit-for-Tat PULL unchoker (Ch5 §5.1).

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::net::IpAddr;

use bs_crypto::Signer;
use bs_wire::consts::{BLOCK_SIZE, K_BLOCK};
use bs_wire::frames::{ChokeState, ProofOfUpload, RxFlags};
use bs_wire::{Frame, NodeId, PublicKeyBytes, SegmentSeq, TreeId};
use bytes::Bytes;

use super::{Lifecycle, Node};
use crate::io::{Channel, Event, PeerAddr};
use crate::swarm::VerifiedBlock;
use crate::time::Instant;

/// A receipt we hold as uploader.
#[derive(Debug, Clone)]
pub struct StoredReceipt {
    /// The receipt.
    pub pou: ProofOfUpload,
    /// `min(Bytes, Cap)` for the rate (Ch5 §5.2.2).
    pub capped_bytes: u64,
    /// `w_j`: 3 for RELAYED, else 1.
    pub weight: u32,
    /// When accepted (our clock), for the decayed score.
    pub at: Instant,
}

#[derive(Debug, Clone)]
struct Delivered {
    bitmap: [u8; ProofOfUpload::MAX_BITMAP],
    blocks: u16,
    chunks: u8,
}

impl Default for Delivered {
    fn default() -> Self {
        Self {
            bitmap: [0u8; ProofOfUpload::MAX_BITMAP],
            blocks: 0,
            chunks: 0,
        }
    }
}

#[derive(Debug, Clone)]
struct ChildReceipts {
    /// Highest segment receipted.
    last_receipted: u32,
    /// Deadlines checked up to this segment.
    checked_up_to: u32,
    /// Consecutive unreceipted segments.
    missed: u8,
}

#[derive(Debug, Clone)]
struct ChokeEntry {
    state: ChokeState,
    optimistic_until: Option<Instant>,
}

/// Incentive state on a node.
#[derive(Debug, Default)]
pub struct IncentiveState {
    /// Public keys (and static nonces) learned from validation blocks, by NodeID.
    pub known_keys: BTreeMap<NodeId, (PublicKeyBytes, u64)>,
    // ---- downloader side
    delivered: BTreeMap<(u32, u8, PeerAddr), Delivered>,
    /// Keys already receipted: a second receipt for a key would be an equivocation.
    issued: BTreeSet<(u32, u8, PeerAddr)>,
    link_loss: BTreeMap<PeerAddr, VecDeque<(Instant, u16, u16)>>,
    pull_ingress: BTreeMap<PeerAddr, VecDeque<(Instant, u32)>>,
    /// Providers that have unchoked us (we only ask these).
    pub unchoked_by: BTreeSet<PeerAddr>,
    // ---- uploader side
    /// Receipts naming us as uploader, within the rank window.
    pub receipts: Vec<StoredReceipt>,
    child_receipts: BTreeMap<(u8, PeerAddr), ChildReceipts>,
    /// Peers that missed a receipt deadline (ingress counts as 0 this cycle).
    deadline_missed: BTreeSet<PeerAddr>,
    /// Evicted identities and prefixes, banned from our trees until the instant.
    evicted_ids: BTreeMap<NodeId, Instant>,
    evicted_prefixes: BTreeMap<[u8; 8], Instant>,
    // ---- Tit-for-Tat
    choke: BTreeMap<PeerAddr, ChokeEntry>,
    first_seen: BTreeMap<PeerAddr, Instant>,
    /// Peers that asked us for PULL service: (first request, last request).
    wanted: BTreeMap<PeerAddr, (Instant, Instant)>,
    cooldown_id: BTreeMap<NodeId, Instant>,
    cooldown_prefix: BTreeMap<[u8; 8], Instant>,
    next_tft: Option<Instant>,
    /// Counters.
    pub receipts_issued: u64,
    /// Counters.
    pub receipts_accepted: u64,
    /// Counters.
    pub receipts_rejected: u64,
    /// Counters.
    pub deadline_evictions: u64,
    /// Counters.
    pub pulls_choked: u64,
}

pub(super) fn prefix_key(ip: IpAddr) -> [u8; 8] {
    let mut k = [0u8; 8];
    match ip {
        IpAddr::V4(v4) => k[..3].copy_from_slice(&v4.octets()[..3]),
        IpAddr::V6(v6) => k[..6].copy_from_slice(&v6.octets()[..6]),
    }
    k
}

/// `E = max(1, ⌈clamp(2ρK, 0.05K, 0.30K)⌉)` (Ch4 §4.2.2).
pub fn parity_for_loss(loss_byte: u8) -> u16 {
    let rho = loss_byte as f64 / 255.0;
    let k = K_BLOCK as f64;
    let e = (2.0 * rho * k).clamp(0.05 * k, 0.30 * k).ceil();
    (e as u16).max(1)
}

impl Node {
    // ------------------------------------------------------------------ accessors

    /// Children evicted for missed receipt deadlines.
    pub fn deadline_evictions(&self) -> u64 {
        self.incentive.deadline_evictions
    }

    /// Receipts held as uploader.
    pub fn receipt_count(&self) -> usize {
        self.incentive.receipts.len()
    }

    /// Mean parity we currently send across all children (Ē_v, Ch1 §1.2.1).
    pub fn mean_child_parity(&self) -> f64 {
        let mut n = 0usize;
        let mut sum = 0u64;
        for t in &self.trees {
            for c in t.children.values() {
                n += 1;
                sum += c.parity as u64;
            }
        }
        if n == 0 {
            self.params.default_parity as f64
        } else {
            sum as f64 / n as f64
        }
    }

    /// Undecayed 60-segment delivered-throughput rate Θ^rate, bit/s (Ch5 §5.2.2).
    pub fn theta_rate(&self) -> f64 {
        let edge = self.swarm.live_edge.0;
        let lo = edge.saturating_sub(self.params.rank_window_segments);
        let bytes: u64 = self
            .incentive
            .receipts
            .iter()
            .filter(|r| r.pou.segment.0 >= lo && r.pou.segment.0 <= edge)
            .map(|r| r.weight as u64 * r.capped_bytes)
            .sum();
        bytes as f64 * 8.0 / 60.0
    }

    /// Decayed Swarm Contribution Score Θ_A = Σ w · bytes · e^{−λ·age} (Ch5 §5.2.2).
    pub fn theta_decayed(&self) -> f64 {
        let lambda = self.params.receipt_decay_lambda;
        self.incentive
            .receipts
            .iter()
            .map(|r| {
                let age = self.now.duration_since(r.at).as_secs_f64();
                r.weight as f64 * r.capped_bytes as f64 * (-lambda * age).exp()
            })
            .sum()
    }

    /// Rank: full-stream-equivalents currently delivered (Θ^rate / B).
    pub fn rank(&self) -> f64 {
        let b_bits = self.stream_bitrate_bits();
        if b_bits <= 0.0 {
            0.0
        } else {
            self.theta_rate() / b_bits
        }
    }

    /// Declared stream bitrate `B` in bit/s (sum of the matrix's tree bitrates).
    pub(super) fn stream_bitrate_bits(&self) -> f64 {
        self.matrix
            .as_ref()
            .map(|m| {
                m.to_entries()
                    .iter()
                    .map(|t| t.bitrate_kbps as f64)
                    .sum::<f64>()
                    * 1000.0
            })
            .unwrap_or(0.0)
    }

    /// Bytes one receipt can honestly attest (Ch5 §5.2.2).
    ///
    /// A PULL receipt is capped at `β_pull` over one second. A tree receipt is capped
    /// at 1.5 times the named tree's declared bitrate over one segment.
    pub(super) fn receipt_cap(&self, pou: &ProofOfUpload) -> u64 {
        if pou.rx_flags.pull {
            self.params.beta_pull_kbps as u64 * 1000 / 8
        } else {
            let kbps = self
                .matrix
                .as_ref()
                .and_then(|m| m.tree(pou.tree_id))
                .map(|t| t.bitrate_kbps as u64)
                .unwrap_or(0);
            (kbps * 1000 / 8) * 3 / 2
        }
    }

    /// Refuse `id` at `from` for the eviction cooldown (forged proof, Ch7 §7.1.1).
    pub(super) fn ban_peer(&mut self, from: PeerAddr, id: NodeId) {
        let until = self.now + self.params.eviction_cooldown;
        self.incentive.evicted_ids.insert(id, until);
        self.incentive
            .evicted_prefixes
            .insert(prefix_key(from.ip()), until);
    }

    /// Whether we currently serve PULL requests from `peer`.
    pub fn serves_pull_to(&self, peer: PeerAddr) -> bool {
        matches!(
            self.incentive.choke.get(&peer).map(|c| c.state),
            Some(ChokeState::Unchoke) | Some(ChokeState::UnchokeOptimistic)
        )
    }

    /// Record a peer's key from a verified validation block.
    pub(super) fn learn_key(&mut self, vb: &bs_wire::ValidationBlock) {
        self.incentive
            .known_keys
            .insert(vb.node_id, (vb.public_key, vb.static_nonce));
    }

    pub(super) fn k_s_hash(&self) -> bs_wire::Hash {
        bs_wire::Hash(crate::dht::stream_key(&self.stream_id()))
    }

    // ------------------------------------------------------------------ downloader: issuing receipts

    /// A block verified: remember who delivered it, and its link loss, for the receipt.
    pub(super) fn note_delivered(&mut self, vb: &VerifiedBlock, from: PeerAddr, via_pull: bool) {
        let tree = if via_pull { 0 } else { vb.tree.0 };
        let Some(bit) = ProofOfUpload::bit(vb.index.chunk(), vb.index.j()) else {
            return;
        };
        // RULE: One receipt per (segment, tree, provider). A block that arrives after
        //       that receipt went out is not receipted again (Ch5 §5.2.1).
        if self.incentive.issued.contains(&(vb.segment.0, tree, from)) {
            return;
        }
        let d = self
            .incentive
            .delivered
            .entry((vb.segment.0, tree, from))
            .or_default();
        if d.bitmap[bit / 8] & (0x80 >> (bit % 8)) == 0 {
            d.bitmap[bit / 8] |= 0x80 >> (bit % 8);
            d.blocks += 1;
        }
        d.chunks = d.chunks.max(vb.index.chunk() + 1);
        if !via_pull {
            // The sender pushes K + E symbols; we count what reached us (E is the
            // default until the parent tells us otherwise: a floor on the estimate).
            let sent = (K_BLOCK as u16).saturating_add(self.params.default_parity);
            let lost = sent.saturating_sub(vb.symbols_received);
            let q = self.incentive.link_loss.entry(from).or_default();
            q.push_back((self.now, lost, sent));
        } else {
            self.note_pull_ingress(from, BLOCK_SIZE as u32);
        }
        // Tree receipts for earlier segments are due now. PULL receipts wait for the
        // pull zone to close on their segment, so repairs are credited in full.
        self.flush_receipts_before(vb.segment.0, false);
    }

    /// Bytes a provider served us on the PULL path (reciprocity input).
    pub(super) fn note_pull_ingress(&mut self, from: PeerAddr, bytes: u32) {
        let q = self.incentive.pull_ingress.entry(from).or_default();
        q.push_back((self.now, bytes));
    }

    fn link_loss_byte(&mut self, from: PeerAddr) -> u8 {
        let window = self.params.tft_window;
        let now = self.now;
        let Some(q) = self.incentive.link_loss.get_mut(&from) else {
            return 0;
        };
        while q
            .front()
            .map(|(t, _, _)| now.duration_since(*t) > window)
            .unwrap_or(false)
        {
            q.pop_front();
        }
        let (lost, sent) = q.iter().fold((0u32, 0u32), |(l, s), (_, a, b)| {
            (l + *a as u32, s + *b as u32)
        });
        if sent == 0 {
            0
        } else {
            ((lost as f64 / sent as f64) * 255.0).floor().min(255.0) as u8
        }
    }

    /// Issue receipts for every entry with segment < `before`; PULL entries only when
    /// `pull` is set.
    fn flush_receipts_before(&mut self, before: u32, pull: bool) {
        let due: Vec<((u32, u8, PeerAddr), Delivered)> = self
            .incentive
            .delivered
            .iter()
            .filter(|((s, t, _), _)| *s < before && (pull || *t != 0))
            .map(|(k, v)| (*k, v.clone()))
            .collect();
        for (key, d) in due {
            self.incentive.delivered.remove(&key);
            self.incentive.issued.insert(key);
            self.issue_receipt(key.0, key.1, key.2, &d);
        }
        let floor = self.swarm.live_edge.0.saturating_sub(80);
        self.incentive.issued.retain(|(s, _, _)| *s >= floor);
    }

    /// Flush every pending receipt (stream end, quit).
    pub(super) fn flush_all_receipts(&mut self) {
        self.flush_receipts_before(u32::MAX, true);
    }

    fn issue_receipt(&mut self, segment: u32, tree: u8, provider: PeerAddr, d: &Delivered) {
        if d.blocks == 0 {
            return;
        }
        let uploader = match self.sessions.get(&provider).and_then(|s| s.node_id) {
            Some(id) => id,
            None => match self.trees.iter().find_map(|t| {
                t.parent
                    .as_ref()
                    .filter(|p| p.addr == provider)
                    .map(|p| p.node_id)
            }) {
                Some(id) => id,
                None => return,
            },
        };
        let pull = tree == 0;
        let loss = if pull {
            0
        } else {
            self.link_loss_byte(provider)
        };
        let len = (d.chunks as usize * ProofOfUpload::BLOCKS_PER_CHUNK / 8)
            .min(ProofOfUpload::MAX_BITMAP);
        let bitmap = Bytes::copy_from_slice(&d.bitmap[..len]);
        let rec = self.peer_record();
        let k_avail = if pull {
            0
        } else {
            self.trees
                .get(TreeId(tree).index())
                .map(|t| t.k_avail(self.is_source()))
                .unwrap_or(0)
        };
        let mut pou = ProofOfUpload {
            segment: SegmentSeq(segment),
            tree_id: TreeId(tree),
            rx_flags: RxFlags {
                // Served through an emergent relay: weighted 3× by verifiers (Ch6 §6.3.2).
                relayed: !pull && self.bridged_parent(TreeId(tree)) == Some(provider),
                pull,
            },
            loss_rate: loss,
            bitmap,
            k_avail,
            assigned_trees: rec.assigned_trees,
            node_class: rec.node_class,
            stream_key: self.k_s_hash(),
            uploader,
            downloader: self.node_id(),
            timestamp_us: self.now.as_micros(),
            signature: bs_wire::SignatureBytes::ZERO,
        };
        pou.signature = self.identity.sign(&pou.signable_bytes());
        let channel = if pull {
            Channel::Control
        } else {
            Channel::Tree(TreeId(tree))
        };
        self.incentive.receipts_issued += 1;
        self.emit(Event::ReceiptIssued {
            tree,
            segment,
            blocks: d.blocks,
            loss,
        });
        self.send(provider, channel, Frame::ProofOfUpload(pou), None);
    }

    // ------------------------------------------------------------------ uploader: verifying receipts

    pub(super) fn on_receipt(&mut self, from: PeerAddr, pou: ProofOfUpload) {
        let reject = |this: &mut Node, reason: &'static str| {
            this.incentive.receipts_rejected += 1;
            this.emit(Event::ReceiptRejected { reason });
        };
        if pou.uploader != self.node_id() {
            return reject(self, "wrong_uploader");
        }
        if pou.stream_key != self.k_s_hash() {
            return reject(self, "wrong_stream");
        }
        let edge = self.swarm.live_edge.0;
        if pou.segment.0 + self.params.rank_window_segments < edge {
            return reject(self, "stale");
        }
        if let Some(Some(id)) = self.sessions.get(&from).map(|s| s.node_id) {
            if id != pou.downloader {
                return reject(self, "wrong_downloader");
            }
        }
        let Some((pk, _)) = self.incentive.known_keys.get(&pou.downloader).copied() else {
            return reject(self, "unknown_key");
        };
        if bs_crypto::Verifier::verify(&pk, &pou.signable_bytes(), &pou.signature).is_err() {
            return reject(self, "bad_signature");
        }
        let capped = pou.byte_value().min(self.receipt_cap(&pou).max(1));
        let weight = if pou.rx_flags.relayed { 3 } else { 1 };
        let key = pou.canonical_key();
        // One receipt per canonical key. A replacement with another bitmap is an
        // equivocation by the signer (Ch5 §5.3.3).
        if let Some(old) = self
            .incentive
            .receipts
            .iter()
            .find(|r| r.pou.canonical_key() == key)
            .map(|r| r.pou.clone())
        {
            self.audit_check_equivocation(&old, &pou);
        }
        self.incentive
            .receipts
            .retain(|r| r.pou.canonical_key() != key);
        let (tree, seg, loss) = (pou.tree_id, pou.segment.0, pou.loss_rate);
        self.incentive.receipts.push(StoredReceipt {
            pou,
            capped_bytes: capped,
            weight,
            at: self.now,
        });
        self.incentive.receipts_accepted += 1;
        self.emit(Event::ReceiptAccepted {
            tree: tree.0,
            segment: seg,
            bytes: capped,
        });
        // Deadline bookkeeping for tree receipts.
        if tree.is_tree() {
            let e = self
                .incentive
                .child_receipts
                .entry((tree.0, from))
                .or_insert(ChildReceipts {
                    last_receipted: seg,
                    checked_up_to: seg,
                    missed: 0,
                });
            if seg >= e.last_receipted {
                e.last_receipted = seg;
                e.missed = 0;
                self.incentive.deadline_missed.remove(&from);
            }
            self.apply_loss_report(from, tree, loss);
        }
        // Trim the ledger to the rank window.
        let lo = edge.saturating_sub(self.params.rank_window_segments);
        self.incentive.receipts.retain(|r| r.pou.segment.0 >= lo);
    }

    /// Adaptive parity: recompute this child's parity from its loss report, and the
    /// slot count from the mean parity (Ch4 §4.2.2 rule 5, Ch1 §1.2.1).
    fn apply_loss_report(&mut self, child: PeerAddr, tree: TreeId, loss: u8) {
        let e = parity_for_loss(loss);
        let mut changed = false;
        let mut child_id = None;
        if let Some(c) = self
            .trees
            .get_mut(tree.index())
            .and_then(|t| t.children.get_mut(&child))
        {
            child_id = Some(c.node_id);
            if c.parity != e {
                c.parity = e;
                changed = true;
            }
        }
        if changed {
            if let Some(id) = child_id {
                self.emit(Event::ParityChanged {
                    child: id,
                    parity: e,
                    loss,
                });
            }
            self.recompute_slots();
        }
    }

    /// A fallen `K_v(m)` releases the newest non-draining child through the drain path
    /// (reason CAPACITY), never by dropping it (Ch1 §1.2.1 §1.3).
    pub(super) fn shed_capacity(&mut self, tree: TreeId) {
        let idx = tree.index();
        let over = self.trees[idx]
            .children
            .values()
            .filter(|c| c.draining_until.is_none())
            .count()
            .saturating_sub(self.trees[idx].k_v as usize);
        if over == 0 {
            return;
        }
        let deadline = SegmentSeq(self.swarm.live_edge.0 + self.params.tau_drain_segments);
        // A bridge child holds a slot of its own budget: never the victim here.
        let mut victims: Vec<(Instant, PeerAddr)> = self.trees[idx]
            .children
            .values()
            .filter(|c| c.draining_until.is_none())
            .filter(|c| !self.is_bridge_child(c.addr, tree))
            .map(|c| (c.admitted_at, c.addr))
            .collect();
        victims.sort();
        for (_, v) in victims.into_iter().rev().take(over) {
            let d = bs_wire::frames::DrainNotice {
                tree_id: tree,
                reason: bs_wire::DisconnectReason::Capacity,
                scope: bs_wire::frames::DrainScope::ThisChild,
                deadline,
            };
            self.send(v, Channel::Tree(tree), Frame::DrainNotice(d), None);
            if let Some(c) = self.trees[idx].children.get_mut(&v) {
                c.draining_until = Some(deadline);
            }
        }
    }

    /// Before pushing segment `seg` to `child` in `tree`: the receipt for `seg − 2` is
    /// due (Ch5 §5.2.1). A miss chokes at the next TFT cycle; three consecutive misses
    /// evict (`DISCONNECT` EVICTION). Returns `false` when the child was evicted.
    pub(super) fn check_receipt_deadline(
        &mut self,
        tree: TreeId,
        child: PeerAddr,
        seg: u32,
    ) -> bool {
        let e = self
            .incentive
            .child_receipts
            .entry((tree.0, child))
            .or_insert(ChildReceipts {
                // The admission segment is usually partial: the first receipt due is
                // for the first full segment after it.
                last_receipted: seg,
                checked_up_to: seg,
                missed: 0,
            });
        if seg <= e.checked_up_to {
            return true;
        }
        let mut newly_missed = 0u8;
        for k in (e.checked_up_to + 1)..=seg {
            let due = k.saturating_sub(2);
            if due == 0 {
                continue;
            }
            if e.last_receipted >= due {
                e.missed = 0;
            } else {
                e.missed = e.missed.saturating_add(1);
                newly_missed = e.missed;
            }
        }
        e.checked_up_to = seg;
        let missed = e.missed;
        if newly_missed > 0 {
            self.incentive.deadline_missed.insert(child);
            let id = self
                .trees
                .get(tree.index())
                .and_then(|t| t.children.get(&child))
                .map(|c| c.node_id)
                .unwrap_or(NodeId::ZERO);
            self.emit(Event::ReceiptMissed {
                tree: tree.0,
                child: id,
                consecutive: missed,
            });
            if missed >= self.params.receipt_evict_after {
                let f = bs_wire::frames::Disconnect {
                    sender: self.node_id(),
                    reason: bs_wire::DisconnectReason::Eviction,
                    tree_id: tree,
                };
                self.send(child, Channel::Control, Frame::Disconnect(f), None);
                self.incentive.deadline_evictions += 1;
                self.incentive.child_receipts.remove(&(tree.0, child));
                // Recorded against the identity and the prefix (Ch5 §5.2.1): a fresh
                // NodeID is ~10 ms of CPU, an address prefix is not.
                let until = self.now + self.params.eviction_cooldown;
                if id != NodeId::ZERO {
                    self.incentive.evicted_ids.insert(id, until);
                }
                self.incentive
                    .evicted_prefixes
                    .insert(prefix_key(child.ip()), until);
                self.remove_child_from_tree(tree, child, "receipt deadline");
                return false;
            }
        }
        true
    }

    /// Whether a join from `from`/`id` is refused because of a recent eviction.
    pub(super) fn admission_banned(&mut self, from: PeerAddr, id: NodeId) -> bool {
        let now = self.now;
        self.incentive.evicted_ids.retain(|_, t| *t > now);
        self.incentive.evicted_prefixes.retain(|_, t| *t > now);
        self.incentive.evicted_ids.contains_key(&id)
            || self
                .incentive
                .evicted_prefixes
                .contains_key(&prefix_key(from.ip()))
    }

    /// Forget receipt bookkeeping for a departed child.
    pub(super) fn forget_child_receipts(&mut self, child: PeerAddr) {
        self.incentive
            .child_receipts
            .retain(|(_, a), _| *a != child);
    }

    // ------------------------------------------------------------------ Tit-for-Tat unchoker

    pub(super) fn incentive_start(&mut self) {
        self.incentive.next_tft = Some(self.now + self.params.tau_tft);
    }

    pub(super) fn incentive_next_timer(&self) -> Option<Instant> {
        self.incentive.next_tft
    }

    pub(super) fn incentive_timers(&mut self) {
        if self.state == Lifecycle::Terminated {
            return;
        }
        if let Some(t) = self.incentive.next_tft {
            if self.now >= t {
                self.incentive.next_tft = Some(self.now + self.params.tau_tft);
                self.tft_cycle();
                self.audit_tick();
                // Receipts for segments two behind the live edge are overdue: issue them
                // even if no newer block arrived from that provider.
                let edge = self.swarm.live_edge.0;
                if edge >= 2 {
                    self.flush_receipts_before(edge - 1, false);
                }
                if edge >= 5 {
                    self.flush_receipts_before(edge - 4, true);
                }
            }
        }
    }

    fn ingress_rate_kbps(&mut self, peer: PeerAddr) -> f64 {
        let window = self.params.tft_window;
        let now = self.now;
        let Some(q) = self.incentive.pull_ingress.get_mut(&peer) else {
            return 0.0;
        };
        while q
            .front()
            .map(|(t, _)| now.duration_since(*t) > window)
            .unwrap_or(false)
        {
            q.pop_front();
        }
        let bytes: u64 = q.iter().map(|(_, b)| *b as u64).sum();
        bytes as f64 * 8.0 / window.as_secs_f64() / 1000.0
    }

    /// PullSlots = ⌊(r_pull·u − Σ_handovers B_m Ω) / β_pull⌋ (Ch5 §5.1.2).
    fn pull_slots(&self) -> usize {
        let omega = (1.0 + self.mean_child_parity() / K_BLOCK as f64) * (1.0 + self.params.f_frame);
        let mut handover_kbps = 0.0;
        for t in &self.trees {
            let over = t.children.len().saturating_sub(t.k_v as usize);
            if over > 0 && t.children.values().any(|c| c.draining_until.is_some()) {
                let b = self
                    .matrix
                    .as_ref()
                    .and_then(|m| m.tree(t.id))
                    .map(|x| x.bitrate_kbps as f64)
                    .unwrap_or(0.0);
                handover_kbps += over as f64 * b * omega;
            }
        }
        let reserve = self.params.r_pull * self.cfg.upload_kbps as f64 - handover_kbps;
        (reserve / self.params.beta_pull_kbps as f64)
            .floor()
            .max(0.0) as usize
    }

    fn set_choke(&mut self, peer: PeerAddr, state: ChokeState, optimistic_until: Option<Instant>) {
        let prev = self.incentive.choke.get(&peer).map(|c| c.state);
        self.incentive.choke.insert(
            peer,
            ChokeEntry {
                state,
                optimistic_until,
            },
        );
        if prev != Some(state) {
            let id = self
                .sessions
                .get(&peer)
                .and_then(|s| s.node_id)
                .unwrap_or(NodeId::ZERO);
            self.emit(Event::ChokeChanged {
                peer: id,
                state: match state {
                    ChokeState::Choke => "choke",
                    ChokeState::Unchoke => "unchoke",
                    ChokeState::UnchokeOptimistic => "optimistic",
                },
            });
            self.send(peer, Channel::Control, Frame::ChokeState(state), None);
        }
    }

    /// One sliding-window unchoker cycle (Ch5 §5.1.2–§5.1.3).
    fn tft_cycle(&mut self) {
        let now = self.now;
        let peers: Vec<PeerAddr> = self
            .active_peers()
            .into_iter()
            .filter(|p| self.sessions.get(p).map(|s| s.open).unwrap_or(false))
            .collect();
        // Forget departed peers.
        let alive: BTreeSet<PeerAddr> = peers.iter().copied().collect();
        self.incentive.choke.retain(|p, _| alive.contains(p));
        self.incentive.first_seen.retain(|p, _| alive.contains(p));
        for p in &peers {
            self.incentive.first_seen.entry(*p).or_insert(now);
        }
        let pull_slots = self.pull_slots();
        let threshold = self.params.tft_threshold_min_kbps as f64;
        // Close optimistic windows that ended: promote or re-choke with cooldown.
        let ended: Vec<PeerAddr> = self
            .incentive
            .choke
            .iter()
            .filter(|(_, c)| {
                c.state == ChokeState::UnchokeOptimistic
                    && c.optimistic_until.map(|t| now >= t).unwrap_or(false)
            })
            .map(|(p, _)| *p)
            .collect();
        for p in ended {
            let rate = self.ingress_rate_kbps(p);
            if rate >= threshold {
                self.set_choke(p, ChokeState::Unchoke, None);
            } else {
                let until = now + self.params.optimistic_cooldown;
                if let Some(Some(id)) = self.sessions.get(&p).map(|s| s.node_id) {
                    self.incentive.cooldown_id.insert(id, until);
                }
                self.incentive
                    .cooldown_prefix
                    .insert(prefix_key(p.ip()), until);
                self.set_choke(p, ChokeState::Choke, None);
            }
        }
        self.incentive.cooldown_id.retain(|_, t| *t > now);
        self.incentive.cooldown_prefix.retain(|_, t| *t > now);
        // Rank neighbours by what they gave us (a missed receipt deadline counts as 0).
        let mut ranked: Vec<(f64, Instant, PeerAddr)> = peers
            .iter()
            .map(|p| {
                let r = if self.incentive.deadline_missed.contains(p) {
                    0.0
                } else {
                    self.ingress_rate_kbps(*p)
                };
                (r, self.incentive.first_seen[p], *p)
            })
            .collect();
        ranked.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap().then(a.1.cmp(&b.1)));
        // Bootstrap queue: zero contributors that are actually waiting (they asked us
        // for PULL service within the window) and not in cooldown, FIFO by first ask.
        let stale = self.params.tft_window * 5;
        self.incentive
            .wanted
            .retain(|_, (_, last)| now.duration_since(*last) <= stale);
        let wanted = self.incentive.wanted.clone();
        let mut queue: Vec<(Instant, PeerAddr)> = ranked
            .iter()
            .filter(|(r, _, p)| {
                *r <= 0.0
                    && wanted.contains_key(p)
                    && !self
                        .sessions
                        .get(p)
                        .and_then(|s| s.node_id)
                        .map(|id| self.incentive.cooldown_id.contains_key(&id))
                        .unwrap_or(false)
                    && !self
                        .incentive
                        .cooldown_prefix
                        .contains_key(&prefix_key(p.ip()))
            })
            .map(|(_, _, p)| (wanted[p].0, *p))
            .collect();
        queue.sort();
        let j_new = queue.len();
        let optimistic_slots = if pull_slots <= 1 {
            0
        } else {
            (j_new.div_ceil(3)).min(4).min(pull_slots / 2)
        };
        let regular = pull_slots.saturating_sub(optimistic_slots.min(pull_slots / 2));
        let currently_optimistic: Vec<PeerAddr> = self
            .incentive
            .choke
            .iter()
            .filter(|(_, c)| c.state == ChokeState::UnchokeOptimistic)
            .map(|(p, _)| *p)
            .collect();
        // Regular slots to the top reciprocators; a slot no reciprocator claims is
        // spent on a waiting peer instead of held idle (the budget stays PullSlots).
        let mut unchoked: BTreeSet<PeerAddr> = BTreeSet::new();
        for (r, _, p) in ranked.iter().take(regular) {
            if *r > 0.0 {
                unchoked.insert(*p);
            }
        }
        let optimistic_slots = (pull_slots - unchoked.len()).min(pull_slots);
        // Optimistic slots: keep windows still open, fill free ones from the queue head.
        let mut optimistic: Vec<PeerAddr> = currently_optimistic
            .into_iter()
            .filter(|p| !unchoked.contains(p))
            .collect();
        for (_, p) in queue {
            if optimistic.len() >= optimistic_slots {
                break;
            }
            if !unchoked.contains(&p) && !optimistic.contains(&p) {
                optimistic.push(p);
            }
        }
        optimistic.truncate(optimistic_slots);
        let opt_set: BTreeSet<PeerAddr> = optimistic.iter().copied().collect();
        for p in peers {
            if unchoked.contains(&p) {
                self.set_choke(p, ChokeState::Unchoke, None);
            } else if opt_set.contains(&p) {
                let until = self
                    .incentive
                    .choke
                    .get(&p)
                    .and_then(|c| {
                        (c.state == ChokeState::UnchokeOptimistic).then_some(c.optimistic_until)
                    })
                    .flatten()
                    .unwrap_or(now + self.params.optimistic_window);
                self.set_choke(p, ChokeState::UnchokeOptimistic, Some(until));
            } else {
                self.set_choke(p, ChokeState::Choke, None);
            }
        }
        self.incentive.deadline_missed.clear();
    }

    /// A provider told us its choke state toward us.
    pub(super) fn on_choke_state(&mut self, from: PeerAddr, s: ChokeState) {
        match s {
            ChokeState::Choke => {
                self.incentive.unchoked_by.remove(&from);
            }
            ChokeState::Unchoke | ChokeState::UnchokeOptimistic => {
                self.incentive.unchoked_by.insert(from);
            }
        }
    }

    /// A peer asked us for PULL service: it is waiting (bootstrap queue input).
    pub(super) fn note_pull_want(&mut self, from: PeerAddr) {
        let now = self.now;
        self.incentive
            .wanted
            .entry(from)
            .and_modify(|(_, last)| *last = now)
            .or_insert((now, now));
    }

    /// Whether a PULL_REQUEST from `from` is served now. A waiting peer takes a free
    /// optimistic slot at once rather than at the next 2 s boundary: the slot budget
    /// is unchanged, only the wait is removed (Ch5 §5.1.3 fills slots from the queue
    /// head; nothing there requires holding a free slot idle).
    pub(super) fn pull_allowed(
        &mut self,
        from: PeerAddr,
        r: &bs_wire::frames::PullRequest,
    ) -> bool {
        if self.serves_pull_to(from) {
            return true;
        }
        // A child repairing a block of the stripe *we* push to it is not free-riding:
        // its receipts for that tree already pay for those bytes, and the loss was on
        // our link. Served within the PULL budget regardless of choke state (ISSUE-066).
        let tree = self
            .swarm
            .chunk(r.segment, r.block.chunk())
            .and_then(|c| c.tree_of.get(r.block.j() as usize).copied());
        if let Some(t) = tree {
            if self
                .trees
                .get(t.index())
                .map(|ts| ts.children.contains_key(&from))
                .unwrap_or(false)
            {
                return true;
            }
        }
        if self.try_optimistic_now(from) {
            return true;
        }
        self.incentive.pulls_choked += 1;
        self.emit(Event::PullChoked);
        false
    }

    fn try_optimistic_now(&mut self, from: PeerAddr) -> bool {
        let now = self.now;
        if !self.sessions.get(&from).map(|s| s.open).unwrap_or(false) {
            return false;
        }
        let pull_slots = self.pull_slots();
        if pull_slots <= 1 {
            return false;
        }
        let in_cooldown = self
            .sessions
            .get(&from)
            .and_then(|s| s.node_id)
            .map(|id| {
                self.incentive
                    .cooldown_id
                    .get(&id)
                    .map(|t| *t > now)
                    .unwrap_or(false)
            })
            .unwrap_or(false)
            || self
                .incentive
                .cooldown_prefix
                .get(&prefix_key(from.ip()))
                .map(|t| *t > now)
                .unwrap_or(false);
        if in_cooldown {
            return false;
        }
        let used = self
            .incentive
            .choke
            .values()
            .filter(|c| c.state != ChokeState::Choke)
            .count();
        let optimistic = self
            .incentive
            .choke
            .values()
            .filter(|c| c.state == ChokeState::UnchokeOptimistic)
            .count();
        if used >= pull_slots || optimistic >= pull_slots / 2 {
            return false;
        }
        self.incentive.first_seen.entry(from).or_insert(now);
        self.set_choke(
            from,
            ChokeState::UnchokeOptimistic,
            Some(now + self.params.optimistic_window),
        );
        true
    }
}
