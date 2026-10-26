//! The source side: builds signed chunks and the Stream Record (Ch4 §4.1, Ch2 §2.3.3).

use bs_crypto::{Identity, Signer};
use bs_media::chunk::BuiltChunk;
use bs_media::{ChunkBuilder, Ladder, LayeredChunk, MediaError, SlicingMatrix};
use bs_wire::frames::ManifestBody;
use bs_wire::{Hash, SegmentSeq, SignatureBytes, SlicingMode, StreamRecord, TreeMappingEntry};

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
    /// Registration sampling exponent `s` published in the record (Ch2 §2.3.2).
    pub sample_log2: u8,
    /// Last segment emitted.
    pub last_segment: SegmentSeq,
    /// Manifest root of the last chunk.
    pub last_root: Hash,
    /// Last emitted chunk's timestamp.
    pub last_ts_us: u64,
    /// The STREAM_DESCRIPTOR in force, if the application set one.
    pub descriptor: Option<bs_wire::frames::StreamDescriptor>,
    /// The announced matrix and its switch segment, until the switch.
    pub pending: Option<(Vec<TreeMappingEntry>, SegmentSeq)>,
    /// Body of the last chunk built; a `MANIFEST_UPDATE` carries it.
    pub last_body: Option<ManifestBody>,
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
            sample_log2: 0,
            last_segment: SegmentSeq::NONE,
            last_root: Hash::ZERO,
            last_ts_us: 0,
            descriptor: None,
            pending: None,
            last_body: None,
        })
    }

    /// Cut later chunks under matrix version `v`.
    pub fn set_matrix_version(&mut self, v: u8) {
        self.builder.set_matrix_version(v);
    }

    /// Cut, hash and sign a chunk.
    pub fn build(&mut self, chunk: &LayeredChunk) -> Result<BuiltChunk, MediaError> {
        let built = self.builder.build(chunk)?;
        self.last_segment = chunk.segment;
        self.last_root = built.manifest.body.merkle_root;
        self.last_ts_us = chunk.timestamp_us;
        self.last_body = Some(built.manifest.body.clone());
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
            register_sample_log2: self.sample_log2,
            swarm_size,
            relay_count,
            live_edge_segment: self.last_segment,
            live_edge_manifest_hash: self.last_root,
            live_edge_timestamp_us: self.last_ts_us,
            effective_segment: self
                .pending
                .as_ref()
                .map(|(_, e)| *e)
                .unwrap_or(SegmentSeq::NONE),
            descriptor_version: self.descriptor.as_ref().map(|d| d.version).unwrap_or(0),
            descriptor_hash: self
                .descriptor
                .as_ref()
                .map(|d| bs_crypto::blake3_hash(&bs_wire::Encode::to_vec(d)))
                .unwrap_or(Hash::ZERO),
            trees: self.matrix.to_entries(),
            trees_next: self
                .pending
                .as_ref()
                .map(|(t, _)| t.clone())
                .unwrap_or_default(),
            signature: SignatureBytes::ZERO,
        };
        r.signature = self.identity.sign(&r.signable_bytes());
        r
    }
}

impl Publisher {
    /// Install (and sign) a descriptor; `version` must increase. Returns the signed copy.
    pub fn set_descriptor(
        &mut self,
        mut d: bs_wire::frames::StreamDescriptor,
    ) -> bs_wire::frames::StreamDescriptor {
        d.stream_id = self.stream_id();
        d.signature = self.identity.sign(&d.signable_bytes());
        self.descriptor = Some(d.clone());
        d
    }
}
