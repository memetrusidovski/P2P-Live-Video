//! Chunk producers. Both are pure: they are polled for the next chunk and never
//! read a clock, so the simulator and the real runtime pace them.

use bs_wire::consts::{BLOCK_SIZE, CHUNKS_PER_SEGMENT};
use bs_wire::SegmentSeq;
use bytes::Bytes;

use crate::chunk::LayeredChunk;
use crate::ladder::Ladder;

/// Chunk period in milliseconds.
pub const CHUNK_MS: u32 = 250;

/// Per-layer byte sizes for one chunk such that every layer **except the last**
/// is a whole number of 16 KB blocks (at least one) and the last layer takes the
/// remainder of the ladder's total. The manifest carries only the chunk's total
/// byte length, so a consumer can recover exact layer boundaries only if the
/// non-final layers carry no padding; this rule guarantees that for opaque
/// payloads (see [`crate::sink::ChunkAssembler`] and ISSUE-060).
pub fn aligned_layer_sizes(ladder: &Ladder, chunk_ms: u32) -> Vec<usize> {
    let nominal = ladder.bytes_per_chunk(chunk_ms);
    let total: usize = nominal.iter().sum();
    let n = nominal.len();
    let mut out = Vec::with_capacity(n);
    let mut used = 0usize;
    for (i, &b) in nominal.iter().enumerate() {
        if i + 1 == n {
            out.push(total.saturating_sub(used).max(1));
        } else {
            let blocks = (b as f64 / BLOCK_SIZE as f64).round().max(1.0) as usize;
            let sz = blocks * BLOCK_SIZE;
            out.push(sz);
            used += sz;
        }
    }
    out
}

/// Something that yields the next 250 ms chunk.
pub trait Source {
    /// The ladder the chunks follow.
    fn ladder(&self) -> &Ladder;
    /// Next chunk, or `None` when the source is exhausted (a file ended).
    fn next_chunk(&mut self) -> Option<LayeredChunk>;
    /// Blake3 hash per layer of everything emitted so far. Compared with the
    /// sink's hashes for the end-to-end correctness check.
    fn layer_hashes(&self) -> Vec<bs_wire::Hash>;
}

/// Segment/chunk counter shared by sources.
#[derive(Debug, Clone)]
struct Cursor {
    segment: SegmentSeq,
    chunk: u8,
    ts_us: u64,
}
impl Cursor {
    fn new(start_ts_us: u64) -> Self {
        Self {
            segment: SegmentSeq::FIRST,
            chunk: 0,
            ts_us: start_ts_us,
        }
    }
    fn advance(&mut self) -> (SegmentSeq, u8, u64) {
        let out = (self.segment, self.chunk, self.ts_us);
        self.chunk += 1;
        if self.chunk == CHUNKS_PER_SEGMENT {
            self.chunk = 0;
            self.segment = self.segment.next();
        }
        self.ts_us += CHUNK_MS as u64 * 1000;
        out
    }
}

/// Deterministic pseudo-random bytes at the ladder's bitrates. Seeded, so two
/// runs produce identical streams and identical hashes.
pub struct SyntheticSource {
    ladder: Ladder,
    per_chunk: Vec<usize>,
    cursor: Cursor,
    state: u64,
    hashers: Vec<blake3::Hasher>,
    remaining_chunks: Option<u64>,
}

impl SyntheticSource {
    /// New source. `duration_chunks = None` runs forever.
    pub fn new(ladder: Ladder, seed: u64, duration_chunks: Option<u64>) -> Self {
        let per_chunk = aligned_layer_sizes(&ladder, CHUNK_MS);
        let n = ladder.len();
        Self {
            ladder,
            per_chunk,
            cursor: Cursor::new(0),
            state: seed ^ 0x9E37_79B9_7F4A_7C15,
            hashers: (0..n).map(|_| blake3::Hasher::new()).collect(),
            remaining_chunks: duration_chunks,
        }
    }
    fn fill(&mut self, n: usize) -> Bytes {
        // xorshift64*; fast, deterministic, good enough for payload bytes.
        let mut v = Vec::with_capacity(n);
        while v.len() < n {
            self.state ^= self.state >> 12;
            self.state ^= self.state << 25;
            self.state ^= self.state >> 27;
            let x = self.state.wrapping_mul(0x2545_F491_4F6C_DD1D);
            let b = x.to_le_bytes();
            let take = (n - v.len()).min(8);
            v.extend_from_slice(&b[..take]);
        }
        Bytes::from(v)
    }
}

impl Source for SyntheticSource {
    fn ladder(&self) -> &Ladder {
        &self.ladder
    }
    fn next_chunk(&mut self) -> Option<LayeredChunk> {
        if let Some(r) = self.remaining_chunks.as_mut() {
            if *r == 0 {
                return None;
            }
            *r -= 1;
        }
        let (segment, chunk_index, timestamp_us) = self.cursor.advance();
        let sizes = self.per_chunk.clone();
        let mut layers = Vec::with_capacity(sizes.len());
        for (i, n) in sizes.into_iter().enumerate() {
            let b = self.fill(n);
            self.hashers[i].update(&b);
            layers.push(b);
        }
        Some(LayeredChunk {
            segment,
            chunk_index,
            timestamp_us,
            layers,
        })
    }
    fn layer_hashes(&self) -> Vec<bs_wire::Hash> {
        self.hashers
            .iter()
            .map(|h| bs_wire::Hash(*h.finalize().as_bytes()))
            .collect()
    }
}

/// A file (e.g. an `.mp4`) streamed as opaque bytes at the ladder's total
/// bitrate, split across layers in proportion to their bitrates. A viewer that
/// reassembles every layer in order and concatenates them layer-by-layer per
/// chunk recovers the file byte for byte — [`crate::sink::ChunkAssembler`]
/// does exactly that.
pub struct FileSource {
    ladder: Ladder,
    per_chunk: Vec<usize>,
    data: Bytes,
    pos: usize,
    cursor: Cursor,
    hashers: Vec<blake3::Hasher>,
    /// Blake3 of the whole file, for the simplest end-to-end check.
    file_hash: bs_wire::Hash,
}

impl FileSource {
    /// Wrap file contents.
    pub fn new(ladder: Ladder, data: Bytes) -> Self {
        let per_chunk = aligned_layer_sizes(&ladder, CHUNK_MS);
        let n = ladder.len();
        let file_hash = bs_wire::Hash(*blake3::hash(&data).as_bytes());
        Self {
            ladder,
            per_chunk,
            data,
            pos: 0,
            cursor: Cursor::new(0),
            hashers: (0..n).map(|_| blake3::Hasher::new()).collect(),
            file_hash,
        }
    }
    /// Blake3 of the source file.
    pub fn file_hash(&self) -> bs_wire::Hash {
        self.file_hash
    }
    /// Total chunks this file yields at the ladder rate.
    pub fn total_chunks(&self) -> u64 {
        let per: usize = self.per_chunk.iter().sum();
        (self.data.len() as u64).div_ceil(per as u64)
    }
    /// Total bytes.
    pub fn len(&self) -> usize {
        self.data.len()
    }
    /// Whether the file is empty.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

impl Source for FileSource {
    fn ladder(&self) -> &Ladder {
        &self.ladder
    }
    fn next_chunk(&mut self) -> Option<LayeredChunk> {
        if self.pos >= self.data.len() {
            return None;
        }
        let (segment, chunk_index, timestamp_us) = self.cursor.advance();
        let mut layers = Vec::with_capacity(self.per_chunk.len());
        for (i, n) in self.per_chunk.iter().enumerate() {
            let end = (self.pos + n).min(self.data.len());
            let b = self.data.slice(self.pos..end);
            self.hashers[i].update(&b);
            layers.push(b);
            self.pos = end;
        }
        Some(LayeredChunk {
            segment,
            chunk_index,
            timestamp_us,
            layers,
        })
    }
    fn layer_hashes(&self) -> Vec<bs_wire::Hash> {
        self.hashers
            .iter()
            .map(|h| bs_wire::Hash(*h.finalize().as_bytes()))
            .collect()
    }
}
