//! Blake3 Merkle tree over 16 KB blocks (Ch4 §4.1.2).
//!
//! Leaves are `Blake3(block)`. Internal nodes are `Blake3(left ‖ right)`. The leaf
//! count is padded to the next power of two with **zero leaves** (all-zero 32-byte
//! hashes), matching the padding rule the spec states for the `RANK_PROOF` receipt
//! tree, so a proof for any block has exactly `log2(padded_len)` siblings. A single
//! block yields a tree of depth 0 whose root is the leaf hash.

use bs_wire::Hash;

use crate::signing::CryptoError;

/// Hash a block payload into a leaf.
pub fn hash_block(block: &[u8]) -> Hash {
    Hash(*blake3::hash(block).as_bytes())
}

fn hash_pair(l: &Hash, r: &Hash) -> Hash {
    let mut h = blake3::Hasher::new();
    h.update(l.as_bytes());
    h.update(r.as_bytes());
    Hash(*h.finalize().as_bytes())
}

/// A complete Merkle tree with all levels retained for proof generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MerkleTree {
    /// `levels[0]` are the padded leaves; the last level holds the single root.
    levels: Vec<Vec<Hash>>,
    leaf_count: usize,
}

impl MerkleTree {
    /// Build from leaf hashes. Panics on an empty input.
    pub fn from_leaves(leaves: &[Hash]) -> Self {
        assert!(!leaves.is_empty(), "merkle tree needs at least one leaf");
        let padded = leaves.len().next_power_of_two();
        let mut level: Vec<Hash> = leaves.to_vec();
        level.resize(padded, Hash::ZERO);
        let mut levels = vec![level];
        while levels.last().unwrap().len() > 1 {
            let prev = levels.last().unwrap();
            let next: Vec<Hash> = prev.chunks(2).map(|p| hash_pair(&p[0], &p[1])).collect();
            levels.push(next);
        }
        Self {
            levels,
            leaf_count: leaves.len(),
        }
    }

    /// Build directly from block payloads.
    pub fn from_blocks<'a>(blocks: impl IntoIterator<Item = &'a [u8]>) -> Self {
        let leaves: Vec<Hash> = blocks.into_iter().map(hash_block).collect();
        Self::from_leaves(&leaves)
    }

    /// Root hash.
    pub fn root(&self) -> Hash {
        self.levels.last().unwrap()[0]
    }
    /// Number of real (unpadded) leaves.
    pub fn leaf_count(&self) -> usize {
        self.leaf_count
    }
    /// Proof depth: number of sibling hashes per proof.
    pub fn depth(&self) -> usize {
        self.levels.len() - 1
    }

    /// Sibling hashes from leaf level upward for leaf `index`.
    pub fn proof(&self, index: usize) -> Vec<Hash> {
        assert!(index < self.leaf_count, "leaf index out of range");
        let mut out = Vec::with_capacity(self.depth());
        let mut i = index;
        for level in &self.levels[..self.depth()] {
            out.push(level[i ^ 1]);
            i >>= 1;
        }
        out
    }

    /// Recompute a root from a leaf hash, its index and its sibling path.
    pub fn root_from_proof(leaf: Hash, index: usize, siblings: &[Hash]) -> Hash {
        let mut acc = leaf;
        let mut i = index;
        for s in siblings {
            acc = if i & 1 == 0 {
                hash_pair(&acc, s)
            } else {
                hash_pair(s, &acc)
            };
            i >>= 1;
        }
        acc
    }

    /// Verify that `block` at `index` belongs to the tree with `root`.
    pub fn verify_block(
        root: &Hash,
        block: &[u8],
        index: usize,
        siblings: &[Hash],
    ) -> Result<(), CryptoError> {
        if Self::root_from_proof(hash_block(block), index, siblings) == *root {
            Ok(())
        } else {
            Err(CryptoError::MerkleMismatch)
        }
    }
}
