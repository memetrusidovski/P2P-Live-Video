//! Chunk → blocks → Merkle tree → signed manifest (Ch4 §4.1).

use bs_crypto::{MerkleTree, Signer, Verifier};
use bs_wire::consts::{BLOCK_SIZE, CHUNKS_PER_SEGMENT};
use bs_wire::frames::{BlockProof, Manifest, ManifestBody};
use bs_wire::{BlockIndex, Hash, PublicKeyBytes, SegmentSeq, StreamId};
use bytes::Bytes;

use crate::MediaError;

/// Recover a layer's payload from its concatenated, zero-padded blocks using the
/// manifest's `LayerByteLength` (App D §D.4.8).
pub fn strip_layer(padded: &[u8], byte_length: u32) -> Option<Bytes> {
    let n = byte_length as usize;
    if n > padded.len() {
        return None;
    }
    Some(Bytes::copy_from_slice(&padded[..n]))
}

/// 250 ms of stream: one byte-string per layer, lowest layer first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayeredChunk {
    /// Segment (starts at 1).
    pub segment: SegmentSeq,
    /// Chunk within the segment, `0..CHUNKS_PER_SEGMENT`.
    pub chunk_index: u8,
    /// Presentation timestamp, µs.
    pub timestamp_us: u64,
    /// Layer payloads.
    pub layers: Vec<Bytes>,
}

impl LayeredChunk {
    /// Total payload bytes across layers.
    pub fn byte_len(&self) -> usize {
        self.layers.iter().map(|l| l.len()).sum()
    }
}

/// A chunk cut into blocks with its tree and signed manifest.
#[derive(Debug, Clone)]
pub struct BuiltChunk {
    /// The signed manifest.
    pub manifest: Manifest,
    /// Blocks in `j` order, each exactly `BLOCK_SIZE` bytes (zero-padded).
    pub blocks: Vec<Bytes>,
    /// Merkle tree over the blocks.
    pub tree: MerkleTree,
    /// Original per-layer byte lengths (for exact reassembly).
    pub layer_byte_lengths: Vec<u32>,
}

impl BuiltChunk {
    /// The `BLOCK_PROOF` a sender at `hop_depth` emits for block `j`.
    pub fn block_proof(&self, j: u16, hop_depth: u8) -> BlockProof {
        BlockProof {
            segment: self.manifest.body.segment,
            block: BlockIndex::new(self.manifest.body.chunk_index, j)
                .expect("j < 4096 by construction"),
            sender_hop_depth: hop_depth,
            siblings: self.tree.proof(j as usize),
        }
    }
    /// Number of blocks.
    pub fn block_count(&self) -> u16 {
        self.blocks.len() as u16
    }
}

/// Publisher-side builder. Holds the stream identity and matrix version.
pub struct ChunkBuilder<S: Signer> {
    signer: S,
    stream_id: StreamId,
    matrix_version: u8,
}

impl<S: Signer> ChunkBuilder<S> {
    /// New builder for the publisher `signer`.
    pub fn new(signer: S, matrix_version: u8) -> Self {
        let stream_id = bs_crypto::stream_id(&signer.public_key());
        Self {
            signer,
            stream_id,
            matrix_version,
        }
    }
    /// Stream id.
    pub fn stream_id(&self) -> StreamId {
        self.stream_id
    }
    /// Publisher public key.
    pub fn public_key(&self) -> PublicKeyBytes {
        self.signer.public_key()
    }
    /// Bump the slicing matrix version (forest resize / fold).
    pub fn set_matrix_version(&mut self, v: u8) {
        self.matrix_version = v;
    }

    /// Cut a chunk into 16 KB blocks numbered layer-major, build the Merkle tree
    /// and sign the manifest. Each layer's last block is zero-padded; the
    /// manifest's `LayerByteLength` locates the padding (App D §D.4.8).
    pub fn build(&self, chunk: &LayeredChunk) -> Result<BuiltChunk, MediaError> {
        let mut blocks: Vec<Bytes> = Vec::new();
        let mut layer_block_counts = Vec::with_capacity(chunk.layers.len());
        let mut layer_byte_lengths = Vec::with_capacity(chunk.layers.len());
        for layer in &chunk.layers {
            // An empty layer still occupies one (all-zero) block so the tree and the
            // layer-major numbering stay well defined.
            let n = layer.len().div_ceil(BLOCK_SIZE).max(1);
            layer_block_counts.push(n as u16);
            layer_byte_lengths.push(layer.len() as u32);
            let mut padded = Vec::with_capacity(n * BLOCK_SIZE);
            padded.extend_from_slice(layer);
            padded.resize(n * BLOCK_SIZE, 0);
            let padded = Bytes::from(padded);
            for b in 0..n {
                blocks.push(padded.slice(b * BLOCK_SIZE..(b + 1) * BLOCK_SIZE));
            }
        }
        if blocks.len() > bs_wire::consts::MAX_BLOCKS_PER_CHUNK {
            return Err(MediaError::TooManyBlocks {
                blocks: blocks.len(),
            });
        }
        if blocks.is_empty() {
            return Err(MediaError::LayerCountMismatch {
                got: 0,
                expected: 1,
            });
        }
        if blocks.len() > bs_wire::consts::MAX_BLOCKS_PER_CHUNK {
            return Err(MediaError::TooManyBlocks {
                blocks: blocks.len(),
            });
        }
        let tree = MerkleTree::from_blocks(blocks.iter().map(|b| b.as_ref()));
        let body = ManifestBody {
            stream_id: self.stream_id,
            segment: chunk.segment,
            chunk_index: chunk.chunk_index,
            chunk_count: CHUNKS_PER_SEGMENT,
            timestamp_us: chunk.timestamp_us,
            merkle_root: tree.root(),
            slicing_matrix_version: self.matrix_version,
            layer_block_counts,
            layer_byte_lengths: layer_byte_lengths.clone(),
        };
        let mut unsigned = Manifest {
            body,
            signature: bs_wire::SignatureBytes::ZERO,
        };
        let sig = self.signer.sign(&unsigned.signable_bytes());
        unsigned.signature = sig;
        Ok(BuiltChunk {
            manifest: unsigned,
            blocks,
            tree,
            layer_byte_lengths,
        })
    }
}

/// Verify a manifest's signature against the publisher key and that its stream
/// id is the hash of that key (Ch7 §7.1.1).
pub fn verify_manifest(manifest: &Manifest, publisher: &PublicKeyBytes) -> Result<(), MediaError> {
    if manifest.body.stream_id != bs_crypto::stream_id(publisher) {
        return Err(MediaError::BadManifestSignature);
    }
    Verifier::verify(publisher, &manifest.signable_bytes(), &manifest.signature)
        .map_err(|_| MediaError::BadManifestSignature)
}

/// Verify a block against a manifest root using its proof (Ch4 §4.1.2).
pub fn verify_block(
    manifest: &ManifestBody,
    block_index: BlockIndex,
    block: &[u8],
    siblings: &[Hash],
) -> Result<(), MediaError> {
    MerkleTree::verify_block(
        &manifest.merkle_root,
        block,
        block_index.j() as usize,
        siblings,
    )
    .map_err(|_| MediaError::BadBlock {
        segment: manifest.segment.0,
        block: block_index.0,
    })
}
