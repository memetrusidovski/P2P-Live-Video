//! Post-handshake control frames on the QUIC stream: NEIGHBOR, ACCEPTED,
//! DISCONNECT, DRAIN_NOTICE, STREAM_END, CHOKE_STATE, MANIFEST_REQUEST, PULL_REQUEST.

use bytes::{Buf, BufMut};

use crate::codec::{
    get_u16, get_u32, get_u64, get_u8, put_reserved, skip_reserved, Decode, Encode,
};
use crate::error::{Result, WireError};
use crate::types::{
    BlockIndex, DisconnectReason, NodeClass, NodeId, SegmentSeq, SignatureBytes, StreamId, TreeId,
    TreeSet,
};

/// `NEIGHBOR.Priority`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Priority {
    /// Must be accepted if at all possible (repair, `RELAY_JOIN_REQUEST`).
    High = 0x01,
    /// Ordinary request.
    Low = 0x02,
}
impl Priority {
    fn from_code(c: u8) -> Result<Self> {
        match c {
            0x01 => Ok(Self::High),
            0x02 => Ok(Self::Low),
            v => Err(WireError::InvalidField {
                field: "Priority",
                value: v as u64,
            }),
        }
    }
}

/// NEIGHBOR (0x05): `[Sender NodeID 32][Priority 1][TreeID 1][NodeClass 1][AssignedTrees 1]`.
/// `TreeID = 0` requests membership; `m > 0` requests a parent slot in `T_m`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Neighbor {
    /// Requester.
    pub sender: NodeId,
    /// Request priority.
    pub priority: Priority,
    /// Tree requested, or `NONE` for membership.
    pub tree_id: TreeId,
    /// Requester's class.
    pub node_class: NodeClass,
    /// Requester's assigned trees.
    pub assigned_trees: TreeSet,
}
impl Neighbor {
    /// Encoded length.
    pub const LEN: usize = 36;
}
impl Encode for Neighbor {
    fn encoded_len(&self) -> usize {
        Self::LEN
    }
    fn encode<B: BufMut>(&self, buf: &mut B) {
        self.sender.encode(buf);
        buf.put_u8(self.priority as u8);
        buf.put_u8(self.tree_id.0);
        buf.put_u8(self.node_class as u8);
        buf.put_u8(self.assigned_trees.0);
    }
}
impl Decode for Neighbor {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        Ok(Self {
            sender: NodeId::decode(buf)?,
            priority: Priority::from_code(get_u8(buf, "Neighbor.priority")?)?,
            tree_id: TreeId(get_u8(buf, "Neighbor.tree_id")?),
            node_class: NodeClass::from_code(get_u8(buf, "Neighbor.class")?)?,
            assigned_trees: TreeSet(get_u8(buf, "Neighbor.assigned_trees")?),
        })
    }
}

/// What an ACCEPTED answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum AcceptedType {
    /// Answers a JOIN.
    Join = 0x03,
    /// Answers a NEIGHBOR.
    Neighbor = 0x05,
}
impl AcceptedType {
    fn from_code(c: u8) -> Result<Self> {
        match c {
            0x03 => Ok(Self::Join),
            0x05 => Ok(Self::Neighbor),
            v => Err(WireError::InvalidField {
                field: "AcceptedType",
                value: v as u64,
            }),
        }
    }
}

/// ACCEPTED (0x08): `[Sender NodeID 32][AcceptedType 1][TreeID 1][HopDepth 1][AcceptFlags 1]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Accepted {
    /// Acceptor.
    pub sender: NodeId,
    /// Which request this answers.
    pub accepted_type: AcceptedType,
    /// Tree granted (0 for membership).
    pub tree_id: TreeId,
    /// Acceptor's hop depth in `tree_id`; joiner sets its own to `hop_depth + 1`.
    pub hop_depth: u8,
    /// Bit 0 `PENDING`: slot granted but still held by a draining child.
    pub pending: bool,
}
impl Accepted {
    /// Encoded length.
    pub const LEN: usize = 36;
}
impl Encode for Accepted {
    fn encoded_len(&self) -> usize {
        Self::LEN
    }
    fn encode<B: BufMut>(&self, buf: &mut B) {
        self.sender.encode(buf);
        buf.put_u8(self.accepted_type as u8);
        buf.put_u8(self.tree_id.0);
        buf.put_u8(self.hop_depth);
        buf.put_u8(self.pending as u8);
    }
}
impl Decode for Accepted {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        Ok(Self {
            sender: NodeId::decode(buf)?,
            accepted_type: AcceptedType::from_code(get_u8(buf, "Accepted.type")?)?,
            tree_id: TreeId(get_u8(buf, "Accepted.tree_id")?),
            hop_depth: get_u8(buf, "Accepted.hop_depth")?,
            pending: get_u8(buf, "Accepted.flags")? & 1 != 0,
        })
    }
}

/// DISCONNECT (0x07): `[Sender NodeID 32][Reason 1][TreeID 1][Reserved 2]` (App D §D.4.3b).
///
/// `tree_id = 0` ends the whole connection; `m > 0` ends only the tree-`m`
/// relationship, or — with a `Rejected*` reason — declines a `NEIGHBOR(m)` that
/// never became one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Disconnect {
    /// Sender.
    pub sender: NodeId,
    /// Reason.
    pub reason: DisconnectReason,
    /// Scope: `NONE` = whole connection, else one tree.
    pub tree_id: TreeId,
}
impl Disconnect {
    /// Encoded length.
    pub const LEN: usize = 36;
}
impl Encode for Disconnect {
    fn encoded_len(&self) -> usize {
        Self::LEN
    }
    fn encode<B: BufMut>(&self, buf: &mut B) {
        self.sender.encode(buf);
        buf.put_u8(self.reason as u8);
        buf.put_u8(self.tree_id.0);
        put_reserved(buf, 2);
    }
}
impl Decode for Disconnect {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        let sender = NodeId::decode(buf)?;
        let reason = DisconnectReason::from_code(get_u8(buf, "Disconnect.reason")?)?;
        let tree_id = TreeId(get_u8(buf, "Disconnect.tree_id")?);
        skip_reserved(buf, 2, "Disconnect.reserved")?;
        Ok(Self {
            sender,
            reason,
            tree_id,
        })
    }
}

/// `DRAIN_NOTICE.Scope`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum DrainScope {
    /// Only the addressed child.
    ThisChild = 0x00,
    /// Every child in the tree (triggers Deputy election).
    AllChildren = 0x01,
}

/// DRAIN_NOTICE (0x1E): `[TreeID 1][Reason 1][Scope 1][Reserved 1][DeadlineSegmentSeq 4]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrainNotice {
    /// Tree being drained.
    pub tree_id: TreeId,
    /// Why (drawn from the DISCONNECT code space).
    pub reason: DisconnectReason,
    /// Who is affected.
    pub scope: DrainScope,
    /// Parent keeps serving until this segment.
    pub deadline: SegmentSeq,
}
impl DrainNotice {
    /// Encoded length.
    pub const LEN: usize = 8;
}
impl Encode for DrainNotice {
    fn encoded_len(&self) -> usize {
        Self::LEN
    }
    fn encode<B: BufMut>(&self, buf: &mut B) {
        buf.put_u8(self.tree_id.0);
        buf.put_u8(self.reason as u8);
        buf.put_u8(self.scope as u8);
        put_reserved(buf, 1);
        buf.put_u32(self.deadline.0);
    }
}
impl Decode for DrainNotice {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        let tree_id = TreeId(get_u8(buf, "DrainNotice.tree_id")?);
        let reason = DisconnectReason::from_code(get_u8(buf, "DrainNotice.reason")?)?;
        let scope = match get_u8(buf, "DrainNotice.scope")? {
            0x00 => DrainScope::ThisChild,
            0x01 => DrainScope::AllChildren,
            v => {
                return Err(WireError::InvalidField {
                    field: "DrainNotice.scope",
                    value: v as u64,
                })
            }
        };
        skip_reserved(buf, 1, "DrainNotice.reserved")?;
        let deadline = SegmentSeq(get_u32(buf, "DrainNotice.deadline")?);
        Ok(Self {
            tree_id,
            reason,
            scope,
            deadline,
        })
    }
}

/// STREAM_END (0x04): `[StreamID 32][FinalSegmentSeq 4][Timestamp 8][Signature 64]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamEnd {
    /// Stream.
    pub stream_id: StreamId,
    /// Last segment the source emitted.
    pub final_segment: SegmentSeq,
    /// Timestamp, µs.
    pub timestamp_us: u64,
    /// Source signature over `StreamID ‖ FinalSeqNum ‖ Timestamp`.
    pub signature: SignatureBytes,
}
impl StreamEnd {
    /// Encoded length.
    pub const LEN: usize = 32 + 4 + 8 + 64;
    /// Bytes the signature covers.
    pub fn signable_bytes(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(44);
        self.stream_id.encode(&mut v);
        v.extend_from_slice(&self.final_segment.0.to_be_bytes());
        v.extend_from_slice(&self.timestamp_us.to_be_bytes());
        v
    }
}
impl Encode for StreamEnd {
    fn encoded_len(&self) -> usize {
        Self::LEN
    }
    fn encode<B: BufMut>(&self, buf: &mut B) {
        self.stream_id.encode(buf);
        buf.put_u32(self.final_segment.0);
        buf.put_u64(self.timestamp_us);
        self.signature.encode(buf);
    }
}
impl Decode for StreamEnd {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        Ok(Self {
            stream_id: StreamId::decode(buf)?,
            final_segment: SegmentSeq(get_u32(buf, "StreamEnd.final_segment")?),
            timestamp_us: get_u64(buf, "StreamEnd.timestamp")?,
            signature: SignatureBytes::decode(buf)?,
        })
    }
}

/// CHOKE_STATE (0x21): `[State 1][Reserved 3]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ChokeState {
    /// Choked.
    Choke = 0,
    /// Unchoked by reciprocity.
    Unchoke = 1,
    /// Unchoked optimistically.
    UnchokeOptimistic = 2,
}
impl Encode for ChokeState {
    fn encoded_len(&self) -> usize {
        4
    }
    fn encode<B: BufMut>(&self, buf: &mut B) {
        buf.put_u8(*self as u8);
        put_reserved(buf, 3);
    }
}
impl Decode for ChokeState {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        let s = match get_u8(buf, "ChokeState.state")? {
            0 => Self::Choke,
            1 => Self::Unchoke,
            2 => Self::UnchokeOptimistic,
            v => {
                return Err(WireError::InvalidField {
                    field: "ChokeState",
                    value: v as u64,
                })
            }
        };
        skip_reserved(buf, 3, "ChokeState.reserved")?;
        Ok(s)
    }
}

/// MANIFEST_REQUEST (0x1D): `[SegmentSeq 4][ChunkIndex 1][Reserved 3]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestRequest {
    /// Segment wanted (ignored for the pending update).
    pub segment: SegmentSeq,
    /// Chunk, or a selector.
    pub selector: ManifestSelector,
}
/// `MANIFEST_REQUEST.ChunkIndex` semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ManifestSelector {
    /// One chunk of the segment.
    Chunk(u8),
    /// Every chunk of the segment (`0xFF`).
    AllChunks,
    /// The pending MANIFEST_UPDATE (`0xFE`).
    PendingUpdate,
    /// The STREAM_DESCRIPTOR in force and any pending one (`0xFD`).
    Descriptor,
}
impl ManifestRequest {
    /// Encoded length.
    pub const LEN: usize = 8;
}
impl Encode for ManifestRequest {
    fn encoded_len(&self) -> usize {
        Self::LEN
    }
    fn encode<B: BufMut>(&self, buf: &mut B) {
        buf.put_u32(self.segment.0);
        buf.put_u8(match self.selector {
            ManifestSelector::Chunk(c) => c,
            ManifestSelector::AllChunks => 0xFF,
            ManifestSelector::PendingUpdate => 0xFE,
            ManifestSelector::Descriptor => 0xFD,
        });
        put_reserved(buf, 3);
    }
}
impl Decode for ManifestRequest {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        let segment = SegmentSeq(get_u32(buf, "ManifestRequest.segment")?);
        let selector = match get_u8(buf, "ManifestRequest.chunk")? {
            0xFF => ManifestSelector::AllChunks,
            0xFE => ManifestSelector::PendingUpdate,
            0xFD => ManifestSelector::Descriptor,
            c => ManifestSelector::Chunk(c),
        };
        skip_reserved(buf, 3, "ManifestRequest.reserved")?;
        Ok(Self { segment, selector })
    }
}

/// PULL_REQUEST (0x16):
/// `[SegmentSeq 4][BlockIndex 2][MissingSymbolCount 1][UrgencyMs 2][Flags 1][Reserved 2]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequest {
    /// Segment.
    pub segment: SegmentSeq,
    /// Block wanted.
    pub block: BlockIndex,
    /// `0` = whole block (BLOCK_TRANSMISSION); `n` = that many repair symbols.
    pub missing_symbols: u8,
    /// Requester's time to playout deadline.
    pub urgency_ms: u16,
    /// Bit 0 `BROADCAST_WANT`: rarity signal sent to every Active Set neighbour.
    pub broadcast_want: bool,
}
impl PullRequest {
    /// Encoded length.
    pub const LEN: usize = 12;
}
impl Encode for PullRequest {
    fn encoded_len(&self) -> usize {
        Self::LEN
    }
    fn encode<B: BufMut>(&self, buf: &mut B) {
        buf.put_u32(self.segment.0);
        buf.put_u16(self.block.0);
        buf.put_u8(self.missing_symbols);
        buf.put_u16(self.urgency_ms);
        buf.put_u8(self.broadcast_want as u8);
        put_reserved(buf, 2);
    }
}
impl Decode for PullRequest {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        let segment = SegmentSeq(get_u32(buf, "PullRequest.segment")?);
        let block = BlockIndex(get_u16(buf, "PullRequest.block")?);
        let missing_symbols = get_u8(buf, "PullRequest.missing")?;
        let urgency_ms = get_u16(buf, "PullRequest.urgency")?;
        let broadcast_want = get_u8(buf, "PullRequest.flags")? & 1 != 0;
        skip_reserved(buf, 2, "PullRequest.reserved")?;
        Ok(Self {
            segment,
            block,
            missing_symbols,
            urgency_ms,
            broadcast_want,
        })
    }
}
