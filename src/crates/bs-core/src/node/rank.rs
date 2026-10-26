//! RANK_PROOF (Ch5 §5.2.2): a joiner commits to its receipt list, a later manifest
//! root selects eight adjacent pairs, and the parent recomputes the rank from the
//! sampled receipts. The rank feeds the admission rule of Ch1 §1.2.2.

use std::collections::{BTreeMap, BTreeSet};

use bs_crypto::merkle::MerkleTree;
use bs_crypto::Signer;
use bs_wire::consts::RANK_PROOF_PAIRS;

/// Segments between the live edge and the nonce manifest of a rank proof.
const RANK_PROOF_NONCE_LAG: u32 = 2;
use bs_wire::frames::{ProofOfUpload, RankPair, RankProof};
use bs_wire::{Encode, Hash, NodeId, SegmentSeq, TreeId};

use super::Node;
use crate::io::{Event, PeerAddr};
use crate::time::Instant;

/// Ranks learned from verified proofs, by peer address.
#[derive(Debug, Default)]
pub struct RankState {
    verified: BTreeMap<PeerAddr, (f64, Instant)>,
    /// Counters.
    pub proofs_sent: u64,
    /// Counters.
    pub proofs_verified: u64,
    /// Counters.
    pub proofs_rejected: u64,
}

fn leaf_hash(pou: &ProofOfUpload) -> Hash {
    Hash(*blake3::hash(&pou.to_vec()).as_bytes())
}

/// Pair index `p_i = Blake3(root ‖ nonce ‖ i) mod ⌈count / 2⌉` (Ch5 §5.2.2 step 3).
fn pair_index(root: &Hash, nonce: &Hash, i: u8, count: u32) -> usize {
    let mut h = blake3::Hasher::new();
    h.update(root.as_bytes());
    h.update(nonce.as_bytes());
    h.update(&[i]);
    let d = h.finalize();
    let v = u64::from_be_bytes(d.as_bytes()[..8].try_into().unwrap());
    let half = (count as u64).div_ceil(2).max(1);
    (v % half) as usize
}

impl Node {
    /// Rank a peer proved to us within `rank_proof_ttl`, else 0.
    pub(super) fn peer_rank(&self, peer: PeerAddr) -> f64 {
        match self.rank.verified.get(&peer) {
            Some((r, at)) if self.now.duration_since(*at) <= self.params.rank_proof_ttl => *r,
            _ => 0.0,
        }
    }

    /// Build a proof over the receipts in the rank window. Returns `None` if the ledger
    /// is empty, a signer key is unknown, or no manifest can serve as the nonce.
    pub fn build_rank_proof(&self) -> Option<RankProof> {
        let edge = self.swarm.live_edge;
        // WHY: Gossip receivers lag the live edge by a few segments. Every verifier
        //      holds a nonce manifest that old. The commitment is dated one segment
        //      before the nonce.
        let nonce_segment = SegmentSeq(edge.0.saturating_sub(RANK_PROOF_NONCE_LAG));
        if nonce_segment.0 < 1 {
            return None;
        }
        let commit_segment = SegmentSeq(nonce_segment.0 - 1);
        let lo = commit_segment
            .0
            .saturating_sub(self.params.rank_window_segments);
        let mut receipts: Vec<&ProofOfUpload> = self
            .incentive
            .receipts
            .iter()
            .map(|r| &r.pou)
            .filter(|p| p.segment.0 >= lo && p.segment.0 <= commit_segment.0)
            .collect();
        if receipts.is_empty() {
            return None;
        }
        // RULE: Sort by the canonical key and keep one receipt per key. The verifier
        //       binds ReceiptCount to distinct receipts through that order (Ch5 §5.2.2).
        receipts.sort_by_key(|p| p.canonical_key());
        receipts.dedup_by_key(|p| p.canonical_key());
        let count = receipts.len() as u32;
        let mut leaves: Vec<Hash> = receipts.iter().map(|p| leaf_hash(p)).collect();
        // RULE: A single receipt forms one pair with the zero leaf. The tree must have at
        //       least two leaves. In a one-leaf tree the leaf is its own root.
        if leaves.len() == 1 {
            leaves.push(Hash::ZERO);
        }
        let tree = MerkleTree::from_leaves(&leaves);
        let root = tree.root();
        let (nonce_chunk, nonce) =
            (0..bs_wire::consts::CHUNKS_PER_SEGMENT)
                .rev()
                .find_map(|c| {
                    self.swarm
                        .chunk(nonce_segment, c)
                        .map(|cb| (c, cb.manifest.merkle_root))
                })?;
        let prover = self.node_id();
        let signature = self.identity.sign(&RankProof::signable_bytes(
            count,
            commit_segment,
            &root,
            &prover,
        ));
        let mut pairs = Vec::with_capacity(RANK_PROOF_PAIRS as usize);
        for i in 0..RANK_PROOF_PAIRS {
            let p = pair_index(&root, &nonce, i, count);
            let left = receipts[2 * p].clone();
            let right = receipts.get(2 * p + 1).map(|r| (*r).clone());
            let mut signers = Vec::with_capacity(2);
            for r in std::iter::once(&left).chain(right.iter()) {
                let (pk, nonce) = self.incentive.known_keys.get(&r.downloader)?;
                signers.push((*pk, *nonce));
            }
            let path: Vec<Hash> = tree.proof(2 * p).into_iter().skip(1).collect();
            pairs.push(RankPair {
                left,
                right,
                signers,
                path,
            });
        }
        Some(RankProof {
            receipt_count: count,
            commit_segment,
            root,
            prover,
            signature,
            nonce_segment,
            nonce_chunk,
            pairs,
        })
    }

    /// Verify a proof from `from` and remember the rank it establishes.
    pub(super) fn on_rank_proof(&mut self, from: PeerAddr, proof: RankProof) {
        let expect = self.sessions.get(&from).and_then(|s| s.node_id);
        match self.verify_rank_proof(expect, &proof) {
            Ok(rank) => {
                self.rank.proofs_verified += 1;
                self.rank.verified.insert(from, (rank, self.now));
                let sampled: Vec<ProofOfUpload> = proof
                    .pairs
                    .iter()
                    .flat_map(|p| std::iter::once(p.left.clone()).chain(p.right.clone()))
                    .collect();
                for r in &sampled {
                    self.audit_note_sampled(r);
                }
                self.emit(Event::RankProofVerified {
                    peer: proof.prover,
                    receipts: proof.receipt_count,
                    rank,
                });
            }
            Err((reason, forged)) => {
                self.rank.proofs_rejected += 1;
                self.rank.verified.remove(&from);
                self.emit(Event::RankProofRejected { reason });
                // SECURITY: A failed cryptographic check is a forged signature (Ch7 §7.1.1).
                //           Refuse the peer for the eviction cooldown. A nonce manifest we
                //           do not hold is not evidence of a forgery.
                if forged {
                    self.ban_peer(from, proof.prover);
                }
            }
        }
    }

    /// Verify `proof` and return the rank it establishes. `expect` is the identity the
    /// prover must have: the session peer, or an accuser. The error says whether the
    /// failure is a forgery.
    pub(super) fn verify_rank_proof(
        &self,
        expect: Option<NodeId>,
        proof: &RankProof,
    ) -> Result<f64, (&'static str, bool)> {
        if let Some(id) = expect {
            if id != proof.prover {
                return Err(("wrong_prover", true));
            }
        }
        if proof.receipt_count == 0 || proof.pairs.len() != RANK_PROOF_PAIRS as usize {
            return Err(("shape", true));
        }
        let Some((pk, _)) = self.incentive.known_keys.get(&proof.prover) else {
            return Err(("unknown_key", false));
        };
        let msg = RankProof::signable_bytes(
            proof.receipt_count,
            proof.commit_segment,
            &proof.root,
            &proof.prover,
        );
        if bs_crypto::Verifier::verify(pk, &msg, &proof.signature).is_err() {
            return Err(("bad_signature", true));
        }
        if proof.nonce_segment <= proof.commit_segment {
            return Err(("nonce_order", true));
        }
        let Some(nonce) = self
            .swarm
            .chunk(proof.nonce_segment, proof.nonce_chunk)
            .map(|cb| cb.manifest.merkle_root)
        else {
            return Err(("nonce_unknown", false));
        };
        // RULE: The receipt window trails the prover's commitment, not the verifier's
        //       edge. A verifier a few segments ahead must accept the same proof.
        let edge = self.swarm.live_edge.0;
        if proof.commit_segment.0 > edge + 2 {
            return Err(("commit_ahead", false));
        }
        let hi = proof.commit_segment.0;
        let lo = hi.saturating_sub(self.params.rank_window_segments);
        let plen = RankProof::path_len(proof.receipt_count);
        let count = proof.receipt_count as usize;
        let mut sampled: Vec<u64> = Vec::with_capacity(16);
        for (i, pair) in proof.pairs.iter().enumerate() {
            let p = pair_index(&proof.root, &nonce, i as u8, proof.receipt_count);
            if pair.path.len() != plen {
                return Err(("path_len", true));
            }
            let right_hash = match &pair.right {
                Some(r) => {
                    if pair.left.canonical_key() >= r.canonical_key() {
                        return Err(("pair_order", true));
                    }
                    leaf_hash(r)
                }
                None => {
                    // RULE: The zero leaf is the right half of the last pair only (odd count).
                    if 2 * p + 1 != count {
                        return Err(("zero_leaf", true));
                    }
                    Hash::ZERO
                }
            };
            let mut siblings = Vec::with_capacity(plen + 1);
            siblings.push(right_hash);
            siblings.extend(pair.path.iter().copied());
            if MerkleTree::root_from_proof(leaf_hash(&pair.left), 2 * p, &siblings) != proof.root {
                return Err(("merkle", true));
            }
            let leaves: Vec<&ProofOfUpload> = std::iter::once(&pair.left)
                .chain(pair.right.iter())
                .collect();
            if pair.signers.len() != leaves.len() {
                return Err(("signers", true));
            }
            for (r, (spk, snonce)) in leaves.iter().zip(pair.signers.iter()) {
                if bs_crypto::pow::verify_static(spk, *snonce, &r.downloader, self.params.c1)
                    .is_err()
                {
                    return Err(("signer_identity", true));
                }
                if bs_crypto::Verifier::verify(spk, &r.signable_bytes(), &r.signature).is_err() {
                    return Err(("receipt_signature", true));
                }
                if r.uploader != proof.prover {
                    return Err(("receipt_uploader", true));
                }
                if r.stream_key != self.k_s_hash() {
                    return Err(("receipt_stream", true));
                }
                if r.segment.0 < lo || r.segment.0 > hi {
                    return Err(("receipt_window", true));
                }
                let weight = if r.rx_flags.relayed { 3 } else { 1 };
                sampled.push(weight * r.byte_value().min(self.receipt_cap(r)));
            }
        }
        let mean = sampled.iter().sum::<u64>() as f64 / sampled.len() as f64;
        let theta_rate = proof.receipt_count as f64 / 60.0 * mean * 8.0;
        let b_bits = self.stream_bitrate_bits();
        let rank = if b_bits > 0.0 {
            theta_rate / b_bits
        } else {
            0.0
        };
        Ok(rank * self.subnet_penalty(proof))
    }

    /// `1 − clustered/resolved` if more than `subnet_cluster_share` of the resolved
    /// sampled signers share one prefix. 1 otherwise, and below N_collusion (Ch5 §5.3.2).
    fn subnet_penalty(&self, proof: &RankProof) -> f64 {
        // RULE: Count each signer once. One child signs many sampled receipts. A
        //       cluster is distinct identities behind one prefix (Ch5 §5.3.2).
        let signers: BTreeSet<NodeId> = proof
            .pairs
            .iter()
            .flat_map(|pair| std::iter::once(&pair.left).chain(pair.right.iter()))
            .map(|r| r.downloader)
            .collect();
        let mut by_prefix: BTreeMap<[u8; 8], u32> = BTreeMap::new();
        let mut resolved = 0u32;
        for id in &signers {
            if let Some(ip) = self.resolve_addr(id) {
                resolved += 1;
                *by_prefix
                    .entry(super::incentive::prefix_key(ip))
                    .or_default() += 1;
            }
        }
        let largest = by_prefix.values().copied().max().unwrap_or(0);
        if resolved == 0
            || (largest as f64) <= self.params.subnet_cluster_share * resolved as f64
            || self.swarm_size < self.params.n_collusion
        {
            return 1.0;
        }
        1.0 - largest as f64 / resolved as f64
    }

    /// Send our proof ahead of a NEIGHBOR for an enhancement tree, when we have rank.
    pub(super) fn send_rank_proof_for(&mut self, tree: TreeId, addr: PeerAddr) {
        let enhancement = self
            .matrix
            .as_ref()
            .and_then(|m| m.tree(tree))
            .map(|t| t.layer > 0)
            .unwrap_or(false);
        if !enhancement {
            return;
        }
        let Some(proof) = self.build_rank_proof() else {
            return;
        };
        self.rank.proofs_sent += 1;
        self.emit(Event::RankProofSent {
            receipts: proof.receipt_count,
            rank: self.rank(),
        });
        self.send(
            addr,
            crate::io::Channel::Control,
            bs_wire::Frame::RankProof(proof),
            None,
        );
    }

    /// Lowest-ranked child of `tree` that is not draining, with its rank.
    pub(super) fn weakest_child(&self, tree: TreeId) -> Option<(PeerAddr, NodeId, f64)> {
        self.trees
            .get(tree.index())?
            .children
            .values()
            .filter(|c| c.draining_until.is_none())
            .min_by(|a, b| {
                a.rank
                    .partial_cmp(&b.rank)
                    .unwrap()
                    .then(b.admitted_at.cmp(&a.admitted_at))
            })
            .map(|c| (c.addr, c.node_id, c.rank))
    }
}
