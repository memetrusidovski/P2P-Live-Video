//! Reputation auditing (Ch5 §5.3): symmetric-pair and equivocation detection on
//! receipts, signed evidence-carrying accusations over gossip, consensus eviction
//! with bounded local bans, and the subnet penalty on rank proofs.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::net::IpAddr;

use bs_crypto::Signer;
use bs_wire::frames::{Accusation, AuditGossip, EvidenceType, ProofOfUpload};
use bs_wire::{Frame, NodeId, PublicKeyBytes, TreeId};

use super::incentive::prefix_key;
use super::Node;
use crate::io::{Channel, Event, PeerAddr};
use crate::time::{Duration, Instant};

#[derive(Debug, Clone)]
struct Sampled {
    pou: ProofOfUpload,
    at: Instant,
}

/// Auditing state on a node.
#[derive(Debug, Default)]
pub struct AuditState {
    /// Sampled receipts from verified proofs, by (uploader, downloader, tree, segment).
    seen: BTreeMap<(NodeId, NodeId, u8, u32), Sampled>,
    /// Distinct counterparties per uploader, from sampled receipts (the degree).
    degree: BTreeMap<NodeId, BTreeSet<NodeId>>,
    /// Verified accusations per suspect: accuser → (accuser prefix, when counted).
    verified: BTreeMap<NodeId, BTreeMap<NodeId, ([u8; 8], Instant)>>,
    /// Suspects we accused ourselves, to accuse each once per window.
    accused: BTreeMap<NodeId, Instant>,
    /// Accusations received per accuser in the last minute (rate limit).
    rate: BTreeMap<NodeId, VecDeque<Instant>>,
    /// Bans: suspect → (until, strikes).
    banned: BTreeMap<NodeId, (Instant, u32)>,
    /// Accusations whose accuser we are resolving through the DHT: (gossip, sender, since).
    parked: Vec<(AuditGossip, PeerAddr, Instant)>,
    /// Counters.
    pub accusations_sent: u64,
    /// Counters.
    pub accusations_counted: u64,
    /// Counters.
    pub accusations_dropped: u64,
    /// Counters.
    pub evictions: u64,
}

/// Whether two receipt bitmaps claim at least one common block.
fn bitmaps_intersect(a: &[u8], b: &[u8]) -> bool {
    a.iter().zip(b.iter()).any(|(x, y)| x & y != 0)
}

fn within(a: u64, b: u64, tolerance: f64) -> bool {
    let (lo, hi) = (a.min(b) as f64, a.max(b) as f64);
    hi == 0.0 || (hi - lo) / hi <= tolerance
}

impl Node {
    // ------------------------------------------------------------------ accessors

    /// Whether `id` is under a consensus ban now.
    pub fn audit_banned(&self, id: &NodeId) -> bool {
        self.audit
            .banned
            .get(id)
            .map(|(until, _)| *until > self.now)
            .unwrap_or(false)
    }

    /// Consensus evictions this node performed.
    pub fn audit_evictions(&self) -> u64 {
        self.audit.evictions
    }

    /// `N_guard`: the larger of the signed record's swarm size and the peers we saw.
    fn n_guard(&self) -> u32 {
        let mut seen: BTreeSet<NodeId> = self.passive.keys().copied().collect();
        seen.extend(self.membership.active.values().map(|a| a.node_id));
        seen.extend(self.sessions.values().filter_map(|s| s.node_id));
        seen.extend(self.dht.table.contacts().map(|c| c.record.node_id));
        self.swarm_size.max(seen.len() as u32)
    }

    /// An address observed by us or by a guardian for `id`, if any.
    pub(super) fn resolve_addr(&self, id: &NodeId) -> Option<IpAddr> {
        if let Some((addr, _)) = self.sessions.iter().find(|(_, s)| s.node_id == Some(*id)) {
            return Some(addr.ip());
        }
        if let Some(a) = self.membership.active.values().find(|a| a.node_id == *id) {
            return Some(a.record.addr.0.ip());
        }
        if let Some(r) = self.passive.get(id) {
            return Some(r.addr.0.ip());
        }
        self.dht.table.get(id).map(|c| c.record.addr.0.ip())
    }

    // ------------------------------------------------------------------ detection

    /// A sampled receipt from a verified RANK_PROOF: remember it and test the pair.
    pub(super) fn audit_note_sampled(&mut self, pou: &ProofOfUpload) {
        if pou.rx_flags.pull || pou.tree_id.0 == 0 {
            return;
        }
        let key = (pou.uploader, pou.downloader, pou.tree_id.0, pou.segment.0);
        self.audit
            .degree
            .entry(pou.uploader)
            .or_default()
            .insert(pou.downloader);
        // WHY: The window applies to the receipts' own timestamps (checked in
        //      `symmetric_pair`), not to when the two proofs reached us. Samples are kept
        //      for τ_accuse so a pair whose proofs arrive minutes apart is still caught.
        let reverse = self
            .audit
            .seen
            .get(&(pou.downloader, pou.uploader, pou.tree_id.0, pou.segment.0))
            .map(|s| s.pou.clone())
            .filter(|r| self.symmetric_pair(pou, r));
        self.audit.seen.insert(
            key,
            Sampled {
                pou: pou.clone(),
                at: self.now,
            },
        );
        let Some(rev) = reverse else { return };
        // RULE: Suppress the detector below N_collusion peers. Symmetric low-degree
        //       edges are the expected topology of a small swarm (Ch5 §5.3.1).
        let n = self.n_guard();
        if n < self.params.n_collusion {
            return;
        }
        let opportunity = (n.saturating_sub(1) as usize).min(self.params.c_a + self.params.c_p);
        if opportunity == 0 {
            return;
        }
        for suspect in [pou.uploader, rev.uploader] {
            let degree = self
                .audit
                .degree
                .get(&suspect)
                .map(|d| d.len())
                .unwrap_or(1);
            let confidence = 1.0 - degree as f64 / opportunity as f64;
            if confidence > 0.5 {
                self.accuse(
                    suspect,
                    EvidenceType::SymmetricTreePair,
                    pou.clone(),
                    rev.clone(),
                );
            }
        }
    }

    /// Two receipts that name each other's signer as uploader for one segment of one
    /// tree, with bytes within the tolerance (Ch5 §5.3.1).
    ///
    /// RULE: Both receipts must cover the same segment and claim one common block.
    ///       A parent and child that swap roles within the window produce one receipt
    ///       each way for different segments. Only a forger claims one block both ways.
    fn symmetric_pair(&self, a: &ProofOfUpload, b: &ProofOfUpload) -> bool {
        a.uploader == b.downloader
            && a.downloader == b.uploader
            && a.tree_id == b.tree_id
            && a.tree_id.0 != 0
            && a.segment == b.segment
            && bitmaps_intersect(&a.bitmap, &b.bitmap)
            && !a.rx_flags.pull
            && !b.rx_flags.pull
            && a.timestamp_us.abs_diff(b.timestamp_us) <= self.params.symmetry_window.as_micros()
            && within(
                a.byte_value(),
                b.byte_value(),
                self.params.symmetry_tolerance,
            )
    }

    /// Two receipts by one signer for one (segment, tree, uploader) with different bitmaps.
    fn equivocation(a: &ProofOfUpload, b: &ProofOfUpload) -> bool {
        a.downloader == b.downloader
            && a.segment == b.segment
            && a.tree_id == b.tree_id
            && a.uploader == b.uploader
            && a.bitmap != b.bitmap
    }

    /// A receipt that replaces one with the same key but a different bitmap is an
    /// equivocation by its signer (Ch5 §5.3.3).
    pub(super) fn audit_check_equivocation(&mut self, old: &ProofOfUpload, new: &ProofOfUpload) {
        if Self::equivocation(old, new) {
            self.accuse(
                new.downloader,
                EvidenceType::Equivocation,
                old.clone(),
                new.clone(),
            );
        }
    }

    // ------------------------------------------------------------------ accusing

    fn accuse(
        &mut self,
        suspect: NodeId,
        kind: EvidenceType,
        first: ProofOfUpload,
        second: ProofOfUpload,
    ) {
        if suspect == self.node_id() {
            return;
        }
        let window = self.params.tau_accuse;
        if let Some(at) = self.audit.accused.get(&suspect) {
            if self.now.duration_since(*at) < window {
                return;
            }
        }
        let mut signers: Vec<(PublicKeyBytes, u64)> = Vec::with_capacity(2);
        for r in [&first, &second] {
            let Some(k) = self.incentive.known_keys.get(&r.downloader) else {
                return;
            };
            signers.push(*k);
        }
        let mut acc = Accusation {
            suspect,
            evidence_type: kind,
            first,
            second,
            signers,
            timestamp_us: self.now.as_micros(),
            signature: bs_wire::SignatureBytes::ZERO,
        };
        acc.signature = self.identity.sign(&acc.signable_bytes());
        self.audit.accused.insert(suspect, self.now);
        self.audit.accusations_sent += 1;
        self.emit(Event::AccusationMade {
            suspect,
            evidence: match kind {
                EvidenceType::SymmetricTreePair => "symmetric_tree_pair",
                EvidenceType::Equivocation => "equivocation",
            },
        });
        // Our own re-verified evidence is one counted accuser: ourselves.
        let me = self.node_id();
        let my_prefix = prefix_key(self.cfg.addr.ip());
        self.audit
            .verified
            .entry(suspect)
            .or_default()
            .insert(me, (my_prefix, self.now));
        self.audit_evaluate(suspect);
        self.gossip_accusation(acc, None);
    }

    /// Send an accusation to the Active Set (all but `except`), with our proof unless
    /// the receiver is a tree parent or child of ours.
    fn gossip_accusation(&mut self, acc: Accusation, except: Option<PeerAddr>) {
        let peers: Vec<PeerAddr> = self
            .membership
            .active
            .keys()
            .copied()
            .filter(|p| Some(*p) != except)
            .filter(|p| self.sessions.get(p).map(|s| s.open).unwrap_or(false))
            .collect();
        if peers.is_empty() {
            return;
        }
        let proof = self.build_rank_proof().map(Box::new);
        let tree_peers = self.tree_peer_addrs();
        let me = self.node_id();
        for p in peers {
            let g = AuditGossip {
                accuser: me,
                accuser_key: (self.identity.public_key(), self.identity.static_nonce()),
                rank_proof: if tree_peers.contains(&p) {
                    None
                } else {
                    proof.clone()
                },
                accusations: vec![acc.clone()],
            };
            self.send(p, Channel::Control, Frame::AuditGossip(g), None);
        }
    }

    // ------------------------------------------------------------------ receiving

    pub(super) fn on_audit_gossip(&mut self, from: PeerAddr, g: AuditGossip) {
        let now = self.now;
        // Rate limit: at most `accusations_per_minute` per accuser, the rest unread.
        let q = self.audit.rate.entry(g.accuser).or_default();
        while q
            .front()
            .map(|t| now.duration_since(*t) > Duration::from_secs(60))
            .unwrap_or(false)
        {
            q.pop_front();
        }
        if q.len() >= self.params.accusations_per_minute as usize {
            self.audit.accusations_dropped += 1;
            return;
        }
        q.push_back(now);
        // The carried key must be the accuser's identity (static puzzle, Ch2 §2.2.1).
        let (apk, anonce) = g.accuser_key;
        if bs_crypto::pow::verify_static(&apk, anonce, &g.accuser, self.params.c1).is_err() {
            self.audit.accusations_dropped += 1;
            self.emit(Event::AccusationDropped {
                reason: "accuser_key_invalid",
            });
            return;
        }
        self.incentive.known_keys.insert(g.accuser, (apk, anonce));
        // Standing: a current child's proved rank, or the attached proof.
        let mut standing = self
            .trees
            .iter()
            .flat_map(|t| t.children.values())
            .find(|c| c.node_id == g.accuser)
            .map(|c| c.rank);
        if standing.is_none() {
            if let Some(p) = g.rank_proof.as_deref() {
                standing = self.verify_rank_proof(Some(g.accuser), p).ok();
                if standing.is_some() {
                    // A verified proof is sampled evidence wherever it arrives.
                    let sampled: Vec<ProofOfUpload> = p
                        .pairs
                        .iter()
                        .flat_map(|pair| std::iter::once(&pair.left).chain(pair.right.iter()))
                        .cloned()
                        .collect();
                    for r in &sampled {
                        self.audit_note_sampled(r);
                    }
                }
            }
        }
        let Some(rank) = standing else {
            self.audit.accusations_dropped += 1;
            self.emit(Event::AccusationDropped {
                reason: "accuser_unranked",
            });
            return;
        };
        if rank < self.params.r_accuser {
            self.audit.accusations_dropped += 1;
            self.emit(Event::AccusationDropped {
                reason: "accuser_rank_low",
            });
            return;
        }
        let prefix = if self.sessions.get(&from).and_then(|s| s.node_id) == Some(g.accuser) {
            Some(prefix_key(from.ip()))
        } else {
            self.resolve_addr(&g.accuser).map(prefix_key)
        };
        let Some(prefix) = prefix else {
            // Resolve the accuser through the DHT and retry for a few seconds.
            if self.audit.parked.len() < 64 {
                self.audit.parked.push((g, from, now));
                let target = self
                    .audit
                    .parked
                    .last()
                    .map(|(g, _, _)| g.accuser.0)
                    .unwrap();
                self.start_lookup(
                    target,
                    crate::dht::LookupKind::Node,
                    crate::dht::Purpose::Resolve,
                    0,
                );
            } else {
                self.audit.accusations_dropped += 1;
                self.emit(Event::AccusationDropped {
                    reason: "accuser_unresolved",
                });
            }
            return;
        };
        for acc in g.accusations {
            if now.as_micros().abs_diff(acc.timestamp_us) > self.params.tau_accuse.as_micros() {
                self.audit.accusations_dropped += 1;
                continue;
            }
            if bs_crypto::Verifier::verify(&apk, &acc.signable_bytes(), &acc.signature).is_err()
                || !self.evidence_holds(&acc)
            {
                // SECURITY: Invalid evidence is a forgery by the accuser: refuse the accuser
                //           for the eviction cooldown and never forward (Ch5 §5.3.3).
                self.audit.accusations_dropped += 1;
                self.emit(Event::AccusationDropped {
                    reason: "evidence_invalid",
                });
                self.ban_peer(from, g.accuser);
                return;
            }
            let fresh = self
                .audit
                .verified
                .entry(acc.suspect)
                .or_default()
                .insert(g.accuser, (prefix, now))
                .is_none();
            self.audit.accusations_counted += 1;
            self.emit(Event::AccusationCounted {
                suspect: acc.suspect,
                accuser: g.accuser,
            });
            self.audit_evaluate(acc.suspect);
            // RULE: Forward only what we counted, once per (accuser, suspect).
            if fresh {
                let fwd = AuditGossip {
                    accuser: g.accuser,
                    accuser_key: g.accuser_key,
                    rank_proof: g.rank_proof.clone(),
                    accusations: vec![acc],
                };
                self.forward_gossip(fwd, from);
            }
        }
    }

    fn forward_gossip(&mut self, g: AuditGossip, except: PeerAddr) {
        let peers: Vec<PeerAddr> = self
            .membership
            .active
            .keys()
            .copied()
            .filter(|p| *p != except)
            .filter(|p| self.sessions.get(p).map(|s| s.open).unwrap_or(false))
            .collect();
        for p in peers {
            self.send(p, Channel::Control, Frame::AuditGossip(g.clone()), None);
        }
    }

    /// Re-run the offence check on the evidence, signatures included.
    fn evidence_holds(&self, acc: &Accusation) -> bool {
        if acc.signers.len() != 2 {
            return false;
        }
        for (r, (pk, nonce)) in [&acc.first, &acc.second]
            .into_iter()
            .zip(acc.signers.iter())
        {
            if bs_crypto::pow::verify_static(pk, *nonce, &r.downloader, self.params.c1).is_err() {
                return false;
            }
            if bs_crypto::Verifier::verify(pk, &r.signable_bytes(), &r.signature).is_err() {
                return false;
            }
            if r.stream_key != self.k_s_hash() {
                return false;
            }
        }
        match acc.evidence_type {
            EvidenceType::SymmetricTreePair => {
                self.symmetric_pair(&acc.first, &acc.second)
                    && (acc.suspect == acc.first.uploader || acc.suspect == acc.second.uploader)
            }
            EvidenceType::Equivocation => {
                Self::equivocation(&acc.first, &acc.second) && acc.suspect == acc.first.downloader
            }
        }
    }

    // ------------------------------------------------------------------ eviction

    /// Evict when three accusers with standing span three prefixes (Ch5 §5.3.3).
    fn audit_evaluate(&mut self, suspect: NodeId) {
        let now = self.now;
        let window = self.params.tau_accuse;
        let Some(accusers) = self.audit.verified.get_mut(&suspect) else {
            return;
        };
        accusers.retain(|_, (_, at)| now.duration_since(*at) <= window);
        let prefixes: BTreeSet<[u8; 8]> = accusers.values().map(|(p, _)| *p).collect();
        if accusers.len() >= 3 && prefixes.len() >= 3 && !self.audit_banned(&suspect) {
            self.audit_evict(suspect);
        }
    }

    fn audit_evict(&mut self, suspect: NodeId) {
        let strikes = self
            .audit
            .banned
            .get(&suspect)
            .map(|(_, s)| *s)
            .unwrap_or(0)
            + 1;
        // RULE: The ban is local and bounded: τ_ban on the first eviction, doubled on
        //       each repeat, capped at `ban_cap` (Ch5 §5.3.3).
        let secs = (self.params.tau_ban.as_secs_f64() * 2f64.powi(strikes as i32 - 1))
            .min(self.params.ban_cap.as_secs_f64());
        let until = self.now + Duration::from_secs_f64(secs);
        self.audit.banned.insert(suspect, (until, strikes));
        self.audit.evictions += 1;
        self.emit(Event::ConsensusEviction { suspect, strikes });
        // Sever: tree edges, membership, pools and sessions.
        let addrs: Vec<PeerAddr> = self
            .sessions
            .iter()
            .filter(|(_, s)| s.node_id == Some(suspect))
            .map(|(a, _)| *a)
            .collect();
        for a in addrs {
            let parent_trees: Vec<TreeId> = self
                .trees
                .iter()
                .filter(|t| t.parent.as_ref().map(|p| p.addr == a).unwrap_or(false))
                .map(|t| t.id)
                .collect();
            for t in parent_trees {
                self.lose_parent(t, "consensus eviction");
            }
            self.remove_child(a, "consensus eviction");
            self.active_remove(a, "consensus eviction");
            self.sessions.remove(&a);
            self.out.push_back(crate::io::Output::CloseSession(a));
        }
        self.passive.remove(&suspect);
    }

    /// Housekeeping at the TFT tick: retry parked accusations, drop expired bans and
    /// stale samples.
    pub(super) fn audit_tick(&mut self) {
        let now = self.now;
        let keep = self.params.tau_accuse;
        self.audit
            .seen
            .retain(|_, s| now.duration_since(s.at) <= keep);
        let parked = std::mem::take(&mut self.audit.parked);
        for (g, from, since) in parked {
            if self.resolve_addr(&g.accuser).is_some() {
                self.on_audit_gossip(from, g);
            } else if now.duration_since(since) < Duration::from_secs(5) {
                self.audit.parked.push((g, from, since));
            } else {
                self.audit.accusations_dropped += 1;
                self.emit(Event::AccusationDropped {
                    reason: "accuser_unresolved",
                });
            }
        }
        self.audit.banned.retain(|_, (until, _)| {
            // Keep the strike count for a day after expiry so repeats double.
            *until + self.params.ban_cap > now
        });
    }

    // ------------------------------------------------------------------ test hooks

    /// Drop the parent of `tree` so the next join presents a fresh rank proof.
    ///
    /// The collusion behaviour of adversarial scenarios: a colluder shops its pumped
    /// rank for a better slot. Not used by honest nodes.
    #[doc(hidden)]
    pub fn shop_parent(&mut self, tree: TreeId) {
        let idx = tree.index();
        let attached = self.trees[idx]
            .parent
            .as_ref()
            .map(|p| !p.pending)
            .unwrap_or(false);
        if attached && self.trees[idx].depth >= 1 && self.trees[idx].old_parent.is_none() {
            let old = self.trees[idx].parent.as_ref().map(|p| p.node_id);
            self.lose_parent(tree, "shop");
            self.trees[idx].avoid_parent = old;
        }
    }

    /// Sign a receipt for `uploader` that attests bytes never received.
    ///
    /// The collusion behaviour of adversarial scenarios. Not used by honest nodes.
    #[doc(hidden)]
    pub fn forge_receipt(&mut self, to: PeerAddr, uploader: NodeId, tree: TreeId) {
        let edge = self.swarm.live_edge;
        if edge.0 < 2 {
            return;
        }
        if !self.sessions.get(&to).map(|s| s.open).unwrap_or(false) {
            self.open_session(to);
            return;
        }
        // RULE: Do not forge against the real parent. The genuine receipt for the same
        //       key would make the pair an equivocation.
        let real_parent = self
            .trees
            .iter()
            .any(|t| t.id == tree && t.parent.as_ref().map(|p| p.node_id) == Some(uploader));
        if real_parent {
            return;
        }
        let rec = self.peer_record();
        let blocks = self
            .matrix
            .as_ref()
            .and_then(|m| m.tree(tree))
            .map(|_| 12u16)
            .unwrap_or(12);
        let mut bitmap = vec![0u8; 64];
        for c in 0..4u8 {
            for j in 0..blocks.min(128) {
                let bit = c as usize * 128 + j as usize;
                bitmap[bit / 8] |= 0x80 >> (bit % 8);
            }
        }
        let mut pou = ProofOfUpload {
            segment: bs_wire::SegmentSeq(edge.0 - 1),
            tree_id: tree,
            rx_flags: bs_wire::frames::RxFlags::default(),
            loss_rate: 0,
            bitmap: bytes::Bytes::from(bitmap),
            k_avail: 0,
            assigned_trees: rec.assigned_trees,
            node_class: rec.node_class,
            stream_key: self.k_s_hash(),
            uploader,
            downloader: self.node_id(),
            timestamp_us: self.now.as_micros(),
            signature: bs_wire::SignatureBytes::ZERO,
        };
        pou.signature = self.identity.sign(&pou.signable_bytes());
        self.send(to, Channel::Control, Frame::ProofOfUpload(pou), None);
    }
}
