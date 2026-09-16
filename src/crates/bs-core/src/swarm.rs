//! The swarm buffer: manifests admitted by sequence window, per-block symbol
//! collection, verify-then-forward, the playout timeline and retention
//! (Ch4 §4.1–4.3, Ch7 §7.1.2).

use std::collections::{BTreeMap, HashMap};

use bs_media::chunk::{verify_block, verify_manifest, BuiltChunk};
use bs_media::{LayeredChunk, MediaError, SlicingMatrix, SymbolCollector};
use bs_wire::consts::{BLOCK_SIZE, CHUNKS_PER_SEGMENT};
use bs_wire::frames::{Manifest, ManifestBody};
use bs_wire::{BlockIndex, Hash, NodeId, PublicKeyBytes, SegmentSeq, TreeId};
use bytes::Bytes;

use crate::params::Params;
use crate::time::{Duration, Instant};

/// State of one block.
#[derive(Debug)]
pub enum BlockSlot {
    /// Collecting symbols and/or waiting for its proof.
    Pending {
        /// Symbols so far.
        collector: SymbolCollector,
        /// Proof (siblings, sender depth, sender id) once received.
        proof: Option<(Vec<Hash>, u8, NodeId)>,
        /// Verification attempts that failed (poisoned data from a parent).
        failures: u8,
    },
    /// Verified bytes with the Merkle siblings, kept so the block can be served
    /// on the pull path (`BLOCK_TRANSMISSION` carries the proof inline).
    Verified {
        /// Block bytes.
        bytes: Bytes,
        /// Sister hashes leaf → root.
        siblings: Vec<Hash>,
    },
}

/// One chunk's buffer.
#[derive(Debug)]
pub struct ChunkBuf {
    /// The verified manifest body.
    pub manifest: ManifestBody,
    /// Blocks by `j`.
    pub blocks: Vec<BlockSlot>,
    /// Verified count.
    pub verified: u16,
    /// Tree carrying each block.
    pub tree_of: Vec<TreeId>,
    /// Layer of each block.
    pub layer_of: Vec<u8>,
    /// Playout deadline.
    pub deadline: Instant,
    /// Whether the deadline has fired.
    pub played: bool,
    /// When the manifest was accepted.
    pub accepted_at: Instant,
    /// The source signature, kept so the manifest can be re-served.
    pub signature: bs_wire::SignatureBytes,
    /// Accepted behind the live edge (late-join backfill), so its blocks are pulled,
    /// never pushed; playout statistics treat it as best effort.
    pub backfill: bool,
}

impl ChunkBuf {
    /// Whether every block of `tree` in this chunk is verified.
    pub fn complete_for_tree(&self, tree: TreeId) -> bool {
        self.blocks
            .iter()
            .zip(&self.tree_of)
            .filter(|(_, t)| **t == tree)
            .all(|(b, _)| matches!(b, BlockSlot::Verified { .. }))
    }
    /// Blocks of `tree` in this chunk.
    pub fn block_count_for_tree(&self, tree: TreeId) -> usize {
        self.tree_of.iter().filter(|t| **t == tree).count()
    }
    /// Whether layer `l` is fully verified.
    pub fn layer_complete(&self, l: u8) -> bool {
        self.blocks
            .iter()
            .zip(&self.layer_of)
            .filter(|(_, ll)| **ll == l)
            .all(|(b, _)| matches!(b, BlockSlot::Verified { .. }))
    }
    /// Missing block count.
    pub fn missing(&self) -> u16 {
        self.blocks.len() as u16 - self.verified
    }
    /// Verified bytes of block `j`.
    pub fn block(&self, j: u16) -> Option<&Bytes> {
        match self.blocks.get(j as usize) {
            Some(BlockSlot::Verified { bytes, .. }) => Some(bytes),
            _ => None,
        }
    }
    /// Verified bytes and proof of block `j`.
    pub fn block_with_proof(&self, j: u16) -> Option<(Bytes, Vec<Hash>)> {
        match self.blocks.get(j as usize) {
            Some(BlockSlot::Verified { bytes, siblings }) => {
                Some((bytes.clone(), siblings.clone()))
            }
            _ => None,
        }
    }
    /// Symbols held for a pending block (0 if verified or unknown).
    pub fn symbols_held(&self, j: u16) -> usize {
        match self.blocks.get(j as usize) {
            Some(BlockSlot::Pending { collector, .. }) => collector.count(),
            _ => 0,
        }
    }
}

/// A block that has just been verified and should be forwarded.
#[derive(Debug, Clone)]
pub struct VerifiedBlock {
    /// Segment.
    pub segment: SegmentSeq,
    /// Block index.
    pub index: BlockIndex,
    /// Tree that carries it.
    pub tree: TreeId,
    /// Layer.
    pub layer: u8,
    /// Bytes.
    pub bytes: Bytes,
    /// Merkle siblings (for our own BLOCK_PROOFs).
    pub siblings: Vec<Hash>,
    /// Sender's advertised depth (we are one deeper).
    pub sender_depth: u8,
    /// Whether RaptorQ decoding was needed.
    pub decoded: bool,
}

/// A block the scheduler may pull.
#[derive(Debug, Clone)]
pub struct MissingBlock {
    /// Segment.
    pub segment: SegmentSeq,
    /// Block index.
    pub index: BlockIndex,
    /// Playout deadline of its chunk.
    pub deadline: Instant,
    /// Symbols already held (0..16).
    pub symbols_held: usize,
    /// Tree that carries it.
    pub tree: TreeId,
    /// Chunk accepted behind the live edge (best effort).
    pub backfill: bool,
}

/// A chunk at its deadline.
#[derive(Debug)]
pub struct PlayedChunk {
    /// Chunk with the leading complete layers only.
    pub chunk: LayeredChunk,
    /// Leading complete layers.
    pub layers_complete: u8,
    /// Total layers in the manifest.
    pub layers_total: u8,
    /// Missing blocks across all layers.
    pub missing: u16,
    /// Backfilled (accepted behind the live edge).
    pub backfill: bool,
}

/// Why a manifest was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestVerdict {
    /// New and accepted.
    Accepted,
    /// Already held (never penalised, Ch7 §7.1.2).
    Duplicate,
    /// Signature or stream id wrong.
    BadSignature,
    /// Below the playout floor or too far ahead of the live edge.
    OutsideWindow,
    /// Slicing matrix version we do not hold.
    UnknownMatrix,
}

/// The node's media buffer.
#[derive(Debug)]
pub struct SwarmBuffer {
    segments: BTreeMap<u32, Vec<Option<ChunkBuf>>>,
    /// Highest segment for which a manifest was accepted.
    pub live_edge: SegmentSeq,
    /// Playout anchor: (segment, chunk, when the first manifest arrived).
    anchor: Option<(SegmentSeq, u8, Instant)>,
    /// Symbols and proofs that arrived before their manifest, keyed by (segment, block index).
    orphans: HashMap<(u32, u16), OrphanBlock>,
    /// Segments whose chunks have all been played (floor for the acceptance window).
    played_through: SegmentSeq,
    /// Whether any chunk has reached its deadline yet.
    any_played: bool,
    delta_buffer: Duration,
    chunk_period: Duration,
    retain: Duration,
    window_ahead: u32,
}

#[derive(Debug, Default)]
struct OrphanBlock {
    symbols: Vec<(u16, Bytes)>,
    proof: Option<(Vec<Hash>, u8, NodeId)>,
    since: Option<Instant>,
}

impl SwarmBuffer {
    /// New buffer with the timing parameters.
    pub fn new(params: &Params) -> Self {
        Self {
            segments: BTreeMap::new(),
            live_edge: SegmentSeq::NONE,
            anchor: None,
            orphans: HashMap::new(),
            played_through: SegmentSeq::NONE,
            any_played: false,
            delta_buffer: params.delta_buffer,
            chunk_period: params.chunk_period,
            retain: params.tau_retain,
            window_ahead: params.manifest_window_ahead,
        }
    }

    fn deadline_for(&self, seg: SegmentSeq, chunk: u8, now: Instant) -> Instant {
        match self.anchor {
            None => now + self.delta_buffer,
            Some((s0, c0, t0)) => {
                let idx = (seg.0 as i64 - s0.0 as i64) * CHUNKS_PER_SEGMENT as i64
                    + (chunk as i64 - c0 as i64);
                let base = t0 + self.delta_buffer;
                if idx >= 0 {
                    base + Duration(self.chunk_period.0 * idx as u64)
                } else {
                    Instant(base.0.saturating_sub(self.chunk_period.0 * (-idx) as u64))
                }
            }
        }
    }

    /// Whether a manifest for (seg, chunk) is held.
    pub fn has_manifest(&self, seg: SegmentSeq, chunk: u8) -> bool {
        self.chunk(seg, chunk).is_some()
    }

    /// Chunk buffer.
    pub fn chunk(&self, seg: SegmentSeq, chunk: u8) -> Option<&ChunkBuf> {
        self.segments
            .get(&seg.0)
            .and_then(|s| s.get(chunk as usize))
            .and_then(|c| c.as_ref())
    }
    fn chunk_mut(&mut self, seg: SegmentSeq, chunk: u8) -> Option<&mut ChunkBuf> {
        self.segments
            .get_mut(&seg.0)
            .and_then(|s| s.get_mut(chunk as usize))
            .and_then(|c| c.as_mut())
    }

    /// Admit a manifest (Ch7 §7.1.2 sequence window; Ch4 §4.3.1 anchoring).
    pub fn accept_manifest(
        &mut self,
        m: &Manifest,
        publisher: &PublicKeyBytes,
        matrix: &SlicingMatrix,
        now: Instant,
    ) -> ManifestVerdict {
        let seg = m.body.segment;
        let c = m.body.chunk_index;
        if c >= CHUNKS_PER_SEGMENT || !seg.is_some() {
            return ManifestVerdict::OutsideWindow;
        }
        if self.has_manifest(seg, c) {
            return ManifestVerdict::Duplicate;
        }
        if seg <= self.played_through {
            return ManifestVerdict::OutsideWindow;
        }
        if self.live_edge.is_some() && seg.0 > self.live_edge.0 + self.window_ahead {
            return ManifestVerdict::OutsideWindow;
        }
        if m.body.slicing_matrix_version != matrix.version {
            return ManifestVerdict::UnknownMatrix;
        }
        if verify_manifest(m, publisher).is_err() {
            return ManifestVerdict::BadSignature;
        }
        self.insert_manifest(m.body.clone(), m.signature, matrix, now, None);
        ManifestVerdict::Accepted
    }

    fn insert_manifest(
        &mut self,
        body: ManifestBody,
        signature: bs_wire::SignatureBytes,
        matrix: &SlicingMatrix,
        now: Instant,
        built: Option<&BuiltChunk>,
    ) {
        let seg = body.segment;
        let c = body.chunk_index;
        // Anchor the playout timeline at the newest manifest seen before the first
        // chunk plays: joiners receive the in-flight segments' manifests in one
        // burst, and the live edge among them is the one to be 3.0 s behind
        // (Ch4 §4.3.1 "Re-anchor on entry to ACTIVE"; never re-anchor backward).
        let newer = match self.anchor {
            None => true,
            Some((s0, c0, _)) => {
                !self.played_through.is_some() && !self.any_played && (seg, c) > (s0, c0)
            }
        };
        let backfill = self.live_edge.is_some() && seg < self.live_edge && built.is_none();
        if newer {
            self.anchor = Some((seg, c, now));
            let keys: Vec<u32> = self.segments.keys().copied().collect();
            for k in keys {
                for ci in 0..CHUNKS_PER_SEGMENT {
                    let d = self.deadline_for(SegmentSeq(k), ci, now);
                    if let Some(Some(cb)) = self.segments.get_mut(&k).map(|v| &mut v[ci as usize]) {
                        cb.deadline = d;
                    }
                }
            }
        }
        if seg > self.live_edge {
            self.live_edge = seg;
        }
        let n = body.block_count() as usize;
        let mut tree_of = Vec::with_capacity(n);
        let mut layer_of = Vec::with_capacity(n);
        for j in 0..n as u16 {
            let (layer, _) = body.layer_of(j).unwrap_or((0, 0));
            layer_of.push(layer);
            tree_of.push(matrix.tree_for_block(&body, j).unwrap_or(TreeId::BASE));
        }
        let blocks: Vec<BlockSlot> = match built {
            Some(b) => b
                .blocks
                .iter()
                .enumerate()
                .map(|(j, x)| BlockSlot::Verified {
                    bytes: x.clone(),
                    siblings: b.tree.proof(j),
                })
                .collect(),
            None => (0..n)
                .map(|_| BlockSlot::Pending {
                    collector: SymbolCollector::new(),
                    proof: None,
                    failures: 0,
                })
                .collect(),
        };
        let verified = if built.is_some() { n as u16 } else { 0 };
        let deadline = self.deadline_for(seg, c, now);
        let buf = ChunkBuf {
            manifest: body,
            blocks,
            verified,
            tree_of,
            layer_of,
            deadline,
            played: false,
            accepted_at: now,
            backfill,
            signature,
        };
        let slot = self
            .segments
            .entry(seg.0)
            .or_insert_with(|| (0..CHUNKS_PER_SEGMENT).map(|_| None).collect());
        slot[c as usize] = Some(buf);
        // Attach orphans.
        let keys: Vec<(u32, u16)> = self
            .orphans
            .keys()
            .filter(|(s, b)| *s == seg.0 && BlockIndex(*b).chunk() == c)
            .copied()
            .collect();
        for k in keys {
            if let Some(o) = self.orphans.remove(&k) {
                let idx = BlockIndex(k.1);
                if let Some((sib, d, from)) = o.proof {
                    let _ = self.add_proof(seg, idx, sib, d, from, now);
                }
                for (esi, p) in o.symbols {
                    let _ = self.add_symbol(seg, idx, esi, p, now);
                }
            }
        }
    }

    /// Publisher path: insert a chunk we built ourselves (all blocks verified).
    pub fn insert_built(&mut self, built: &BuiltChunk, matrix: &SlicingMatrix, now: Instant) {
        self.insert_manifest(
            built.manifest.body.clone(),
            built.manifest.signature,
            matrix,
            now,
            Some(built),
        );
    }

    /// Signed manifests held for segments `>= from`, in presentation order.
    pub fn manifests_from(&self, from: SegmentSeq) -> Vec<Manifest> {
        self.segments
            .range(from.0..)
            .flat_map(|(_, chunks)| chunks.iter().flatten())
            .map(|cb| Manifest {
                body: cb.manifest.clone(),
                signature: cb.signature,
            })
            .collect()
    }

    /// Record a BLOCK_PROOF. Returns `true` if the block is now ready to verify.
    pub fn add_proof(
        &mut self,
        seg: SegmentSeq,
        idx: BlockIndex,
        siblings: Vec<Hash>,
        sender_depth: u8,
        from: NodeId,
        now: Instant,
    ) -> bool {
        match self.chunk_mut(seg, idx.chunk()) {
            None => {
                if seg > self.played_through {
                    let o = self.orphans.entry((seg.0, idx.0)).or_default();
                    o.proof = Some((siblings, sender_depth, from));
                    o.since.get_or_insert(now);
                }
                false
            }
            Some(cb) => match cb.blocks.get_mut(idx.j() as usize) {
                Some(BlockSlot::Pending {
                    collector, proof, ..
                }) => {
                    if proof.is_none() {
                        *proof = Some((siblings, sender_depth, from));
                    }
                    collector.is_complete()
                }
                _ => false,
            },
        }
    }

    /// Record a symbol. Returns `true` if the block is now ready to verify
    /// (enough symbols and a proof).
    pub fn add_symbol(
        &mut self,
        seg: SegmentSeq,
        idx: BlockIndex,
        esi: u16,
        payload: Bytes,
        now: Instant,
    ) -> bool {
        match self.chunk_mut(seg, idx.chunk()) {
            None => {
                if seg > self.played_through {
                    let o = self.orphans.entry((seg.0, idx.0)).or_default();
                    if o.symbols.len() < 64 {
                        o.symbols.push((esi, payload));
                    }
                    o.since.get_or_insert(now);
                }
                false
            }
            Some(cb) => match cb.blocks.get_mut(idx.j() as usize) {
                Some(BlockSlot::Pending {
                    collector, proof, ..
                }) => {
                    let _ = collector.add(esi, payload);
                    collector.is_complete() && proof.is_some()
                }
                _ => false,
            },
        }
    }

    /// Whether block `idx` is already verified.
    pub fn is_verified(&self, seg: SegmentSeq, idx: BlockIndex) -> bool {
        self.chunk(seg, idx.chunk())
            .map(|c| c.block(idx.j()).is_some())
            .unwrap_or(false)
    }

    /// Attempt to reconstruct and verify a block. On success the slot becomes
    /// `Verified` and the block is returned for forwarding.
    pub fn try_verify(
        &mut self,
        seg: SegmentSeq,
        idx: BlockIndex,
    ) -> Result<Option<VerifiedBlock>, (MediaError, Option<NodeId>)> {
        let Some(cb) = self.chunk_mut(seg, idx.chunk()) else {
            return Ok(None);
        };
        let j = idx.j() as usize;
        let tree = *cb.tree_of.get(j).ok_or((MediaError::Undecodable, None))?;
        let layer = cb.layer_of[j];
        let manifest = cb.manifest.clone();
        let Some(BlockSlot::Pending {
            collector,
            proof,
            failures,
        }) = cb.blocks.get_mut(j)
        else {
            return Ok(None);
        };
        let Some((siblings, sender_depth, from)) = proof.clone() else {
            return Ok(None);
        };
        if !collector.is_complete() {
            return Ok(None);
        }
        let decoded = !collector.all_source();
        let bytes = match collector.decode() {
            Ok(b) => b,
            Err(e) => return Err((e, Some(from))),
        };
        if let Err(e) = verify_block(&manifest, idx, &bytes, &siblings) {
            // Poisoned: drop everything from this attempt and wait for a re-push/pull.
            *failures = failures.saturating_add(1);
            *collector = SymbolCollector::new();
            *proof = None;
            return Err((e, Some(from)));
        }
        cb.blocks[j] = BlockSlot::Verified {
            bytes: bytes.clone(),
            siblings: siblings.clone(),
        };
        cb.verified += 1;
        Ok(Some(VerifiedBlock {
            segment: seg,
            index: idx,
            tree,
            layer,
            bytes,
            siblings,
            sender_depth,
            decoded,
        }))
    }

    /// Whether every chunk of `seg` is present and every block of `tree` in it is verified.
    pub fn segment_complete_for_tree(&self, seg: SegmentSeq, tree: TreeId) -> bool {
        match self.segments.get(&seg.0) {
            None => false,
            Some(chunks) => chunks.iter().all(|c| match c {
                Some(cb) => cb.complete_for_tree(tree),
                None => false,
            }),
        }
    }

    /// Earliest unplayed deadline.
    pub fn next_deadline(&self) -> Option<Instant> {
        self.segments
            .values()
            .flatten()
            .flatten()
            .filter(|c| !c.played)
            .map(|c| c.deadline)
            .min()
    }

    /// Fire every deadline `<= now`, in presentation order.
    pub fn play_due(&mut self, now: Instant) -> Vec<PlayedChunk> {
        let mut out = Vec::new();
        let segs: Vec<u32> = self.segments.keys().copied().collect();
        for s in segs {
            for c in 0..CHUNKS_PER_SEGMENT as usize {
                let Some(Some(cb)) = self.segments.get_mut(&s).map(|v| &mut v[c]) else {
                    continue;
                };
                if cb.played || cb.deadline > now {
                    continue;
                }
                cb.played = true;
                self.any_played = true;
                out.push(assemble(cb));
            }
            // Segment floor: all four chunks played.
            let all_played = self.segments[&s]
                .iter()
                .all(|c| c.as_ref().map(|x| x.played).unwrap_or(false));
            if all_played && SegmentSeq(s) > self.played_through {
                self.played_through = SegmentSeq(s);
            }
        }
        out
    }

    /// Drop segments beyond the retention window and stale orphans.
    pub fn gc(&mut self, now: Instant) {
        let retain = self.retain;
        self.segments.retain(|_, chunks| {
            chunks.iter().any(|c| match c {
                Some(cb) => !cb.played || now.duration_since(cb.deadline) < retain,
                None => true,
            })
        });
        let floor = self.played_through;
        self.orphans.retain(|(s, _), o| {
            SegmentSeq(*s) > floor
                && o.since
                    .map(|t| now.duration_since(t) < Duration::from_secs(4))
                    .unwrap_or(true)
        });
    }

    /// Whether playout has anchored (first manifest seen).
    pub fn anchored(&self) -> bool {
        self.anchor.is_some()
    }
    /// Segments currently buffered.
    pub fn buffered_segments(&self) -> usize {
        self.segments.len()
    }
    /// Orphan blocks waiting for a manifest.
    pub fn orphan_count(&self) -> usize {
        self.orphans.len()
    }

    /// Pull path: a whole block with its inline proof (`BLOCK_TRANSMISSION`).
    /// Verifies against the manifest and stores it; returns the block for forwarding.
    pub fn add_full_block(
        &mut self,
        seg: SegmentSeq,
        idx: BlockIndex,
        bytes: Bytes,
        siblings: Vec<Hash>,
        from: NodeId,
    ) -> Result<Option<VerifiedBlock>, (MediaError, Option<NodeId>)> {
        let Some(cb) = self.chunk_mut(seg, idx.chunk()) else {
            return Ok(None);
        };
        let j = idx.j() as usize;
        if j >= cb.blocks.len() || matches!(cb.blocks[j], BlockSlot::Verified { .. }) {
            return Ok(None);
        }
        let tree = cb.tree_of[j];
        let layer = cb.layer_of[j];
        let mut padded = bytes;
        if padded.len() < BLOCK_SIZE {
            let mut v = padded.to_vec();
            v.resize(BLOCK_SIZE, 0);
            padded = Bytes::from(v);
        }
        if let Err(e) = verify_block(&cb.manifest, idx, &padded, &siblings) {
            return Err((e, Some(from)));
        }
        cb.blocks[j] = BlockSlot::Verified {
            bytes: padded.clone(),
            siblings: siblings.clone(),
        };
        cb.verified += 1;
        Ok(Some(VerifiedBlock {
            segment: seg,
            index: idx,
            tree,
            layer,
            bytes: padded,
            siblings,
            sender_depth: u8::MAX,
            decoded: false,
        }))
    }

    /// Verified bytes and proof for serving a `PULL_REQUEST`.
    pub fn block_with_proof(&self, seg: SegmentSeq, idx: BlockIndex) -> Option<(Bytes, Vec<Hash>)> {
        self.chunk(seg, idx.chunk())
            .and_then(|c| c.block_with_proof(idx.j()))
    }

    /// The availability bitfield of one chunk (Ch4 §4.3.2), if its manifest is held.
    pub fn bitfield(
        &self,
        seg: SegmentSeq,
        chunk: u8,
    ) -> Option<bs_wire::frames::BufferStateBitfield> {
        let cb = self.chunk(seg, chunk)?;
        let held: Vec<bool> = cb
            .blocks
            .iter()
            .map(|b| matches!(b, BlockSlot::Verified { .. }))
            .collect();
        let start = BlockIndex::new(chunk, 0).ok()?;
        Some(bs_wire::frames::BufferStateBitfield::from_flags(
            seg, start, &held,
        ))
    }

    /// Chunks of the most recent `segments` segments that hold a manifest.
    pub fn recent_chunks(&self, segments: u32) -> Vec<(SegmentSeq, u8)> {
        let floor = self.live_edge.0.saturating_sub(segments.saturating_sub(1));
        self.segments
            .range(floor..)
            .flat_map(|(s, chunks)| {
                chunks
                    .iter()
                    .enumerate()
                    .filter_map(move |(c, cb)| cb.as_ref().map(|_| (SegmentSeq(*s), c as u8)))
            })
            .collect()
    }

    /// Blocks still missing in chunks whose deadline is within `window` (the PULL
    /// zone of Ch4 §4.3.1), oldest deadline first.
    pub fn missing_in_window(&self, now: Instant, window: Duration) -> Vec<MissingBlock> {
        let mut out = Vec::new();
        for (s, chunks) in &self.segments {
            for cb in chunks.iter().flatten() {
                if cb.played || cb.deadline <= now || cb.deadline.duration_since(now) > window {
                    continue;
                }
                for (j, slot) in cb.blocks.iter().enumerate() {
                    if let BlockSlot::Pending { collector, .. } = slot {
                        if let Ok(index) = BlockIndex::new(cb.manifest.chunk_index, j as u16) {
                            out.push(MissingBlock {
                                segment: SegmentSeq(*s),
                                index,
                                deadline: cb.deadline,
                                symbols_held: collector.count(),
                                tree: cb.tree_of[j],
                                backfill: cb.backfill,
                            });
                        }
                    }
                }
            }
        }
        // Live chunks first (they are what the viewer is about to render), then
        // best-effort backfill; within each, the closest deadline first.
        out.sort_by_key(|m| (m.backfill, m.deadline));
        out
    }
}

/// Build the delivered chunk: leading complete layers only, each stripped of its
/// padding through the manifest's `LayerByteLength`.
fn assemble(cb: &ChunkBuf) -> PlayedChunk {
    let m = &cb.manifest;
    let layers_total = m.layer_block_counts.len() as u8;
    let mut layers = Vec::new();
    let mut j = 0u16;
    let mut complete = 0u8;
    for (l, &n) in m.layer_block_counts.iter().enumerate() {
        if !cb.layer_complete(l as u8) {
            break;
        }
        let mut v = Vec::with_capacity(n as usize * BLOCK_SIZE);
        for _ in 0..n {
            if let Some(b) = cb.block(j) {
                v.extend_from_slice(b);
            }
            j += 1;
        }
        let Some(bytes) =
            bs_media::chunk::strip_layer(&v, m.layer_byte_lengths.get(l).copied().unwrap_or(0))
        else {
            break;
        };
        layers.push(bytes);
        complete += 1;
    }
    PlayedChunk {
        chunk: LayeredChunk {
            segment: m.segment,
            chunk_index: m.chunk_index,
            timestamp_us: m.timestamp_us,
            layers,
        },
        layers_complete: complete,
        layers_total,
        missing: cb.missing(),
        backfill: cb.backfill,
    }
}
