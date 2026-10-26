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
    /// A reliable stream for bulk pull responses (`BLOCK_TRANSMISSION`), kept off
    /// the control stream so 16 KB blocks never head-of-line block NEIGHBOR / ACCEPTED.
    Pull,
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
        /// Transient symmetric key both ends derive from the handshake (TLS exporter);
        /// keys the gossip HMAC of Ch3 §3.2.2.
        session_key: [u8; 32],
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
    /// Publisher only: install the STREAM_DESCRIPTOR for the current layers (App D §D.4.20).
    SetDescriptor(bs_wire::frames::StreamDescriptor),
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
    /// The verified STREAM_DESCRIPTOR for the stream (delivered to the player once
    /// per version).
    Descriptor(bs_wire::frames::StreamDescriptor),
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
    /// A `MANIFEST_UPDATE` was made (source) or accepted (peer).
    ForestUpdateAnnounced {
        /// Matrix version.
        version: u8,
        /// Trees in the new layout.
        m: u8,
        /// Layers in the new layout.
        layers: u8,
        /// First segment on the new layout.
        effective: u32,
    },
    /// The pending matrix took effect.
    ForestSwitched {
        /// Matrix version.
        version: u8,
        /// Trees.
        m: u8,
        /// Layers.
        layers: u8,
    },
    /// A relay's rendezvous assignment moved at a resize.
    RelayReassigned {
        /// Old tree.
        from: u8,
        /// New tree.
        to: u8,
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
        /// Backfilled chunk (best effort, excluded from steady-state starvation).
        backfill: bool,
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
    /// A PULL_REQUEST went out.
    PullSent {
        /// Segment.
        segment: u32,
        /// Block.
        block: u16,
        /// Time to deadline, ms.
        urgency_ms: u16,
        /// Rarity broadcast.
        broadcast: bool,
    },
    /// A PULL_REQUEST was answered.
    PullServed {
        /// Segment.
        segment: u32,
        /// Block.
        block: u16,
        /// Requester.
        to: String,
    },
    /// A PULL_REQUEST was refused for lack of PULL budget.
    PullRefused {
        /// Segment.
        segment: u32,
        /// Block.
        block: u16,
    },
    /// A PULL_REQUEST arrived for a block this node does not hold verified (a stale
    /// or optimistic bitfield at the requester); it is answered with silence.
    PullUnheld {
        /// Segment.
        segment: u32,
        /// Block.
        block: u16,
    },
    /// A block was completed through the pull path.
    BlockPulled {
        /// Segment.
        segment: u32,
        /// Block.
        block: u16,
    },
    /// Reachability decided from PONG reflections (Ch2 §2.1.3).
    ReachabilityDetected {
        /// The class decided.
        reach: bs_wire::Reachability,
    },
    /// A PUNCH_REQUEST went to a referrer (App D §D.4.14).
    PunchSent {
        /// The CONE candidate.
        target: NodeId,
    },
    /// A RELAY_PROPOSAL went to a RELAY_CAPABLE peer (Ch6 §6.3.3).
    BridgeProposed {
        /// Tree.
        tree: u8,
    },
    /// A relay bound a bridge for us.
    BridgeBound {
        /// Tree.
        tree: u8,
        /// The relay.
        relay: NodeId,
    },
    /// This node bound a bridge for a NAT-blocked peer (relay side).
    BridgeServed {
        /// Tree.
        tree: u8,
        /// The bridged peer.
        client: NodeId,
    },
    /// A bound bridge whose child never joined was released (relay side).
    BridgeExpired {
        /// Tree.
        tree: u8,
        /// The peer that never joined.
        client: NodeId,
    },
    /// The source bound an ingress relay (Ch6 §6.3.3).
    IngressBound {
        /// The relay.
        relay: NodeId,
    },
    /// This relay became hop 0 for a source behind a NAT.
    IngressServing {
        /// The source.
        source: NodeId,
    },
    /// A relay declined, or did not answer.
    BridgeRejected {
        /// Tree.
        tree: u8,
    },
    /// A DHT lookup finished (Ch2 §2.1).
    DhtLookup {
        /// `self`, `guardians` or `value`.
        purpose: &'static str,
        /// Longest path, in hops.
        hops: u8,
        /// FIND requests sent.
        queried: u16,
        /// Contacts that answered.
        found: u16,
        /// Wall time, ms.
        ms: u32,
    },
    /// This node answered a DHT RPC as a server (`find_node`, `find_value`,
    /// `register`, `get_peers`, `store`).
    DhtServed {
        /// RPC kind.
        rpc: &'static str,
    },
    /// We issued a PROOF_OF_UPLOAD receipt (Ch5 §5.2).
    ReceiptIssued {
        /// Tree (0 = PULL receipt).
        tree: u8,
        /// Segment.
        segment: u32,
        /// Blocks attested.
        blocks: u16,
        /// Loss rate byte.
        loss: u8,
    },
    /// A receipt from a child/peer was accepted into the ledger.
    ReceiptAccepted {
        /// Tree.
        tree: u8,
        /// Segment.
        segment: u32,
        /// Capped bytes credited.
        bytes: u64,
    },
    /// A receipt was rejected (`bad_signature`, `unknown_key`, `wrong_uploader`,
    /// `wrong_stream`, `stale`).
    ReceiptRejected {
        /// Why.
        reason: &'static str,
    },
    /// A child missed a receipt deadline (Ch5 §5.2.1).
    ReceiptMissed {
        /// Tree.
        tree: u8,
        /// Child.
        child: NodeId,
        /// Consecutive misses.
        consecutive: u8,
    },
    /// Our choke state toward a peer changed (Ch5 §5.1.2).
    ChokeChanged {
        /// Peer.
        peer: NodeId,
        /// `choke`, `unchoke`, `optimistic`.
        state: &'static str,
    },
    /// Per-link parity changed after a loss report (Ch4 §4.2.2).
    ParityChanged {
        /// Child.
        child: NodeId,
        /// Symbols per block.
        parity: u16,
        /// Reported loss byte.
        loss: u8,
    },
    /// A PULL_REQUEST from a choked peer was refused.
    PullChoked,
    /// We sent a RANK_PROOF ahead of a join (Ch5 §5.2.2).
    RankProofSent {
        /// Receipts committed.
        receipts: u32,
        /// Our own rank estimate.
        rank: f64,
    },
    /// A peer's RANK_PROOF verified.
    RankProofVerified {
        /// Prover.
        peer: NodeId,
        /// Receipts committed.
        receipts: u32,
        /// Rank established.
        rank: f64,
    },
    /// A RANK_PROOF failed a check.
    RankProofRejected {
        /// Which check.
        reason: &'static str,
    },
    /// We signed and gossiped an accusation (Ch5 §5.3.3).
    AccusationMade {
        /// The accused.
        suspect: NodeId,
        /// `symmetric_tree_pair` or `equivocation`.
        evidence: &'static str,
    },
    /// A received accusation was re-verified and counted.
    AccusationCounted {
        /// The accused.
        suspect: NodeId,
        /// Who accused.
        accuser: NodeId,
    },
    /// A received accusation was not counted.
    AccusationDropped {
        /// Why.
        reason: &'static str,
    },
    /// Three ranked accusers from three prefixes: the suspect is evicted locally.
    ConsensusEviction {
        /// The evicted peer.
        suspect: NodeId,
        /// Evictions of this peer so far (the ban doubles each time).
        strikes: u32,
    },
    /// A rank proof's sampled signers clustered in one prefix (Ch5 §5.3.2).
    SubnetPenalty {
        /// Factor applied to the rank.
        factor: f64,
    },
    /// Rank admission preempted a child (Ch1 §1.2.2 case 2).
    RankPreempted {
        /// Tree.
        tree: u8,
        /// Victim.
        child: NodeId,
        /// Joiner's rank.
        r_new: f64,
        /// Victim's rank.
        r_min: f64,
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
