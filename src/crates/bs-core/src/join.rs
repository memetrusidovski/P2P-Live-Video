//! Per-tree parent selection (Ch1 §1.2.2 §2.2) and its retry/shed bookkeeping.

use bs_wire::{NodeId, PeerRecord, TreeState as WireTreeState};

use crate::io::PeerAddr;
use crate::time::Instant;

/// A probed candidate.
#[derive(Debug, Clone)]
pub struct Candidate {
    /// Record it came from.
    pub record: PeerRecord,
    /// When the PROBE was sent.
    pub probed_at: Instant,
    /// Response, once it arrives.
    pub response: Option<ProbeResult>,
}

/// What a PROBE_RESPONSE told us.
#[derive(Debug, Clone)]
pub struct ProbeResult {
    /// Free slots.
    pub k_avail: u16,
    /// Reliability in [0, 1].
    pub reliability: f32,
    /// Hop depth.
    pub hop: u8,
    /// Per-tree state.
    pub tree_state: WireTreeState,
    /// Measured RTT, ms.
    pub rtt_ms: f64,
    /// Score, once computed.
    pub score: f64,
}

/// The join state machine for one tree.
#[derive(Debug, Clone)]
pub enum JoinState {
    /// Nothing in flight.
    Idle,
    /// Waiting for discovery to answer.
    Discovering {
        /// When we asked.
        since: Instant,
    },
    /// PROBEs out, collecting responses until `deadline`.
    Probing {
        /// Candidates.
        candidates: Vec<Candidate>,
        /// Round deadline.
        deadline: Instant,
    },
    /// Trying scored candidates in order.
    Requesting {
        /// Remaining ordered candidates (best first): record, score, hop, rtt_ms.
        queue: Vec<(PeerRecord, f64, u8, f64)>,
        /// The one we are asking now.
        current: PeerAddr,
        /// Its id.
        current_id: NodeId,
        /// Its hop.
        current_hop: u8,
        /// Its probe RTT, ms.
        current_rtt_ms: f64,
        /// Whether we are waiting for the session to open before sending NEIGHBOR.
        awaiting_session: bool,
        /// Deadline for ACCEPTED / rejection.
        deadline: Instant,
        /// Candidates counted as saturated this round.
        saturated: usize,
    },
    /// Every candidate failed; retry at `until`.
    Backoff {
        /// Retry time.
        until: Instant,
    },
}

impl JoinState {
    /// Whether a join is in progress.
    pub fn is_active(&self) -> bool {
        !matches!(self, JoinState::Idle | JoinState::Backoff { .. })
    }
}

/// Per-tree failure counters that drive the shed rule (Ch1 §1.1.5 §5.3).
#[derive(Debug, Clone, Default)]
pub struct JoinStats {
    /// Consecutive rounds in which every candidate was saturated or absent.
    pub failed_rounds: u32,
    /// Rounds attempted in total.
    pub rounds: u32,
    /// Whether the last round saw a WARMING candidate (round does not count).
    pub saw_warming: bool,
}
