//! The driver contract: what a node consumes and what it produces.

use std::net::SocketAddr;

use bs_media::LayeredChunk;
use bs_wire::{Frame, NodeId, PeerRecord, SegmentSeq, StreamRecord, TreeId};
use serde::{Deserialize, Serialize};

use crate::time::Instant;
use crate::Lifecycle;

/// Network address of a peer (the driver's session key).
pub type PeerAddr = SocketAddr;

/// Which channel a frame travels on. Drivers map this to UDP, a QUIC stream or a
/// QUIC datagram; the simulator uses it for ordering and loss semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Channel {
    /// Plain UDP, no session (pre-session frames; also PING heartbeats).
    Udp,
    /// The session's control stream (reliable, ordered).
    Control,
    /// The reliable, ordered stream of one tree.
    Tree(TreeId),
    /// Unreliable datagram (media symbols).
    Datagram,
}

/// Something the driver tells the node.
#[derive(Debug, Clone)]
pub enum Input {
    /// A frame arrived.
    Frame {
        /// Sender address as observed.
        from: PeerAddr,
        /// Channel it arrived on.
        channel: Channel,
        /// The frame.
        frame: Frame,
    },
    /// A session the node requested (or a peer initiated) is open.
    SessionOpened {
        /// Peer.
        peer: PeerAddr,
        /// Peer identity learned at the handshake.
        node_id: NodeId,
    },
    /// A session closed (by the peer, by timeout, or after `CloseSession`).
    SessionClosed {
        /// Peer.
        peer: PeerAddr,
    },
    /// A local command.
    Cmd(Command),
    /// Time passed; run due timers. Drivers call this when `now >= next_timer()`.
    Tick,
}

/// Local commands from the application or the discovery service.
#[derive(Debug, Clone)]
pub enum Command {
    /// Begin: publisher or viewer, decided by `NodeConfig::role`.
    Start,
    /// Discovery answered (M1: the simulator's oracle; M3: the DHT module).
    Discovered {
        /// Peer records for the wanted trees / membership.
        records: Vec<PeerRecord>,
        /// The publisher's signed Stream Record, if the guardian held one.
        stream_record: Option<StreamRecord>,
    },
    /// Publisher only: the source produced the next chunk.
    PublishChunk(LayeredChunk),
    /// Publisher only: the source ended.
    EndStream,
    /// Leave gracefully.
    Quit,
}

/// Something the node wants the driver to do.
#[derive(Debug, Clone)]
pub enum Output {
    /// Send a frame.
    Send {
        /// Destination.
        to: PeerAddr,
        /// Channel.
        channel: Channel,
        /// Frame.
        frame: Frame,
        /// Do not send before this instant (source pacing). `None` = now.
        not_before: Option<Instant>,
    },
    /// Open a session (QUIC connection) to a peer.
    OpenSession(PeerAddr),
    /// Close a session.
    CloseSession(PeerAddr),
    /// Ask discovery for peers. Answered with `Command::Discovered`.
    Discover {
        /// Trees the node wants relays for; empty = membership query.
        wanted_trees: bs_wire::TreeSet,
        /// Trees the node has failed to find a parent in.
        starved_trees: bs_wire::TreeSet,
    },
    /// Register or refresh this node's record with discovery.
    Register(PeerRecord),
    /// Publisher only: write the signed Stream Record to discovery (once per segment).
    StoreRecord(StreamRecord),
    /// A chunk reached its playout deadline. `layers_complete` is the number of
    /// leading layers that were fully verified; the chunk carries exactly those
    /// layers (an incomplete chunk may carry zero).
    Deliver {
        /// The chunk (possibly partial).
        chunk: LayeredChunk,
        /// Leading complete layers.
        layers_complete: u8,
        /// Layers the node subscribed to.
        layers_subscribed: u8,
    },
    /// A structured event for logs, KPIs and `explain`.
    Event(Event),
}

/// Structured events. Every field is plain data so the simulator can write
/// them as JSON lines.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Event {
    /// Lifecycle transition.
    State {
        /// Previous state.
        from: Lifecycle,
        /// New state.
        to: Lifecycle,
    },
    /// Parent attached in a tree.
    ParentAttached {
        /// Tree.
        tree: u8,
        /// Parent.
        parent: NodeId,
        /// Our depth after attaching.
        depth: u8,
        /// Whether this was a repair.
        repair: bool,
    },
    /// Parent lost (timeout, disconnect, drain).
    ParentLost {
        /// Tree.
        tree: u8,
        /// Parent.
        parent: NodeId,
        /// Why.
        reason: String,
    },
    /// A join round for a tree failed against every candidate.
    JoinRoundFailed {
        /// Tree.
        tree: u8,
        /// Consecutive failed rounds.
        failed_rounds: u32,
        /// Candidates probed.
        candidates: usize,
    },
    /// A layer (and those above) was shed.
    LayerShed {
        /// Highest layer still subscribed.
        top_layer: u8,
    },
    /// A child was admitted.
    ChildAdmitted {
        /// Tree.
        tree: u8,
        /// Child.
        child: NodeId,
        /// Children now.
        children: usize,
        /// Slots.
        k_v: u16,
    },
    /// A join was refused.
    ChildRejected {
        /// Tree.
        tree: u8,
        /// Requester.
        child: NodeId,
        /// Reason name.
        reason: String,
    },
    /// A child left or was removed.
    ChildRemoved {
        /// Tree.
        tree: u8,
        /// Child.
        child: NodeId,
        /// Why.
        reason: String,
    },
    /// Block verified (and forwarded to `forwarded_to` children).
    BlockVerified {
        /// Tree.
        tree: u8,
        /// Segment.
        segment: u32,
        /// Block index.
        block: u16,
        /// Depth at which we hold it.
        depth: u8,
        /// Whether RaptorQ decoding (not the fast path) was needed.
        decoded: bool,
        /// Children it was forwarded to.
        forwarded_to: usize,
    },
    /// Block failed verification.
    BlockRejected {
        /// Tree.
        tree: u8,
        /// Segment.
        segment: u32,
        /// Block index.
        block: u16,
        /// Sender.
        from: NodeId,
    },
    /// Manifest accepted.
    ManifestAccepted {
        /// Segment.
        segment: u32,
        /// Chunk.
        chunk: u8,
        /// Blocks.
        blocks: u16,
    },
    /// Manifest rejected (bad signature, outside window).
    ManifestRejected {
        /// Segment.
        segment: u32,
        /// Chunk.
        chunk: u8,
        /// Why.
        reason: String,
    },
    /// A chunk reached its deadline.
    ChunkPlayed {
        /// Segment.
        segment: u32,
        /// Chunk.
        chunk: u8,
        /// Leading complete layers.
        layers_complete: u8,
        /// Subscribed layers.
        layers_subscribed: u8,
        /// Whether the base layer was complete.
        ok: bool,
        /// Blocks missing (all layers).
        missing_blocks: u16,
    },
    /// First fully played chunk after start (startup join latency marker).
    FirstChunk {
        /// Segment.
        segment: u32,
        /// Chunk.
        chunk: u8,
    },
    /// Heartbeat sent to a child.
    Heartbeat {
        /// Tree (0 = not tree-specific).
        tree: u8,
        /// Peer.
        to: NodeId,
    },
    /// A repair completed.
    RepairDone {
        /// Tree.
        tree: u8,
        /// Microseconds from loss to new parent's first block.
        duration_us: u64,
    },
    /// Free-form diagnostic.
    Note {
        /// Text.
        text: String,
    },
}

/// The live edge and matrix a joiner learns from discovery.
#[derive(Debug, Clone)]
pub struct StreamInfo {
    /// Publisher key.
    pub publisher: bs_wire::PublicKeyBytes,
    /// Swarm size estimate.
    pub swarm_size: u32,
    /// Relay count estimate.
    pub relay_count: u32,
    /// Live edge anchor.
    pub live_edge: SegmentSeq,
    /// Slicing matrix in force.
    pub matrix: bs_media::SlicingMatrix,
}
