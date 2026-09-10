//! RaptorQ FEC: one source block per 16 KB Merkle block, K = 16 symbols of
//! 1024 B, systematic (Ch4 §4.2). Decoding takes the concatenation fast path when
//! every source symbol is present.

use bs_wire::consts::{BLOCK_SIZE, K_BLOCK, SYMBOL_SIZE};
use bs_wire::frames::RaptorQSymbol;
use bs_wire::{BlockIndex, NodeId, SegmentSeq};
use bytes::Bytes;
use raptorq::{
    EncodingPacket, ObjectTransmissionInformation, PayloadId, SourceBlockDecoder,
    SourceBlockEncoder,
};

use crate::MediaError;

fn oti() -> ObjectTransmissionInformation {
    ObjectTransmissionInformation::new(BLOCK_SIZE as u64, SYMBOL_SIZE as u16, 1, 1, 8)
}

/// Parity count for a link with loss rate `rho`:
/// `E = max(1, ceil(clamp(2ρK, 0.05K, 0.30K)))` (Ch4 §4.2.2).
pub fn parity_for_loss(rho: f64) -> u16 {
    let k = K_BLOCK as f64;
    let e = (2.0 * rho * k).clamp(0.05 * k, 0.30 * k).ceil();
    (e as u16).max(1)
}

/// The per-responder repair ESI base of Ch4 §4.1.2:
/// `K + (Blake3(NodeID) mod (2^16 − K − 256))`.
pub fn repair_esi_base(node: &NodeId) -> u16 {
    let h = blake3::hash(node.as_bytes());
    let v = u32::from_be_bytes([
        h.as_bytes()[0],
        h.as_bytes()[1],
        h.as_bytes()[2],
        h.as_bytes()[3],
    ]);
    let range = (1u32 << 16) - K_BLOCK as u32 - 256;
    (K_BLOCK as u32 + (v % range)) as u16
}

/// Encoder for one block.
pub struct FecEncoder {
    enc: SourceBlockEncoder,
    block: Bytes,
    segment: SegmentSeq,
    index: BlockIndex,
}

impl FecEncoder {
    /// Wrap a verified 16 KB block.
    pub fn new(segment: SegmentSeq, index: BlockIndex, block: Bytes) -> Self {
        debug_assert_eq!(block.len(), BLOCK_SIZE);
        let enc = SourceBlockEncoder::new(0, &oti(), &block);
        Self {
            enc,
            block,
            segment,
            index,
        }
    }

    /// The 16 systematic symbols — raw slices, no coding work.
    pub fn source_symbols(&self) -> Vec<RaptorQSymbol> {
        (0..K_BLOCK)
            .map(|i| RaptorQSymbol {
                segment: self.segment,
                block: self.index,
                esi: i as u16,
                payload: self.block.slice(i * SYMBOL_SIZE..(i + 1) * SYMBOL_SIZE),
            })
            .collect()
    }

    /// `count` repair symbols starting at `esi_start` (≥ 16).
    pub fn repair_symbols(&self, esi_start: u16, count: u16) -> Vec<RaptorQSymbol> {
        debug_assert!(esi_start as usize >= K_BLOCK);
        self.enc
            .repair_packets(esi_start as u32, count as u32)
            .into_iter()
            .map(|p| {
                let (pid, data) = p.split();
                RaptorQSymbol {
                    segment: self.segment,
                    block: self.index,
                    esi: pid.encoding_symbol_id() as u16,
                    payload: Bytes::from(data),
                }
            })
            .collect()
    }

    /// Source symbols followed by `parity` repair symbols from ESI 16 — the
    /// push-path emission for one child link.
    pub fn push_symbols(&self, parity: u16) -> Vec<RaptorQSymbol> {
        let mut v = self.source_symbols();
        v.extend(self.repair_symbols(K_BLOCK as u16, parity));
        v
    }
}

/// Accumulates symbols for one block until it can be reconstructed.
#[derive(Debug, Clone)]
pub struct SymbolCollector {
    source: Vec<Option<Bytes>>,
    repair: Vec<(u16, Bytes)>,
    have_source: usize,
}

impl Default for SymbolCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl SymbolCollector {
    /// Empty collector.
    pub fn new() -> Self {
        Self {
            source: vec![None; K_BLOCK],
            repair: Vec::new(),
            have_source: 0,
        }
    }

    /// Add a symbol. Returns `true` if it was new.
    pub fn add(&mut self, esi: u16, payload: Bytes) -> Result<bool, MediaError> {
        if payload.len() != SYMBOL_SIZE {
            return Err(MediaError::BadSymbolSize(payload.len()));
        }
        if (esi as usize) < K_BLOCK {
            let slot = &mut self.source[esi as usize];
            if slot.is_some() {
                return Ok(false);
            }
            *slot = Some(payload);
            self.have_source += 1;
            Ok(true)
        } else {
            if self.repair.iter().any(|(e, _)| *e == esi) {
                return Ok(false);
            }
            self.repair.push((esi, payload));
            Ok(true)
        }
    }

    /// Distinct symbols held.
    pub fn count(&self) -> usize {
        self.have_source + self.repair.len()
    }
    /// Whether at least K symbols are held (decode may be attempted).
    pub fn is_complete(&self) -> bool {
        self.count() >= K_BLOCK
    }
    /// Whether every source symbol is present (fast path).
    pub fn all_source(&self) -> bool {
        self.have_source == K_BLOCK
    }
    /// Symbols still needed to reach K.
    pub fn missing(&self) -> usize {
        K_BLOCK.saturating_sub(self.count())
    }

    /// Reconstruct the block. Concatenates when all source symbols are present;
    /// otherwise runs the RaptorQ decoder.
    pub fn decode(&self) -> Result<Bytes, MediaError> {
        if self.all_source() {
            let mut v = Vec::with_capacity(BLOCK_SIZE);
            for s in &self.source {
                v.extend_from_slice(s.as_ref().unwrap());
            }
            return Ok(Bytes::from(v));
        }
        if !self.is_complete() {
            return Err(MediaError::Undecodable);
        }
        let mut dec = SourceBlockDecoder::new(0, &oti(), BLOCK_SIZE as u64);
        let mut packets = Vec::with_capacity(self.count());
        for (i, s) in self.source.iter().enumerate() {
            if let Some(p) = s {
                packets.push(EncodingPacket::new(PayloadId::new(0, i as u32), p.to_vec()));
            }
        }
        for (esi, p) in &self.repair {
            packets.push(EncodingPacket::new(
                PayloadId::new(0, *esi as u32),
                p.to_vec(),
            ));
        }
        dec.decode(packets)
            .map(Bytes::from)
            .ok_or(MediaError::Undecodable)
    }
}

/// Convenience alias used by consumers.
pub type FecDecoder = SymbolCollector;
