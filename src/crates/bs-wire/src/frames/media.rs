//! Media-path frames: MANIFEST, MANIFEST_UPDATE, BLOCK_PROOF, RAPTORQ_SYMBOL,
//! BLOCK_TRANSMISSION.

use bytes::{Buf, BufMut, Bytes};

use crate::codec::{
    get_u16, get_u32, get_u64, get_u8, get_vec, put_reserved, skip_reserved, Decode, Encode,
};
use crate::consts::SYMBOL_SIZE;
use crate::error::{Result, WireError};
use crate::records::TreeMappingEntry;
use crate::types::{BlockIndex, Hash, SegmentSeq, SignatureBytes, StreamId};

/// The part of MANIFEST / MANIFEST_UPDATE shared by both (Appendix D §D.4.8).
///
/// ```text
/// [StreamID 32][SegmentSeq 4][ChunkIndex 1][ChunkCount 1][Reserved 2][Timestamp 8 µs][MerkleRoot 32]
/// [BlockCount 2][ChunkByteLength 4][SlicingMatrixVersion 1][LayerCount 1][Reserved 2]
/// [LayerBlockCount 2 × LayerCount]
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestBody {
    /// Stream.
    pub stream_id: StreamId,
    /// Segment (starts at 1).
    pub segment: SegmentSeq,
    /// Chunk within the segment.
    pub chunk_index: u8,
    /// Chunks per segment (4).
    pub chunk_count: u8,
    /// Source timestamp, µs.
    pub timestamp_us: u64,
    /// Merkle root over the chunk's blocks.
    pub merkle_root: Hash,
    /// True byte length of the chunk before padding.
    pub chunk_byte_length: u32,
    /// Slicing matrix version this chunk was cut under.
    pub slicing_matrix_version: u8,
    /// Blocks per layer, lowest layer first; the sum is `BlockCount`.
    pub layer_block_counts: Vec<u16>,
}

impl ManifestBody {
    /// Total blocks in the chunk.
    pub fn block_count(&self) -> u16 {
        self.layer_block_counts.iter().copied().sum()
    }
    /// Which layer block `j` belongs to, and its index within that layer.
    pub fn layer_of(&self, j: u16) -> Option<(u8, u16)> {
        let mut base = 0u16;
        for (l, n) in self.layer_block_counts.iter().enumerate() {
            if j < base + n {
                return Some((l as u8, j - base));
            }
            base += n;
        }
        None
    }
    /// First `j` of layer `l`.
    pub fn layer_base(&self, l: u8) -> Option<u16> {
        if l as usize >= self.layer_block_counts.len() {
            return None;
        }
        Some(self.layer_block_counts[..l as usize].iter().copied().sum())
    }
    fn encoded_len(&self) -> usize {
        32 + 4 + 1 + 1 + 2 + 8 + 32 + 2 + 4 + 1 + 1 + 2 + 2 * self.layer_block_counts.len()
    }
    fn encode<B: BufMut>(&self, buf: &mut B) {
        self.stream_id.encode(buf);
        buf.put_u32(self.segment.0);
        buf.put_u8(self.chunk_index);
        buf.put_u8(self.chunk_count);
        put_reserved(buf, 2);
        buf.put_u64(self.timestamp_us);
        self.merkle_root.encode(buf);
        buf.put_u16(self.block_count());
        buf.put_u32(self.chunk_byte_length);
        buf.put_u8(self.slicing_matrix_version);
        buf.put_u8(self.layer_block_counts.len() as u8);
        put_reserved(buf, 2);
        for n in &self.layer_block_counts {
            buf.put_u16(*n);
        }
    }
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        let stream_id = StreamId::decode(buf)?;
        let segment = SegmentSeq(get_u32(buf, "Manifest.segment")?);
        let chunk_index = get_u8(buf, "Manifest.chunk_index")?;
        let chunk_count = get_u8(buf, "Manifest.chunk_count")?;
        skip_reserved(buf, 2, "Manifest.reserved")?;
        let timestamp_us = get_u64(buf, "Manifest.timestamp")?;
        let merkle_root = Hash::decode(buf)?;
        let block_count = get_u16(buf, "Manifest.block_count")?;
        let chunk_byte_length = get_u32(buf, "Manifest.chunk_byte_length")?;
        let slicing_matrix_version = get_u8(buf, "Manifest.matrix_version")?;
        let layer_count = get_u8(buf, "Manifest.layer_count")?;
        skip_reserved(buf, 2, "Manifest.reserved2")?;
        let mut layer_block_counts = Vec::with_capacity(layer_count as usize);
        for _ in 0..layer_count {
            layer_block_counts.push(get_u16(buf, "Manifest.layer_block_count")?);
        }
        let sum: u32 = layer_block_counts.iter().map(|n| *n as u32).sum();
        if sum != block_count as u32 {
            return Err(WireError::CountMismatch {
                context: "Manifest.BlockCount vs LayerBlockCount",
                declared: block_count as usize,
                present: sum as usize,
            });
        }
        Ok(Self {
            stream_id,
            segment,
            chunk_index,
            chunk_count,
            timestamp_us,
            merkle_root,
            chunk_byte_length,
            slicing_matrix_version,
            layer_block_counts,
        })
    }
}

/// MANIFEST (0x11): body followed by the 64-byte source signature over the body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    /// Signed content.
    pub body: ManifestBody,
    /// Source signature.
    pub signature: SignatureBytes,
}
impl Manifest {
    /// Bytes the signature covers.
    pub fn signable_bytes(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(self.body.encoded_len());
        self.body.encode(&mut v);
        v
    }
}
impl Encode for Manifest {
    fn encoded_len(&self) -> usize {
        self.body.encoded_len() + 64
    }
    fn encode<B: BufMut>(&self, buf: &mut B) {
        self.body.encode(buf);
        self.signature.encode(buf);
    }
}
impl Decode for Manifest {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        Ok(Self {
            body: ManifestBody::decode(buf)?,
            signature: SignatureBytes::decode(buf)?,
        })
    }
}

/// MANIFEST_UPDATE (0x17): body, `[EffectiveSegmentSeq 4][NumTrees 1][TreeMappingEntry × N]`,
/// then the signature over everything preceding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestUpdate {
    /// Signed manifest content.
    pub body: ManifestBody,
    /// First segment emitted on the new layout.
    pub effective_segment: SegmentSeq,
    /// The new slicing matrix.
    pub trees: Vec<TreeMappingEntry>,
    /// Source signature.
    pub signature: SignatureBytes,
}
impl ManifestUpdate {
    /// Bytes the signature covers.
    pub fn signable_bytes(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(self.encoded_len() - 64);
        self.encode_unsigned(&mut v);
        v
    }
    fn encode_unsigned<B: BufMut>(&self, buf: &mut B) {
        self.body.encode(buf);
        buf.put_u32(self.effective_segment.0);
        buf.put_u8(self.trees.len() as u8);
        for t in &self.trees {
            t.encode(buf);
        }
    }
}
impl Encode for ManifestUpdate {
    fn encoded_len(&self) -> usize {
        self.body.encoded_len() + 4 + 1 + self.trees.len() * TreeMappingEntry::LEN + 64
    }
    fn encode<B: BufMut>(&self, buf: &mut B) {
        self.encode_unsigned(buf);
        self.signature.encode(buf);
    }
}
impl Decode for ManifestUpdate {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        let body = ManifestBody::decode(buf)?;
        let effective_segment = SegmentSeq(get_u32(buf, "ManifestUpdate.effective")?);
        let n = get_u8(buf, "ManifestUpdate.num_trees")?;
        let mut trees = Vec::with_capacity(n as usize);
        for _ in 0..n {
            trees.push(TreeMappingEntry::decode(buf)?);
        }
        let signature = SignatureBytes::decode(buf)?;
        Ok(Self {
            body,
            effective_segment,
            trees,
            signature,
        })
    }
}

/// BLOCK_PROOF (0x13):
/// `[SegmentSeq 4][BlockIndex 2][ProofDepth 1][SenderHopDepth 1][Sibling Hashes 32 × depth]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockProof {
    /// Segment.
    pub segment: SegmentSeq,
    /// Block the proof is for.
    pub block: BlockIndex,
    /// Sender's current hop depth in this tree; receiver sets its own to `+1`.
    pub sender_hop_depth: u8,
    /// Sister hashes from leaf to root.
    pub siblings: Vec<Hash>,
}
impl Encode for BlockProof {
    fn encoded_len(&self) -> usize {
        8 + 32 * self.siblings.len()
    }
    fn encode<B: BufMut>(&self, buf: &mut B) {
        buf.put_u32(self.segment.0);
        buf.put_u16(self.block.0);
        buf.put_u8(self.siblings.len() as u8);
        buf.put_u8(self.sender_hop_depth);
        for h in &self.siblings {
            h.encode(buf);
        }
    }
}
impl Decode for BlockProof {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        let segment = SegmentSeq(get_u32(buf, "BlockProof.segment")?);
        let block = BlockIndex(get_u16(buf, "BlockProof.block")?);
        let depth = get_u8(buf, "BlockProof.depth")?;
        let sender_hop_depth = get_u8(buf, "BlockProof.hop_depth")?;
        let mut siblings = Vec::with_capacity(depth as usize);
        for _ in 0..depth {
            siblings.push(Hash::decode(buf)?);
        }
        Ok(Self {
            segment,
            block,
            sender_hop_depth,
            siblings,
        })
    }
}

/// RAPTORQ_SYMBOL (0x12): `[SegmentSeq 4][SBN 2][ESI 2][Payload 1024]`.
/// `SBN` is the global `BlockIndex`; `ESI < 16` are the block's raw slices.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RaptorQSymbol {
    /// Segment.
    pub segment: SegmentSeq,
    /// Block (source block number).
    pub block: BlockIndex,
    /// Encoding symbol id.
    pub esi: u16,
    /// Exactly `SYMBOL_SIZE` bytes.
    pub payload: Bytes,
}
impl RaptorQSymbol {
    /// Encoded length.
    pub const LEN: usize = 8 + SYMBOL_SIZE;
    /// Whether this is a source (systematic) symbol.
    pub fn is_source(&self) -> bool {
        (self.esi as usize) < crate::consts::K_BLOCK
    }
}
impl Encode for RaptorQSymbol {
    fn encoded_len(&self) -> usize {
        Self::LEN
    }
    fn encode<B: BufMut>(&self, buf: &mut B) {
        buf.put_u32(self.segment.0);
        buf.put_u16(self.block.0);
        buf.put_u16(self.esi);
        buf.put_slice(&self.payload[..SYMBOL_SIZE.min(self.payload.len())]);
        for _ in self.payload.len()..SYMBOL_SIZE {
            buf.put_u8(0);
        }
    }
}
impl Decode for RaptorQSymbol {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        let segment = SegmentSeq(get_u32(buf, "RaptorQSymbol.segment")?);
        let block = BlockIndex(get_u16(buf, "RaptorQSymbol.sbn")?);
        let esi = get_u16(buf, "RaptorQSymbol.esi")?;
        let payload = Bytes::from(get_vec(buf, SYMBOL_SIZE, "RaptorQSymbol.payload")?);
        Ok(Self {
            segment,
            block,
            esi,
            payload,
        })
    }
}

/// BLOCK_TRANSMISSION (0x10):
/// `[SegmentSeq 4][BlockIndex 2][ProofPathLength 2][Proof 32 × H][Block ...]`.
/// The block payload is whatever remains (16 KB, zero-padded).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockTransmission {
    /// Segment.
    pub segment: SegmentSeq,
    /// Block.
    pub block: BlockIndex,
    /// Merkle sister hashes.
    pub siblings: Vec<Hash>,
    /// Block bytes.
    pub data: Bytes,
}
impl BlockTransmission {
    /// Decode with knowledge of the payload length (the block is the remainder).
    pub fn decode_with_len<B: Buf>(buf: &mut B, payload_len: usize) -> Result<Self> {
        let start = buf.remaining();
        let segment = SegmentSeq(get_u32(buf, "BlockTransmission.segment")?);
        let block = BlockIndex(get_u16(buf, "BlockTransmission.block")?);
        let h = get_u16(buf, "BlockTransmission.proof_len")? as usize;
        let mut siblings = Vec::with_capacity(h);
        for _ in 0..h {
            siblings.push(Hash::decode(buf)?);
        }
        let consumed = start - buf.remaining();
        let rest = payload_len
            .checked_sub(consumed)
            .ok_or_else(|| WireError::Truncated {
                needed: consumed - payload_len,
                context: "BlockTransmission.data",
            })?;
        let data = Bytes::from(get_vec(buf, rest, "BlockTransmission.data")?);
        Ok(Self {
            segment,
            block,
            siblings,
            data,
        })
    }
}
impl Encode for BlockTransmission {
    fn encoded_len(&self) -> usize {
        8 + 32 * self.siblings.len() + self.data.len()
    }
    fn encode<B: BufMut>(&self, buf: &mut B) {
        buf.put_u32(self.segment.0);
        buf.put_u16(self.block.0);
        buf.put_u16(self.siblings.len() as u16);
        for h in &self.siblings {
            h.encode(buf);
        }
        buf.put_slice(&self.data);
    }
}
impl Decode for BlockTransmission {
    /// Decodes assuming the buffer holds exactly this payload.
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        let len = buf.remaining();
        Self::decode_with_len(buf, len)
    }
}
