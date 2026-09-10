//! Consumer-side reassembly with a running hash — the data-correctness witness.

use std::collections::BTreeMap;

use bs_wire::frames::ManifestBody;
use bs_wire::{Hash, SegmentSeq};
use bytes::Bytes;

use crate::chunk::LayeredChunk;

/// Rebuilds chunks from verified blocks and keeps per-layer running hashes.
/// Layers are recovered through the manifest's `LayerByteLength` (`bs_media::chunk::strip_layer`).
#[derive(Debug, Default)]
pub struct ChunkAssembler {
    hashers: Vec<blake3::Hasher>,
    /// Chunks completed, in order of completion.
    completed: u64,
    /// Bytes delivered to the application per layer.
    bytes: Vec<u64>,
    /// Chunks whose blocks are still arriving: (segment, chunk) → blocks by j.
    pending: BTreeMap<(u32, u8), Pending>,
    /// Output in presentation order, if the consumer wants raw bytes.
    keep_output: bool,
    output: Vec<u8>,
}

#[derive(Debug)]
struct Pending {
    manifest: ManifestBody,
    blocks: BTreeMap<u16, Bytes>,
}

impl ChunkAssembler {
    /// New assembler for `layers` layers. `keep_output` retains the reassembled
    /// bytes in memory (tests and the file writer use it; the simulator does not).
    pub fn new(layers: usize, keep_output: bool) -> Self {
        Self {
            hashers: (0..layers).map(|_| blake3::Hasher::new()).collect(),
            bytes: vec![0; layers],
            keep_output,
            ..Default::default()
        }
    }

    /// Register a chunk's manifest so its blocks can be placed.
    pub fn begin_chunk(&mut self, manifest: &ManifestBody) {
        self.pending
            .entry((manifest.segment.0, manifest.chunk_index))
            .or_insert_with(|| Pending {
                manifest: manifest.clone(),
                blocks: BTreeMap::new(),
            });
    }

    /// Add a verified block. Returns the completed chunk when this was the last block.
    pub fn add_block(
        &mut self,
        segment: SegmentSeq,
        chunk: u8,
        j: u16,
        block: Bytes,
    ) -> Option<LayeredChunk> {
        let key = (segment.0, chunk);
        let p = self.pending.get_mut(&key)?;
        p.blocks.insert(j, block);
        if (p.blocks.len() as u16) < p.manifest.block_count() {
            return None;
        }
        let p = self.pending.remove(&key).unwrap();
        Some(self.finish(p))
    }

    fn finish(&mut self, p: Pending) -> LayeredChunk {
        let m = &p.manifest;
        let mut layers = Vec::with_capacity(m.layer_block_counts.len());
        let mut j = 0u16;
        for (l, &n) in m.layer_block_counts.iter().enumerate() {
            let mut v = Vec::with_capacity(n as usize * bs_wire::consts::BLOCK_SIZE);
            for _ in 0..n {
                if let Some(b) = p.blocks.get(&j) {
                    v.extend_from_slice(b);
                }
                j += 1;
            }
            let bytes =
                crate::chunk::strip_layer(&v, m.layer_byte_lengths.get(l).copied().unwrap_or(0))
                    .unwrap_or_default();
            if l < self.hashers.len() {
                self.hashers[l].update(&bytes);
                self.bytes[l] += bytes.len() as u64;
            }
            if self.keep_output {
                self.output.extend_from_slice(&bytes);
            }
            layers.push(bytes);
        }
        self.completed += 1;
        LayeredChunk {
            segment: m.segment,
            chunk_index: m.chunk_index,
            timestamp_us: m.timestamp_us,
            layers,
        }
    }

    /// Per-layer running hashes.
    pub fn layer_hashes(&self) -> Vec<Hash> {
        self.hashers
            .iter()
            .map(|h| Hash(*h.finalize().as_bytes()))
            .collect()
    }
    /// Chunks completed.
    pub fn completed_chunks(&self) -> u64 {
        self.completed
    }
    /// Bytes delivered per layer.
    pub fn bytes_per_layer(&self) -> &[u64] {
        &self.bytes
    }
    /// Reassembled bytes (only when `keep_output`).
    pub fn output(&self) -> &[u8] {
        &self.output
    }
    /// Blake3 of the reassembled output (only meaningful with `keep_output`).
    pub fn output_hash(&self) -> Hash {
        Hash(*blake3::hash(&self.output).as_bytes())
    }
    /// Chunks still incomplete.
    pub fn pending_chunks(&self) -> usize {
        self.pending.len()
    }
}
