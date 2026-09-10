//! Per-tree overlay state: the parent link, child links, depth and slots
//! (Ch1 §1.2).

use std::collections::HashMap;

use bs_wire::{NodeClass, NodeId, SegmentSeq, TreeId, TreeSet};

use crate::io::PeerAddr;
use crate::join::JoinState;
use crate::time::{Duration, Instant};

/// Our parent in one tree.
#[derive(Debug, Clone)]
pub struct ParentLink {
    /// Address.
    pub addr: PeerAddr,
    /// Identity.
    pub node_id: NodeId,
    /// Parent's advertised depth (from ACCEPTED, refreshed by BLOCK_PROOF).
    pub parent_depth: u8,
    /// Last packet received from the parent on any channel.
    pub last_rx: Instant,
    /// Whether we already PINGed during this silence episode.
    pub pinged: bool,
    /// Smoothed RTT estimate.
    pub srtt: Duration,
    /// RTT variance estimate.
    pub rttvar: Duration,
    /// When we attached.
    pub attached_at: Instant,
    /// First block received from this parent (repair timing).
    pub first_block_at: Option<Instant>,
    /// Slot granted but pending a drain (ACCEPTED.PENDING).
    pub pending: bool,
    /// Blocks from this parent that failed verification (Ch4 §4.1.2: flag the parent).
    pub rejected_blocks: u32,
    /// Attached as the replacement in a repair or drain; its first block ends the repair.
    pub attached_as_repair: bool,
}

/// A child we serve in one tree.
#[derive(Debug, Clone)]
pub struct ChildLink {
    /// Address.
    pub addr: PeerAddr,
    /// Identity.
    pub node_id: NodeId,
    /// Class declared in NEIGHBOR.
    pub node_class: NodeClass,
    /// Assigned trees declared in NEIGHBOR.
    pub assigned_trees: TreeSet,
    /// Last packet we sent it on any channel.
    pub last_tx: Instant,
    /// Last packet we received from it.
    pub last_rx: Instant,
    /// When admitted.
    pub admitted_at: Instant,
    /// Parity symbols per block on this link.
    pub parity: u16,
    /// Drain in progress: keep serving until this segment.
    pub draining_until: Option<SegmentSeq>,
    /// Admitted with `ACCEPTED.PENDING`: not served until a draining child leaves
    /// (sequential handover, Ch1 §1.2.2 "Handover budget").
    pub pending: bool,
}

/// State of one tree at this node.
#[derive(Debug)]
pub struct TreeState {
    /// Tree id.
    pub id: TreeId,
    /// Parent, if attached.
    pub parent: Option<ParentLink>,
    /// The previous parent during a warm handover: it keeps delivering until the
    /// new parent's first block arrives, then receives `DISCONNECT_CHOKE`
    /// (Ch1 §1.2.2 The Drain Path, step 3).
    pub old_parent: Option<ParentLink>,
    /// Children, by address.
    pub children: HashMap<PeerAddr, ChildLink>,
    /// Our hop depth from the source (0 = source; `u8::MAX` = unknown/unparented).
    pub depth: u8,
    /// Slot count `K_v(m)`.
    pub k_v: u16,
    /// Whether we relay this tree (assigned) — leaves never do.
    pub assigned: bool,
    /// Whether we subscribe to this tree (render its layer).
    pub subscribed: bool,
    /// Segments verified in this tree (warm-up: 0 → K_avail advertised as 0).
    pub verified_segments: u32,
    /// Highest segment verified in this tree.
    pub live_edge: SegmentSeq,
    /// Per-tree CHURN_REPAIR sub-state.
    pub repairing: bool,
    /// When the parent was lost (for repair timing).
    pub lost_at: Option<Instant>,
    /// Parent selection state.
    pub join: JoinState,
    /// Segments in which at least one block arrived (for live edge accounting).
    pub last_segment_seen: SegmentSeq,
}

impl TreeState {
    /// New, detached tree state.
    pub fn new(id: TreeId) -> Self {
        Self {
            id,
            parent: None,
            old_parent: None,
            children: HashMap::new(),
            depth: u8::MAX,
            k_v: 0,
            assigned: false,
            subscribed: false,
            verified_segments: 0,
            live_edge: SegmentSeq::NONE,
            repairing: false,
            lost_at: None,
            join: JoinState::Idle,
            last_segment_seen: SegmentSeq::NONE,
        }
    }

    /// Whether this node should hold a parent here (subscribed or assigned).
    pub fn wants_parent(&self) -> bool {
        self.subscribed || self.assigned
    }

    /// Free slots advertised to joiners: 0 while warming (Ch1 §1.2.2 warm-up gating),
    /// 0 if not assigned, else `K_v − children` (draining children still count).
    pub fn k_avail(&self, is_source: bool) -> u16 {
        if !self.assigned {
            return 0;
        }
        if !is_source && (self.verified_segments == 0 || self.parent.is_none()) {
            return 0;
        }
        self.k_v.saturating_sub(self.children.len() as u16)
    }

    /// `TreeState` for PROBE_RESPONSE.
    pub fn probe_state(&self, is_source: bool) -> bs_wire::TreeState {
        if is_source {
            return bs_wire::TreeState::Serving;
        }
        match (&self.parent, self.verified_segments) {
            (None, _) => bs_wire::TreeState::Unparented,
            (Some(_), 0) => bs_wire::TreeState::Warming,
            _ => bs_wire::TreeState::Serving,
        }
    }

    /// Whether the depth rule allows another child under us (`h + 1 <= D_max − ?`):
    /// a candidate advertising `h_p >= D_max` is not a legal parent.
    pub fn depth_allows_children(&self, d_max: u8) -> bool {
        self.depth < d_max
    }

    /// Current hop depth for advertisement (source = 0).
    pub fn advertised_depth(&self) -> u8 {
        self.depth
    }

    /// Detach the parent, returning it.
    pub fn detach_parent(&mut self, now: Instant) -> Option<ParentLink> {
        let p = self.parent.take();
        if p.is_some() {
            self.lost_at = Some(now);
            self.depth = u8::MAX;
        }
        p
    }
}
