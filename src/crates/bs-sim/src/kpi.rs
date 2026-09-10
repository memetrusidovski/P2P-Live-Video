//! KPI collection (Ch8 §8.2) and the end-to-end data-correctness check.

use std::collections::HashMap;

use bs_core::{Event, Instant};
use bs_media::LayeredChunk;
use bs_wire::{FrameType, Hash, NodeClass};
use serde::Serialize;

use crate::invariants::Invariants;
use crate::report::EventLog;
use crate::scenario::Scenario;
use crate::sim::SimNode;

/// Per-node statistics.
#[derive(Debug, Clone, Default, Serialize)]
pub struct NodeStats {
    /// Label.
    pub label: String,
    /// Class.
    pub class: String,
    /// Publisher?
    pub is_publisher: bool,
    /// Start.
    pub started_us: Option<u64>,
    /// First fully-played base-layer chunk.
    pub first_chunk_us: Option<u64>,
    /// Reached ACTIVE.
    pub active_us: Option<u64>,
    /// Died.
    pub died_us: Option<u64>,
    /// Chunks played (all).
    pub played: u64,
    /// Chunks played with the base layer complete.
    pub played_ok: u64,
    /// Chunks played after the first ok chunk.
    pub steady_played: u64,
    /// ... of which starved.
    pub steady_starved: u64,
    /// Chunks where every subscribed layer was complete.
    pub played_full: u64,
    /// Layer hash mismatches (data corruption).
    pub hash_mismatches: u64,
    /// Layer-chunks hash-checked.
    pub hash_checked: u64,
    /// Parent losses.
    pub parent_lost: u64,
    /// Repairs completed.
    pub repairs: u64,
    /// Layers shed.
    pub sheds: u64,
    /// Failed join rounds.
    pub failed_join_rounds: u64,
    /// Blocks rejected (poison).
    pub blocks_rejected: u64,
    /// Final depth per tree (255 = none).
    pub depth: Vec<u8>,
    /// Final children per tree.
    pub children: Vec<usize>,
}

/// Aggregate KPIs.
pub struct Kpis {
    /// Chunks the source emitted.
    pub chunks_published: u64,
    source_hashes: HashMap<(u32, u8, u8), Hash>,
    /// Per node.
    pub nodes: Vec<NodeStats>,
    /// Bytes by frame type: (count, bytes).
    pub bytes: HashMap<FrameType, (u64, u64)>,
    /// Repair durations, µs.
    pub repairs_us: Vec<u64>,
    /// Depth snapshots: (t_us, mean, max, active viewers, alive viewers).
    pub depth_series: Vec<(u64, f64, u8, usize, usize)>,
    /// Session-bound sends dropped for lack of a session.
    pub no_session_drops: u64,
}

impl Kpis {
    /// New.
    pub fn new(_s: &Scenario) -> Self {
        Self {
            chunks_published: 0,
            source_hashes: HashMap::new(),
            nodes: Vec::new(),
            bytes: HashMap::new(),
            repairs_us: Vec::new(),
            depth_series: Vec::new(),
            no_session_drops: 0,
        }
    }
    fn stats(&mut self, i: usize) -> &mut NodeStats {
        if self.nodes.len() <= i {
            self.nodes.resize(i + 1, NodeStats::default());
        }
        &mut self.nodes[i]
    }
    /// Node started.
    pub fn node_started(&mut self, i: usize, now: Instant, class: NodeClass, is_pub: bool) {
        let s = self.stats(i);
        s.started_us = Some(now.as_micros());
        s.class = format!("{class:?}");
        s.is_publisher = is_pub;
    }
    /// Node died.
    pub fn node_died(&mut self, i: usize, now: Instant) {
        self.stats(i).died_us = Some(now.as_micros());
    }
    /// Node terminated itself.
    pub fn node_terminated(&mut self, i: usize, now: Instant) {
        let s = self.stats(i);
        if s.died_us.is_none() {
            s.died_us = Some(now.as_micros());
        }
    }
    /// Source emitted a chunk: remember per-layer hashes.
    pub fn publish_chunk(&mut self, c: &LayeredChunk) {
        self.chunks_published += 1;
        for (l, bytes) in c.layers.iter().enumerate() {
            self.source_hashes.insert(
                (c.segment.0, c.chunk_index, l as u8),
                bs_crypto::blake3_hash(bytes),
            );
        }
    }
    /// Bytes sent.
    pub fn bytes(&mut self, ty: FrameType, size: usize) {
        let e = self.bytes.entry(ty).or_insert((0, 0));
        e.0 += 1;
        e.1 += size as u64;
    }
    /// A chunk was delivered to a viewer's application: check every delivered layer.
    pub fn delivered(
        &mut self,
        i: usize,
        now: Instant,
        c: &LayeredChunk,
        layers_complete: u8,
        layers_subscribed: u8,
        inv: &mut Invariants,
        log: &mut EventLog,
    ) {
        if layers_complete >= layers_subscribed.max(1) {
            self.stats(i).played_full += 1;
        }
        for (l, bytes) in c.layers.iter().enumerate().take(layers_complete as usize) {
            let key = (c.segment.0, c.chunk_index, l as u8);
            let s = self.stats(i);
            s.hash_checked += 1;
            match self.source_hashes.get(&key) {
                Some(h) if *h == bs_crypto::blake3_hash(bytes) => {}
                Some(_) => {
                    self.stats(i).hash_mismatches += 1;
                    inv.violation(
                        now,
                        i,
                        "data_integrity",
                        format!(
                            "segment {} chunk {} layer {} bytes differ from the source",
                            key.0, key.1, key.2
                        ),
                        true,
                        log,
                    );
                }
                None => {
                    inv.violation(
                        now,
                        i,
                        "data_integrity",
                        format!(
                            "delivered segment {} chunk {} layer {} that the source never emitted",
                            key.0, key.1, key.2
                        ),
                        true,
                        log,
                    );
                }
            }
        }
    }
    /// A node event.
    pub fn event(&mut self, i: usize, now: Instant, e: &Event) {
        let t = now.as_micros();
        let s = self.stats(i);
        match e {
            Event::State { to, .. } => {
                if *to == bs_core::Lifecycle::Active && s.active_us.is_none() {
                    s.active_us = Some(t);
                }
            }
            Event::ChunkPlayed { ok, .. } => {
                s.played += 1;
                if *ok {
                    s.played_ok += 1;
                }
                if s.first_chunk_us.is_some() {
                    s.steady_played += 1;
                    if !ok {
                        s.steady_starved += 1;
                    }
                }
            }
            Event::FirstChunk { .. } => {
                s.first_chunk_us = Some(t);
            }
            Event::ParentLost { .. } => s.parent_lost += 1,
            Event::RepairDone { duration_us, .. } => {
                s.repairs += 1;
                self.repairs_us.push(*duration_us);
            }
            Event::LayerShed { .. } => s.sheds += 1,
            Event::JoinRoundFailed { .. } => s.failed_join_rounds += 1,
            Event::BlockRejected { .. } => s.blocks_rejected += 1,
            _ => {}
        }
    }
    /// Periodic snapshot of depth and membership.
    pub fn snapshot(&mut self, now: Instant, alive: &[&SimNode]) {
        let mut depths = Vec::new();
        let mut active = 0usize;
        let mut viewers = 0usize;
        for n in alive {
            if n.node.is_source() {
                continue;
            }
            viewers += 1;
            if n.node.state() == bs_core::Lifecycle::Active {
                active += 1;
            }
            for t in n.node.trees() {
                if t.subscribed && t.parent.is_some() && t.depth != u8::MAX {
                    depths.push(t.depth);
                }
            }
        }
        let mean = if depths.is_empty() {
            0.0
        } else {
            depths.iter().map(|d| *d as f64).sum::<f64>() / depths.len() as f64
        };
        let max = depths.iter().copied().max().unwrap_or(0);
        self.depth_series
            .push((now.as_micros(), mean, max, active, viewers));
    }
    /// Record final per-node topology fields.
    pub fn finalize_nodes(&mut self, nodes: &[SimNode]) {
        for (i, n) in nodes.iter().enumerate() {
            let s = self.stats(i);
            s.label = n.label.clone();
            s.depth = n.node.trees().iter().map(|t| t.depth).collect();
            s.children = n.node.trees().iter().map(|t| t.children.len()).collect();
        }
    }

    /// Steady-state PSR over viewers.
    pub fn psr(&self) -> (f64, u64, u64) {
        let (mut st, mut pl) = (0u64, 0u64);
        for s in self.nodes.iter().filter(|s| !s.is_publisher) {
            st += s.steady_starved;
            pl += s.steady_played;
        }
        (if pl == 0 { 1.0 } else { st as f64 / pl as f64 }, st, pl)
    }
    /// Startup join latencies, seconds, for viewers that got a first chunk.
    pub fn sjl(&self) -> Vec<f64> {
        let mut v: Vec<f64> = self
            .nodes
            .iter()
            .filter(|s| !s.is_publisher)
            .filter_map(|s| Some((s.first_chunk_us? as f64 - s.started_us? as f64) / 1e6))
            .collect();
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        v
    }
    /// Control-to-data overhead.
    pub fn cdo(&self) -> (f64, u64, u64) {
        let mut media = 0u64;
        let mut control = 0u64;
        for (ty, (_, b)) in &self.bytes {
            if matches!(
                ty,
                FrameType::RAPTORQ_SYMBOL | FrameType::BLOCK_TRANSMISSION
            ) {
                media += b;
            } else {
                control += b;
            }
        }
        let total = media + control;
        (
            if total == 0 {
                0.0
            } else {
                control as f64 / total as f64
            },
            control,
            media,
        )
    }
    /// Fraction of started, alive viewers that are ACTIVE at the end.
    pub fn active_fraction(&self, nodes: &[SimNode]) -> f64 {
        let mut n = 0;
        let mut a = 0;
        for (i, node) in nodes.iter().enumerate().skip(1) {
            if !node.alive || self.nodes.get(i).and_then(|s| s.started_us).is_none() {
                continue;
            }
            n += 1;
            if node.node.state() == bs_core::Lifecycle::Active {
                a += 1;
            }
        }
        if n == 0 {
            1.0
        } else {
            a as f64 / n as f64
        }
    }
}

/// Percentile of a sorted slice.
pub fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((sorted.len() as f64 - 1.0) * p).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}
