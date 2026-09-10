//! Scenario files (TOML). See `scenarios/README.md` for the schema.

use std::path::Path;

use bs_core::Params;
use bs_media::{Ladder, Layer};
use serde::{Deserialize, Serialize};

/// A whole scenario.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Scenario {
    /// Name (defaults to the file stem).
    pub name: String,
    /// Master seed.
    pub seed: u64,
    /// Virtual duration.
    pub duration_s: f64,
    /// Layer ladder.
    pub ladder: Vec<LayerSpec>,
    /// Forest size M.
    pub forest_size: u8,
    /// Media source.
    pub source: SourceSpec,
    /// The publisher.
    pub publisher: PublisherSpec,
    /// Viewer groups.
    pub viewers: Vec<ViewerGroup>,
    /// Network model.
    pub network: NetworkSpec,
    /// Scripted events.
    pub events: Vec<ScriptedEvent>,
    /// KPI thresholds.
    pub kpis: KpiThresholds,
    /// Protocol parameter overrides (Appendix B names).
    pub params: Params,
    /// Write every node's events (large) or only summary events.
    pub log_all_events: bool,
}

impl Default for Scenario {
    fn default() -> Self {
        Self {
            name: "unnamed".into(),
            seed: 1,
            duration_s: 30.0,
            ladder: LayerSpec::reference(),
            forest_size: 3,
            source: SourceSpec::default(),
            publisher: PublisherSpec::default(),
            viewers: vec![ViewerGroup::default()],
            network: NetworkSpec::default(),
            events: Vec::new(),
            kpis: KpiThresholds::default(),
            params: Params::for_simulation(),
            log_all_events: true,
        }
    }
}

impl Scenario {
    /// Load from a TOML file; the name defaults to the file stem.
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let text = std::fs::read_to_string(path)?;
        let mut s: Scenario = toml::from_str(&text)?;
        if s.name == "unnamed" {
            s.name = path
                .file_stem()
                .map(|x| x.to_string_lossy().into_owned())
                .unwrap_or_else(|| "scenario".into());
        }
        Ok(s)
    }
    /// The ladder as a media type.
    pub fn ladder(&self) -> Ladder {
        Ladder {
            layers: self
                .ladder
                .iter()
                .map(|l| Layer {
                    label: l.label.clone(),
                    bitrate_kbps: l.bitrate_kbps,
                })
                .collect(),
        }
    }
}

/// One layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerSpec {
    /// Label.
    pub label: String,
    /// Bitrate.
    pub bitrate_kbps: u32,
}
impl LayerSpec {
    fn reference() -> Vec<Self> {
        vec![
            LayerSpec {
                label: "L0-480p".into(),
                bitrate_kbps: 1500,
            },
            LayerSpec {
                label: "L1-720p".into(),
                bitrate_kbps: 1500,
            },
            LayerSpec {
                label: "L2-1080p".into(),
                bitrate_kbps: 3000,
            },
        ]
    }
}

/// Media source.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SourceSpec {
    /// Deterministic pseudo-random bytes.
    Synthetic {
        /// Seed (defaults to the scenario seed).
        #[serde(default)]
        seed: Option<u64>,
    },
    /// A file streamed as opaque bytes at the ladder bitrate (loops if shorter than the run).
    File {
        /// Path, relative to the scenario file or absolute.
        path: String,
        /// Loop the file when exhausted.
        #[serde(default = "default_true")]
        repeat: bool,
    },
}
fn default_true() -> bool {
    true
}
impl Default for SourceSpec {
    fn default() -> Self {
        SourceSpec::Synthetic { seed: None }
    }
}

/// Publisher settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PublisherSpec {
    /// Upload capacity.
    pub upload_kbps: u32,
    /// Region.
    pub region: u8,
    /// Start time.
    pub start_s: f64,
}
impl Default for PublisherSpec {
    fn default() -> Self {
        Self {
            upload_kbps: 100_000,
            region: 0,
            start_s: 0.0,
        }
    }
}

/// A group of similar viewers.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ViewerGroup {
    /// Group label.
    pub label: String,
    /// How many.
    pub count: usize,
    /// "relay" or "leaf".
    pub class: String,
    /// Upload range (uniform).
    pub upload_kbps: Range,
    /// Highest layer wanted.
    pub top_layer: u8,
    /// Join window (uniform arrival).
    pub join_s: Range,
    /// Leave window; `None` = stay.
    pub leave_s: Option<Range>,
    /// Region; `None` = random over the network's regions.
    pub region: Option<u8>,
}
impl Default for ViewerGroup {
    fn default() -> Self {
        Self {
            label: "viewers".into(),
            count: 20,
            class: "relay".into(),
            upload_kbps: Range {
                min: 10_000.0,
                max: 10_000.0,
            },
            top_layer: 2,
            join_s: Range { min: 1.0, max: 5.0 },
            leave_s: None,
            region: None,
        }
    }
}

/// Inclusive numeric range.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Range {
    /// Min.
    pub min: f64,
    /// Max.
    pub max: f64,
}

/// Network model.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct NetworkSpec {
    /// Number of regions.
    pub regions: u8,
    /// RTT within a region, ms.
    pub intra_region_rtt_ms: Range,
    /// RTT across regions, ms.
    pub inter_region_rtt_ms: Range,
    /// Per-packet one-way jitter, ms (uniform 0..jitter).
    pub jitter_ms: f64,
    /// Datagram / UDP loss probability.
    pub loss: f64,
    /// Egress queue depth before datagrams are dropped, ms.
    pub max_queue_ms: f64,
    /// Session open cost, in RTTs.
    pub session_open_rtts: f64,
    /// QUIC idle timeout after which a dead peer's sessions close, s.
    pub idle_timeout_s: f64,
    /// Discovery round trip, ms.
    pub discovery_rtt_ms: f64,
    /// Per-packet overhead bytes (UDP/IP/QUIC).
    pub packet_overhead: u32,
}
impl Default for NetworkSpec {
    fn default() -> Self {
        Self {
            regions: 3,
            intra_region_rtt_ms: Range {
                min: 10.0,
                max: 40.0,
            },
            inter_region_rtt_ms: Range {
                min: 60.0,
                max: 160.0,
            },
            jitter_ms: 2.0,
            loss: 0.0,
            max_queue_ms: 500.0,
            session_open_rtts: 1.5,
            idle_timeout_s: 1.0,
            discovery_rtt_ms: 60.0,
            packet_overhead: 60,
        }
    }
}

/// A scripted event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ScriptedEvent {
    /// Kill a fraction of alive viewers (optionally only relays) abruptly.
    KillFraction {
        /// When.
        at_s: f64,
        /// Fraction in [0,1].
        fraction: f64,
        /// "relay", "leaf" or "any".
        #[serde(default = "any")]
        class: String,
    },
    /// Kill a specific count.
    KillCount {
        /// When.
        at_s: f64,
        /// How many.
        count: usize,
        /// Class filter.
        #[serde(default = "any")]
        class: String,
    },
    /// A burst of new viewers.
    JoinBurst {
        /// When.
        at_s: f64,
        /// Group to instantiate.
        group: ViewerGroup,
    },
    /// Change datagram loss.
    SetLoss {
        /// When.
        at_s: f64,
        /// New loss.
        loss: f64,
    },
    /// Make a fraction of relays poison blocks from now on (adversarial).
    Poison {
        /// When.
        at_s: f64,
        /// Fraction of relays.
        fraction: f64,
    },
}
fn any() -> String {
    "any".into()
}

/// KPI thresholds (Ch8 §8.2).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct KpiThresholds {
    /// Steady-state playback starvation ratio.
    pub psr_max: f64,
    /// Mean startup join latency, s.
    pub sjl_mean_max_s: f64,
    /// p95 startup join latency, s.
    pub sjl_p95_max_s: f64,
    /// Mean overlay depth.
    pub depth_mean_max: f64,
    /// Max overlay depth.
    pub depth_max: u8,
    /// Control-to-data overhead.
    pub cdo_max: f64,
    /// p95 repair time, s (tail; capacity-bound after mass churn).
    pub repair_p95_max_s: f64,
    /// Median repair time, s (the common path: detection + re-attach).
    pub repair_median_max_s: f64,
    /// Every delivered layer must hash-match the source.
    pub require_hash_match: bool,
    /// Fraction of viewers that must reach ACTIVE.
    pub active_fraction_min: f64,
}
impl Default for KpiThresholds {
    fn default() -> Self {
        Self {
            psr_max: 0.001,
            // M1: no late-join backfill yet, so the first frame renders Δ_buffer (3 s)
            // after the first manifest. M3 (MANIFEST_REQUEST + PULL) targets 1.5 s.
            sjl_mean_max_s: 3.6,
            sjl_p95_max_s: 4.5,
            depth_mean_max: 7.0,
            depth_max: 8,
            cdo_max: 0.05,
            repair_p95_max_s: 2.0,
            repair_median_max_s: 1.5,
            require_hash_match: true,
            active_fraction_min: 0.99,
        }
    }
}
