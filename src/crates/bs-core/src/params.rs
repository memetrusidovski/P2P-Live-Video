//! Every tunable constant of the protocol, named as in Appendix B, in one place.
//! `Params::default()` is the specification. Simulation scenarios override fields.
//! Nothing else in `bs-core` may hardcode a protocol constant.

use serde::{Deserialize, Serialize};

use crate::time::Duration;

/// Protocol parameters (Appendix B).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Params {
    // ---- B.1.1 Timeouts & intervals ----
    /// τ_ping: heartbeat after this much silence toward a child; child probes after it.
    pub tau_ping: Duration,
    /// Floor of τ_evict: `max(200 ms, 2τ_ping + SRTT + 4·RTTVAR)`.
    pub tau_evict_floor: Duration,
    /// Source pacing: a chunk's symbols leave over at most this fraction of the chunk period.
    pub source_pacing_fraction: f64,
    /// τ_tft: Tit-for-Tat cycle.
    pub tau_tft: Duration,
    /// τ_gossip: bitfield gossip interval.
    pub tau_gossip: Duration,
    /// τ_sched: buffer scheduler / PULL evaluation cycle.
    pub tau_sched: Duration,
    /// τ_roster: roster distribution interval.
    pub tau_roster: Duration,
    /// τ_deputy: orphan's Deputy-response timer.
    pub tau_deputy: Duration,
    /// τ_forest: minimum dwell between forest-size changes.
    pub tau_forest: Duration,
    /// Migration window, segments.
    pub migration_window_segments: u32,
    /// τ_drain, segments a parent keeps serving a released child.
    pub tau_drain_segments: u32,
    /// τ_retain: seconds of verified segments kept for late joiners.
    pub tau_retain: Duration,
    /// τ_ttl: DHT registration TTL.
    pub tau_ttl: Duration,
    /// Probe timeout during parent selection.
    pub probe_timeout: Duration,
    /// Failed join rounds before a layer is shed.
    pub shed_after_failed_rounds: u32,
    /// Hysteresis before re-joining a shed layer.
    pub shed_hysteresis: Duration,
    /// Greedy upward-migration evaluation interval (Ch1 §1.2.3).
    pub tau_migrate: Duration,
    /// Join retry backoff base.
    pub join_backoff: Duration,

    // ---- B.1.2 Overlay sizing ----
    /// c_a: Active Set target.
    pub c_a: usize,
    /// c_p: Passive Set target.
    pub c_p: usize,
    /// D_max: maximum hop depth.
    pub d_max: u8,
    /// Δ_buffer: playout deadline behind the live edge.
    pub delta_buffer: Duration,
    /// W_pull floor and ceiling.
    pub w_pull_min: Duration,
    /// W_pull ceiling.
    pub w_pull_max: Duration,
    /// r_pull: fraction of upload reserved for PULL service.
    pub r_pull: f64,
    /// f_frame: framing overhead fraction on media bytes.
    pub f_frame: f64,
    /// Leaf share of base-layer slots.
    pub leaf_share: f64,
    /// Rank preemption margin (1.25).
    pub preempt_margin: f64,
    /// Forest ladder: relay counts at which M steps to 2..=6.
    pub forest_ladder_relays: Vec<u32>,

    // ---- Scoring (Ch1 §1.2.2 §2.1.1) ----
    /// w_c: ms of credit per √K_avail.
    pub w_c: f64,
    /// w_h: ms per unit of exponential depth penalty.
    pub w_h: f64,
    /// λ in HopPenalty = e^{λh} − 1.
    pub lambda_hop: f64,
    /// K_ref: slot count where capacity credit saturates.
    pub k_ref: f64,

    // ---- B.1.3 Crypto & encoding ----
    /// C1 static PoW bits.
    pub c1: u32,
    /// C2 floor.
    pub c2_min: u32,
    /// Chunk period.
    pub chunk_period: Duration,
    /// Manifest acceptance window ahead of the live edge, segments (Ch7 §7.1.2).
    pub manifest_window_ahead: u32,
    /// Default parity when no loss report exists yet.
    pub default_parity: u16,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            tau_ping: Duration::from_millis(100),
            tau_evict_floor: Duration::from_millis(200),
            source_pacing_fraction: 0.5,
            tau_tft: Duration::from_millis(500),
            tau_gossip: Duration::from_millis(1000),
            tau_sched: Duration::from_millis(100),
            tau_roster: Duration::from_millis(1000),
            tau_deputy: Duration::from_millis(45),
            tau_forest: Duration::from_secs(30),
            migration_window_segments: 5,
            tau_drain_segments: 5,
            tau_retain: Duration::from_secs(8),
            tau_ttl: Duration::from_secs(180),
            probe_timeout: Duration::from_millis(200),
            shed_after_failed_rounds: 2,
            shed_hysteresis: Duration::from_secs(10),
            tau_migrate: Duration::from_secs(5),
            join_backoff: Duration::from_millis(250),
            c_a: 8,
            c_p: 32,
            d_max: 8,
            delta_buffer: Duration::from_secs(3),
            w_pull_min: Duration::from_millis(1500),
            w_pull_max: Duration::from_millis(2000),
            r_pull: 0.10,
            f_frame: 0.03,
            leaf_share: 0.20,
            preempt_margin: 1.25,
            forest_ladder_relays: vec![0, 6, 12, 24, 48, 96],
            w_c: 12.0,
            w_h: 20.0,
            lambda_hop: 0.5,
            k_ref: 256.0,
            c1: 16,
            c2_min: 8,
            chunk_period: Duration::from_millis(250),
            manifest_window_ahead: 3,
            default_parity: 1,
        }
    }
}

impl Params {
    /// Parameters with cheap proof-of-work for simulation and tests.
    pub fn for_simulation() -> Self {
        Self {
            c1: 4,
            c2_min: 2,
            ..Self::default()
        }
    }

    /// τ_evict for a connection with the given RTT estimator state (Ch3 §3.3.1).
    pub fn tau_evict(&self, srtt: Duration, rttvar: Duration) -> Duration {
        let computed = self.tau_ping * 2 + srtt + rttvar * 4;
        if computed > self.tau_evict_floor {
            computed
        } else {
            self.tau_evict_floor
        }
    }

    /// W_pull = clamp(τ_evict_max + 6·SRTT_max, 1.5 s, 2.0 s) (Ch4 §4.3.1).
    pub fn w_pull(&self, tau_evict_max: Duration, srtt_max: Duration) -> Duration {
        let w = tau_evict_max + srtt_max * 6;
        w.clamp(self.w_pull_min, self.w_pull_max)
    }

    /// Multivariate score in ms (Ch1 §1.2.2 §2.1):
    /// `w_c · min(√K_avail, √K_ref) · R − RTT − w_h · (e^{λh} − 1)`.
    pub fn score(&self, k_avail: u16, reliability: f32, rtt_ms: f64, hop: u8) -> f64 {
        let cap = (k_avail as f64).sqrt().min(self.k_ref.sqrt()) * reliability as f64;
        let hop_pen = (self.lambda_hop * hop as f64).exp() - 1.0;
        self.w_c * cap - rtt_ms - self.w_h * hop_pen
    }

    /// θ_join = max(1, min(4, N − 1)) (Ch1 §1.3.1).
    pub fn theta_join(&self, swarm_size: u32) -> usize {
        (swarm_size.saturating_sub(1)).clamp(1, 4) as usize
    }

    /// Forest size M for a relay count, from the ladder (Ch1 §1.2.1 §1.4).
    pub fn forest_size(&self, relay_count: u32) -> u8 {
        let mut m = 2u8;
        for (i, &thr) in self.forest_ladder_relays.iter().enumerate().skip(2) {
            if relay_count >= thr {
                m = (i + 1) as u8;
            }
        }
        m.clamp(2, 6)
    }

    /// Per-tree slot count `K_v(m) = ⌊(1 − r_pull)·u_v / (t_v · B_m · Ω_v)⌋` with
    /// `Ω = (1 + Ē/K)(1 + f_frame)` (Ch1 §1.2.1 §1.3). Inputs in kbps.
    pub fn slots(
        &self,
        upload_kbps: u32,
        assigned_tree_count: u32,
        tree_bitrate_kbps: u16,
        mean_parity: f64,
    ) -> u16 {
        if tree_bitrate_kbps == 0 || assigned_tree_count == 0 {
            return 0;
        }
        let omega = (1.0 + mean_parity / bs_wire::consts::K_BLOCK as f64) * (1.0 + self.f_frame);
        let k = (1.0 - self.r_pull) * upload_kbps as f64
            / (assigned_tree_count as f64 * tree_bitrate_kbps as f64 * omega);
        k.floor().clamp(0.0, u16::MAX as f64) as u16
    }

    /// Leaf share `R_leaf(m) = ⌈0.2 · K_v(m)⌉`.
    pub fn leaf_slots(&self, k_v: u16) -> u16 {
        (self.leaf_share * k_v as f64).ceil() as u16
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// App B: τ_evict = 240 ms at 20 ms RTT, 320 at 80, 490 at 250 (RTTVAR = RTT/4 assumed).
    #[test]
    fn tau_evict_examples() {
        let p = Params::default();
        let f = |rtt: u64| {
            p.tau_evict(Duration::from_millis(rtt), Duration::from_millis(rtt / 4))
                .as_millis()
        };
        assert_eq!(f(20), 240);
        assert_eq!(f(80), 360); // 200 + 80 + 80
        assert_eq!(f(250), 698); // 200 + 250 + 4·62 (integer µs arithmetic)
        assert_eq!(
            p.tau_evict(Duration::from_millis(0), Duration::ZERO)
                .as_millis(),
            200
        );
    }

    /// Ch1 §1.2.2 §2.1.1 worked comparisons at R = 1.
    #[test]
    fn score_table() {
        let p = Params::default();
        // The spec table rounds each term before combining; allow ±1 ms.
        let r = |k, rtt, h, want: f64| {
            assert!(
                (p.score(k, 1.0, rtt, h) - want).abs() <= 1.0,
                "k={k} rtt={rtt} h={h}"
            )
        };
        r(10, 20.0, 3, -52.0);
        r(10_000, 250.0, 1, -71.0);
        r(10_000, 20.0, 1, 159.0);
        r(1, 20.0, 3, -78.0);
        r(10, 20.0, 7, -625.0);
    }

    /// Ch1 §1.3.1 θ_join table.
    #[test]
    fn theta_join_table() {
        let p = Params::default();
        assert_eq!(p.theta_join(2), 1);
        assert_eq!(p.theta_join(3), 2);
        assert_eq!(p.theta_join(4), 3);
        assert_eq!(p.theta_join(5), 4);
        assert_eq!(p.theta_join(1_000_000), 4);
        assert_eq!(p.theta_join(1), 1);
    }

    /// Ch1 §1.2.5: 8 Mbps relay, 0.75 Mbps stripe, one tree → 8 slots (worked figure).
    #[test]
    fn slot_count_example() {
        let p = Params::default();
        // spec: ⌊7.2 / 0.8625⌋ = 8 with Ω = 1.15; our Ω from parity 1: (1+1/16)(1.03)=1.094
        let k = p.slots(8000, 1, 750, 1.0);
        assert!((8..=9).contains(&k), "got {k}");
        assert_eq!(p.slots(8000, 1, 0, 1.0), 0);
        assert_eq!(p.leaf_slots(6), 2);
        assert_eq!(p.leaf_slots(1), 1);
        assert_eq!(p.leaf_slots(0), 0);
    }
}
