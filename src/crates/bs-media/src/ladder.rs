//! The layer ladder: an ordered list of layers with bitrates (Ch1 §1.2.4 §4.2).
//! The reference ladder is L0 = 1.5 Mbps, L1 = 1.5 Mbps, L2 = 3.0 Mbps.

use serde::{Deserialize, Serialize};

/// One layer of the ladder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Layer {
    /// Human label (e.g. "480p", "T1"); not on the wire.
    pub label: String,
    /// Bitrate in kbps.
    pub bitrate_kbps: u32,
}

/// Ordered layers, lowest first. `L0` is the base layer and is never shed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ladder {
    /// Layers, lowest first.
    pub layers: Vec<Layer>,
}

impl Ladder {
    /// The specification's reference ladder (6 Mbps total).
    pub fn reference() -> Self {
        Self {
            layers: vec![
                Layer {
                    label: "L0-480p".into(),
                    bitrate_kbps: 1500,
                },
                Layer {
                    label: "L1-720p".into(),
                    bitrate_kbps: 1500,
                },
                Layer {
                    label: "L2-1080p".into(),
                    bitrate_kbps: 3000,
                },
            ],
        }
    }
    /// A single-layer ladder at the given bitrate.
    pub fn single(bitrate_kbps: u32) -> Self {
        Self {
            layers: vec![Layer {
                label: "L0".into(),
                bitrate_kbps,
            }],
        }
    }
    /// Number of layers.
    pub fn len(&self) -> usize {
        self.layers.len()
    }
    /// Whether the ladder is empty (never for a valid ladder).
    pub fn is_empty(&self) -> bool {
        self.layers.is_empty()
    }
    /// Total bitrate, kbps.
    pub fn total_kbps(&self) -> u32 {
        self.layers.iter().map(|l| l.bitrate_kbps).sum()
    }
    /// Bytes each layer contributes to a chunk of `chunk_ms` at its nominal rate.
    pub fn bytes_per_chunk(&self, chunk_ms: u32) -> Vec<usize> {
        self.layers
            .iter()
            .map(|l| (l.bitrate_kbps as u64 * 1000 / 8 * chunk_ms as u64 / 1000) as usize)
            .collect()
    }
}
