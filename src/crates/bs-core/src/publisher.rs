//! The source side: builds signed chunks and the Stream Record (Ch4 §4.1, Ch2 §2.3.3).

use bs_crypto::{Identity, Signer};
use bs_media::chunk::BuiltChunk;
use bs_media::{ChunkBuilder, Ladder, LayeredChunk, MediaError, SlicingMatrix};
use bs_wire::{Hash, SegmentSeq, SignatureBytes, SlicingMode, StreamRecord};

use crate::time::Instant;

/// Publisher state.
pub struct Publisher {
    builder: ChunkBuilder<Identity>,
    identity: Identity,
    /// Ladder in use.
    pub ladder: Ladder,
    /// Matrix in force.
    pub matrix: SlicingMatrix,
    /// Record version, bumped on every republish.
    pub record_version: u32,
    /// Last segment emitted.
    pub last_segment: SegmentSeq,
    /// Manifest root of the last chunk.
    pub last_root: Hash,
    /// Last emitted chunk's timestamp.
    pub last_ts_us: u64,
}

impl Publisher {
    /// New publisher for `identity` with forest size `m`.
    pub fn new(identity: Identity, ladder: Ladder, m: u8) -> Result<Self, MediaError> {
        let matrix = SlicingMatrix::allocate(1, m, &ladder)?;
        Ok(Self {
            builder: ChunkBuilder::new(identity.clone(), matrix.version),
            identity,
            ladder,
            matrix,
            record_version: 0,
            last_segment: SegmentSeq::NONE,
            last_root: Hash::ZERO,
            last_ts_us: 0,
        })
    }

    /// Cut, hash and sign a chunk.
    pub fn build(&mut self, chunk: &LayeredChunk) -> Result<BuiltChunk, MediaError> {
        let built = self.builder.build(chunk)?;
        self.last_segment = chunk.segment;
        self.last_root = built.manifest.body.merkle_root;
        self.last_ts_us = chunk.timestamp_us;
        Ok(built)
    }

    /// Stream id.
    pub fn stream_id(&self) -> bs_wire::StreamId {
        self.builder.stream_id()
    }

    /// The signed Stream Record for discovery (Ch2 §2.3.3).
    pub fn stream_record(
        &mut self,
        swarm_size: u32,
        relay_count: u32,
        _now: Instant,
    ) -> StreamRecord {
        self.record_version += 1;
        let mut r = StreamRecord {
            publisher_pubkey: self.identity.public_key(),
            manifest_version: self.record_version,
            slicing_mode: SlicingMode::SvcSpatial,
            register_sample_log2: 0,
            swarm_size,
            relay_count,
            live_edge_segment: self.last_segment,
            live_edge_manifest_hash: self.last_root,
            live_edge_timestamp_us: self.last_ts_us,
            effective_segment: SegmentSeq::NONE,
            trees: self.matrix.to_entries(),
            trees_next: Vec::new(),
            signature: SignatureBytes::ZERO,
        };
        r.signature = self.identity.sign(&r.signable_bytes());
        r
    }
}
