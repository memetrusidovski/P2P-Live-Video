//! Pre-session plain-UDP frames: PING, PONG, JOIN, PROBE, PROBE_RESPONSE.
//! (DHT frames — FIND_NODE, GET_PEERS, REGISTER_PEER, STORE_RECORD* — arrive
//! with milestone 3 and currently decode as `Frame::Raw`.)

use bytes::{Buf, BufMut};

use crate::codec::{get_u16, get_u32, get_u8, put_reserved, skip_reserved, Decode, Encode};
use crate::error::Result;
use crate::types::{
    NodeClass, NodeId, PeerFlags, SegmentSeq, StreamId, TreeId, TreeSet, TreeState, WireAddr,
};
use crate::validation::ValidationBlock;

/// PING (0x01): `[VB 152][StreamID 32 (zero if generic)]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ping {
    /// Sender's validation block.
    pub validation: ValidationBlock,
    /// Stream context, or zero.
    pub stream_id: StreamId,
}
impl Encode for Ping {
    fn encoded_len(&self) -> usize {
        ValidationBlock::LEN + 32
    }
    fn encode<B: BufMut>(&self, buf: &mut B) {
        self.validation.encode(buf);
        self.stream_id.encode(buf);
    }
}
impl Decode for Ping {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        Ok(Self {
            validation: ValidationBlock::decode(buf)?,
            stream_id: StreamId::decode(buf)?,
        })
    }
}

/// PONG (0x02): `[VB 152][Reflected AddrFamily 1][IP 4/16][Port 2]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pong {
    /// Responder's validation block.
    pub validation: ValidationBlock,
    /// The requester's address as observed by the responder (STUN-lite).
    pub reflected: WireAddr,
}
impl Encode for Pong {
    fn encoded_len(&self) -> usize {
        ValidationBlock::LEN + self.reflected.wire_len()
    }
    fn encode<B: BufMut>(&self, buf: &mut B) {
        self.validation.encode(buf);
        self.reflected.encode(buf);
    }
}
impl Decode for Pong {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        Ok(Self {
            validation: ValidationBlock::decode(buf)?,
            reflected: WireAddr::decode(buf)?,
        })
    }
}

/// JOIN (0x03): `[VB 152][StreamID 32][NodeClass 1][Reserved 3]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Join {
    /// Joiner's validation block.
    pub validation: ValidationBlock,
    /// Stream being joined.
    pub stream_id: StreamId,
    /// Joiner's self-declared class.
    pub node_class: NodeClass,
}
impl Encode for Join {
    fn encoded_len(&self) -> usize {
        ValidationBlock::LEN + 32 + 4
    }
    fn encode<B: BufMut>(&self, buf: &mut B) {
        self.validation.encode(buf);
        self.stream_id.encode(buf);
        buf.put_u8(self.node_class as u8);
        put_reserved(buf, 3);
    }
}
impl Decode for Join {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        let validation = ValidationBlock::decode(buf)?;
        let stream_id = StreamId::decode(buf)?;
        let node_class = NodeClass::from_code(get_u8(buf, "Join.class")?)?;
        skip_reserved(buf, 3, "Join.reserved")?;
        Ok(Self {
            validation,
            stream_id,
            node_class,
        })
    }
}

/// PROBE (0x0E): `[VB 152][StreamID 32][TreeID 1]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Probe {
    /// Prober's validation block.
    pub validation: ValidationBlock,
    /// Stream.
    pub stream_id: StreamId,
    /// Tree the prober wants a parent in.
    pub tree_id: TreeId,
}
impl Encode for Probe {
    fn encoded_len(&self) -> usize {
        ValidationBlock::LEN + 33
    }
    fn encode<B: BufMut>(&self, buf: &mut B) {
        self.validation.encode(buf);
        self.stream_id.encode(buf);
        buf.put_u8(self.tree_id.0);
    }
}
impl Decode for Probe {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        Ok(Self {
            validation: ValidationBlock::decode(buf)?,
            stream_id: StreamId::decode(buf)?,
            tree_id: TreeId(get_u8(buf, "Probe.tree_id")?),
        })
    }
}

/// PROBE_RESPONSE (0x0F):
/// `[Sender NodeID 32][K_avail 2][Reliability 2 (/65535)][HopCount 1][NodeClass 1]
///  [Flags 1 (bits 0–4 PeerFlags, 5–6 TreeState)][AssignedTrees 1][LiveEdgeSegmentSeq 4]`.
/// Every per-tree field refers to the tree named by the PROBE.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeResponse {
    /// Responder.
    pub sender: NodeId,
    /// Free slots in the probed tree.
    pub k_avail: u16,
    /// Reliability, fixed point over 65535.
    pub reliability: u16,
    /// Responder's hop depth in the probed tree.
    pub hop_count: u8,
    /// Responder's class.
    pub node_class: NodeClass,
    /// Reachability / role flags.
    pub flags: PeerFlags,
    /// State of the probed tree.
    pub tree_state: TreeState,
    /// Every tree the responder is assigned to.
    pub assigned_trees: TreeSet,
    /// Highest segment verified in the probed tree (0 = none).
    pub live_edge: SegmentSeq,
}
impl ProbeResponse {
    /// Encoded length.
    pub const LEN: usize = 32 + 2 + 2 + 1 + 1 + 1 + 1 + 4;
    /// Reliability as a float in `[0, 1]`.
    pub fn reliability_f32(&self) -> f32 {
        self.reliability as f32 / u16::MAX as f32
    }
    /// Convert a float in `[0, 1]` to the wire fixed point.
    pub fn reliability_from_f32(r: f32) -> u16 {
        (r.clamp(0.0, 1.0) * u16::MAX as f32).round() as u16
    }
}
impl Encode for ProbeResponse {
    fn encoded_len(&self) -> usize {
        Self::LEN
    }
    fn encode<B: BufMut>(&self, buf: &mut B) {
        self.sender.encode(buf);
        buf.put_u16(self.k_avail);
        buf.put_u16(self.reliability);
        buf.put_u8(self.hop_count);
        buf.put_u8(self.node_class as u8);
        buf.put_u8(self.flags.to_byte() | self.tree_state.to_flags_bits());
        buf.put_u8(self.assigned_trees.0);
        buf.put_u32(self.live_edge.0);
    }
}
impl Decode for ProbeResponse {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        let sender = NodeId::decode(buf)?;
        let k_avail = get_u16(buf, "ProbeResponse.k_avail")?;
        let reliability = get_u16(buf, "ProbeResponse.reliability")?;
        let hop_count = get_u8(buf, "ProbeResponse.hop_count")?;
        let node_class = NodeClass::from_code(get_u8(buf, "ProbeResponse.class")?)?;
        let fb = get_u8(buf, "ProbeResponse.flags")?;
        let flags = PeerFlags::from_byte(fb)?;
        let tree_state = TreeState::from_flags_byte(fb)?;
        let assigned_trees = TreeSet(get_u8(buf, "ProbeResponse.assigned_trees")?);
        let live_edge = SegmentSeq(get_u32(buf, "ProbeResponse.live_edge")?);
        Ok(Self {
            sender,
            k_avail,
            reliability,
            hop_count,
            node_class,
            flags,
            tree_state,
            assigned_trees,
            live_edge,
        })
    }
}
