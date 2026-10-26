//! Incentive frames (Ch5): PROOF_OF_UPLOAD (0x20) receipts and the RANK_PROOF (0x19)
//! that presents a committed, sampled bundle of them.

use bytes::{Buf, BufMut, Bytes};

use crate::codec::{
    get_u16, get_u32, get_u64, get_u8, get_vec, put_reserved, skip_reserved, Decode, Encode,
};
use crate::error::{Result, WireError};
use crate::types::{
    Hash, NodeClass, NodeId, PublicKeyBytes, SegmentSeq, SignatureBytes, TreeId, TreeSet,
};

/// `RxFlags` of a receipt (Ch5 §5.2.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct RxFlags {
    /// Bit 0: delivered through an emergent relay bridge (weighted 3×).
    pub relayed: bool,
    /// Bit 1: served by PULL_REQUEST; `TreeID` is then 0.
    pub pull: bool,
}

impl RxFlags {
    /// Pack.
    pub fn to_byte(self) -> u8 {
        (self.relayed as u8) | ((self.pull as u8) << 1)
    }
    /// Unpack (reserved bits must be zero).
    pub fn from_byte(b: u8) -> Result<Self> {
        if b & !0x03 != 0 {
            return Err(WireError::InvalidField {
                field: "RxFlags",
                value: b as u64,
            });
        }
        Ok(Self {
            relayed: b & 1 != 0,
            pull: b & 2 != 0,
        })
    }
}

/// PROOF_OF_UPLOAD (0x20): one signed receipt per segment per tree, downloader → uploader.
/// `[SegmentSeq 4][TreeID 1][RxFlags 1][LossRate 1][BitmapLen 1][BlockBitmap ≤ 64]
///  [K_avail 2][AssignedTrees 1][NodeClass 1][K_s 32][Uploader 32][Downloader 32][Timestamp 8][Signature 64]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofOfUpload {
    /// Segment the receipt covers.
    pub segment: SegmentSeq,
    /// Tree whose stripe it covers (0 with `RxFlags.pull`).
    pub tree_id: TreeId,
    /// Flags.
    pub rx_flags: RxFlags,
    /// `⌊255 ρ⌋`, the downloader's symbol loss on this link over the trailing 2 s.
    pub loss_rate: u8,
    /// Bit `ChunkIndex · 128 + j` set iff block (ChunkIndex, j) was delivered by the
    /// uploader on this tree and verified. At most 64 bytes.
    pub bitmap: Bytes,
    /// Downloader's free slots in `TreeID` (child advertisement).
    pub k_avail: u16,
    /// Downloader's assignment bitmap.
    pub assigned_trees: TreeSet,
    /// Downloader's class.
    pub node_class: NodeClass,
    /// `K_s = Blake3(StreamID)`.
    pub stream_key: Hash,
    /// Uploader.
    pub uploader: NodeId,
    /// Downloader (signer).
    pub downloader: NodeId,
    /// µs.
    pub timestamp_us: u64,
    /// Ed25519 signature by the downloader over every preceding byte.
    pub signature: SignatureBytes,
}

impl ProofOfUpload {
    /// Maximum bitmap length.
    pub const MAX_BITMAP: usize = 64;
    /// Blocks per chunk the bitmap addresses.
    pub const BLOCKS_PER_CHUNK: usize = 128;

    /// Fixed part of the encoding (everything but the bitmap).
    pub const FIXED_LEN: usize = 4 + 1 + 1 + 1 + 1 + 2 + 1 + 1 + 32 + 32 + 32 + 8 + 64;

    /// Bit position of block `j` of chunk `chunk`.
    pub fn bit(chunk: u8, j: u16) -> Option<usize> {
        if j as usize >= Self::BLOCKS_PER_CHUNK {
            return None;
        }
        let p = chunk as usize * Self::BLOCKS_PER_CHUNK + j as usize;
        (p < Self::MAX_BITMAP * 8).then_some(p)
    }

    /// Whether the bitmap has bit `p` set.
    pub fn has_bit(&self, p: usize) -> bool {
        self.bitmap
            .get(p / 8)
            .map(|b| b & (0x80 >> (p % 8)) != 0)
            .unwrap_or(false)
    }

    /// Verified blocks the receipt attests.
    pub fn block_count(&self) -> u32 {
        self.bitmap.iter().map(|b| b.count_ones()).sum()
    }

    /// `16 KB × popcount`.
    pub fn byte_value(&self) -> u64 {
        self.block_count() as u64 * crate::consts::BLOCK_SIZE as u64
    }

    /// Canonical sort key `(SegmentSeq, TreeID, RxFlags, Downloader)` (Ch5 §5.2.2).
    pub fn canonical_key(&self) -> (u32, u8, u8, [u8; 32]) {
        (
            self.segment.0,
            self.tree_id.0,
            self.rx_flags.to_byte(),
            self.downloader.0,
        )
    }

    /// The bytes the downloader signs: the whole body except the signature.
    pub fn signable_bytes(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(self.encoded_len());
        self.encode_prefix(&mut v);
        v
    }

    fn encode_prefix<B: BufMut>(&self, buf: &mut B) {
        buf.put_u32(self.segment.0);
        buf.put_u8(self.tree_id.0);
        buf.put_u8(self.rx_flags.to_byte());
        buf.put_u8(self.loss_rate);
        buf.put_u8(self.bitmap.len() as u8);
        buf.put_slice(&self.bitmap);
        buf.put_u16(self.k_avail);
        buf.put_u8(self.assigned_trees.0);
        buf.put_u8(self.node_class as u8);
        self.stream_key.encode(buf);
        self.uploader.encode(buf);
        self.downloader.encode(buf);
        buf.put_u64(self.timestamp_us);
    }
}

impl Encode for ProofOfUpload {
    fn encoded_len(&self) -> usize {
        Self::FIXED_LEN + self.bitmap.len()
    }
    fn encode<B: BufMut>(&self, buf: &mut B) {
        self.encode_prefix(buf);
        self.signature.encode(buf);
    }
}

impl Decode for ProofOfUpload {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        let segment = SegmentSeq(get_u32(buf, "PoU.segment")?);
        let tree_id = TreeId(get_u8(buf, "PoU.tree")?);
        let rx_flags = RxFlags::from_byte(get_u8(buf, "PoU.flags")?)?;
        let loss_rate = get_u8(buf, "PoU.loss")?;
        let l = get_u8(buf, "PoU.bitmap_len")? as usize;
        if l > Self::MAX_BITMAP {
            return Err(WireError::InvalidField {
                field: "PoU.bitmap_len",
                value: l as u64,
            });
        }
        let bitmap = Bytes::from(get_vec(buf, l, "PoU.bitmap")?);
        let k_avail = get_u16(buf, "PoU.k_avail")?;
        let assigned_trees = TreeSet(get_u8(buf, "PoU.assigned")?);
        let node_class = NodeClass::from_code(get_u8(buf, "PoU.class")?)?;
        let stream_key = Hash::decode(buf)?;
        let uploader = NodeId::decode(buf)?;
        let downloader = NodeId::decode(buf)?;
        let timestamp_us = get_u64(buf, "PoU.timestamp")?;
        let signature = SignatureBytes::decode(buf)?;
        Ok(Self {
            segment,
            tree_id,
            rx_flags,
            loss_rate,
            bitmap,
            k_avail,
            assigned_trees,
            node_class,
            stream_key,
            uploader,
            downloader,
            timestamp_us,
            signature,
        })
    }
}

/// One sampled adjacent pair of a RANK_PROOF with its Merkle path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankPair {
    /// Left leaf.
    pub left: ProofOfUpload,
    /// Right leaf; `None` is the zero leaf (only for the last pair of an odd list).
    pub right: Option<ProofOfUpload>,
    /// Downloader keys of the two leaves as `(public key, static nonce)`, so a
    /// third party can bind each signer to its NodeID (ISSUE-065).
    pub signers: Vec<(PublicKeyBytes, u64)>,
    /// Path from the pair's parent node to the root: `⌈log2 ReceiptCount⌉ − 1` hashes.
    pub path: Vec<Hash>,
}

/// RANK_PROOF (0x19): a commit-then-sample presentation of a receipt list (Ch5 §5.2.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankProof {
    /// Distinct receipts in the committed list.
    pub receipt_count: u32,
    /// Segment during which the list was committed.
    pub commit_segment: SegmentSeq,
    /// Merkle root of the sorted leaf hashes, zero-padded to a power of two.
    pub root: Hash,
    /// Prover (the uploader named by every receipt).
    pub prover: NodeId,
    /// Prover signature over `ReceiptCount ‖ CommitSegmentSeq ‖ ReceiptListRoot ‖ Prover`.
    pub signature: SignatureBytes,
    /// Nonce manifest: `SegmentSeq > CommitSegmentSeq`.
    pub nonce_segment: SegmentSeq,
    /// Nonce chunk.
    pub nonce_chunk: u8,
    /// Sampled pairs (8 in the spec; fewer only when the list is that short).
    pub pairs: Vec<RankPair>,
}

impl RankProof {
    /// Merkle path length for `count` leaves: `⌈log2 count⌉ − 1` (0 for one or two leaves).
    pub fn path_len(count: u32) -> usize {
        let n = count.max(2);
        let levels = 32 - (n - 1).leading_zeros() as usize; // ⌈log2 n⌉
        levels.saturating_sub(1)
    }

    /// Bytes the prover signs.
    pub fn signable_bytes(
        receipt_count: u32,
        commit: SegmentSeq,
        root: &Hash,
        prover: &NodeId,
    ) -> Vec<u8> {
        let mut v = Vec::with_capacity(72);
        v.extend_from_slice(&receipt_count.to_be_bytes());
        v.extend_from_slice(&commit.0.to_be_bytes());
        v.extend_from_slice(root.as_bytes());
        v.extend_from_slice(prover.as_bytes());
        v
    }
}

impl Encode for RankProof {
    fn encoded_len(&self) -> usize {
        let mut n = 4 + 4 + 32 + 32 + 64 + 4 + 1 + 1 + 2;
        for p in &self.pairs {
            n += 2
                + p.left.encoded_len()
                + 2
                + p.right.as_ref().map(|r| r.encoded_len()).unwrap_or(0);
            n += 1 + p.signers.len() * 40;
            n += 32 * p.path.len();
        }
        n
    }
    fn encode<B: BufMut>(&self, buf: &mut B) {
        buf.put_u32(self.receipt_count);
        buf.put_u32(self.commit_segment.0);
        self.root.encode(buf);
        self.prover.encode(buf);
        self.signature.encode(buf);
        buf.put_u32(self.nonce_segment.0);
        buf.put_u8(self.nonce_chunk);
        buf.put_u8(self.pairs.len() as u8);
        put_reserved(buf, 2);
        for p in &self.pairs {
            buf.put_u16(p.left.encoded_len() as u16);
            p.left.encode(buf);
            match &p.right {
                Some(r) => {
                    buf.put_u16(r.encoded_len() as u16);
                    r.encode(buf);
                }
                None => buf.put_u16(0),
            }
            buf.put_u8(p.signers.len() as u8);
            for (pk, nonce) in &p.signers {
                pk.encode(buf);
                buf.put_u64(*nonce);
            }
            for h in &p.path {
                h.encode(buf);
            }
        }
    }
}

impl Decode for RankProof {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        let receipt_count = get_u32(buf, "RankProof.count")?;
        let commit_segment = SegmentSeq(get_u32(buf, "RankProof.commit")?);
        let root = Hash::decode(buf)?;
        let prover = NodeId::decode(buf)?;
        let signature = SignatureBytes::decode(buf)?;
        let nonce_segment = SegmentSeq(get_u32(buf, "RankProof.nonce_segment")?);
        let nonce_chunk = get_u8(buf, "RankProof.nonce_chunk")?;
        let pair_count = get_u8(buf, "RankProof.pairs")?;
        if pair_count as usize > crate::consts::RANK_PROOF_PAIRS as usize {
            return Err(WireError::InvalidField {
                field: "RankProof.pairs",
                value: pair_count as u64,
            });
        }
        skip_reserved(buf, 2, "RankProof.reserved")?;
        let plen = Self::path_len(receipt_count);
        let mut pairs = Vec::with_capacity(pair_count as usize);
        for _ in 0..pair_count {
            let ll = get_u16(buf, "RankProof.left_len")? as usize;
            let lv = get_vec(buf, ll, "RankProof.left")?;
            let left = ProofOfUpload::from_slice_exact(&lv)?;
            let rl = get_u16(buf, "RankProof.right_len")? as usize;
            let right = if rl == 0 {
                None
            } else {
                let rv = get_vec(buf, rl, "RankProof.right")?;
                Some(ProofOfUpload::from_slice_exact(&rv)?)
            };
            let ns = get_u8(buf, "RankProof.signers")? as usize;
            let mut signers = Vec::with_capacity(ns);
            for _ in 0..ns {
                let pk = PublicKeyBytes::decode(buf)?;
                let nonce = get_u64(buf, "RankProof.signer_nonce")?;
                signers.push((pk, nonce));
            }
            let mut path = Vec::with_capacity(plen);
            for _ in 0..plen {
                path.push(Hash::decode(buf)?);
            }
            pairs.push(RankPair {
                left,
                right,
                signers,
                path,
            });
        }
        Ok(Self {
            receipt_count,
            commit_segment,
            root,
            prover,
            signature,
            nonce_segment,
            nonce_chunk,
            pairs,
        })
    }
}
