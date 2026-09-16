//! Discovery frames: REGISTER_PEER (0x0A) and GET_PEERS (0x0B) (Ch2 §2.3.3).
//! In M2 a single bootstrap guardian answers them; the S/Kademlia routing that
//! finds guardians arrives with M3.

use bytes::{Buf, BufMut};

use crate::codec::{get_u16, get_u8, put_reserved, skip_reserved, Decode, Encode};
use crate::error::{Result, WireError};
use crate::records::{PeerRecord, StreamRecord};
use crate::types::{NodeClass, NodeId, PeerFlags, SignatureBytes, StreamId, TreeSet};
use crate::validation::ValidationBlock;

/// REGISTER_PEER (0x0A):
/// `[K_s 32][VB 152][Port 2][Protocol 1][NodeClass 1][AssignedTrees 1][Flags 1][Reserved 2][Signature 64]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterPeer {
    /// Stream.
    pub stream_id: StreamId,
    /// Registrant's validation block.
    pub validation: ValidationBlock,
    /// UDP port the registrant listens on (address is observed by the guardian).
    pub port: u16,
    /// Self-declared class.
    pub node_class: NodeClass,
    /// Trees relayed.
    pub assigned_trees: TreeSet,
    /// Reachability / role flags.
    pub flags: PeerFlags,
    /// Registration signature (see [`RegisterPeer::signable_bytes`]).
    pub signature: SignatureBytes,
}

impl RegisterPeer {
    /// Protocol type code for UDP.
    pub const PROTOCOL_UDP: u8 = 0x01;
    /// Encoded length.
    pub const LEN: usize = 32 + ValidationBlock::LEN + 2 + 1 + 1 + 1 + 1 + 2 + 64;

    /// `K_s ‖ NodeID ‖ Port ‖ NodeClass ‖ AssignedTrees ‖ Flags ‖ Timestamp`, the
    /// durable statement the guardian re-serves inside Peer Records.
    pub fn signable_bytes(
        stream_id: &StreamId,
        node_id: &NodeId,
        port: u16,
        node_class: NodeClass,
        assigned_trees: TreeSet,
        flags: PeerFlags,
        timestamp_us: u64,
    ) -> Vec<u8> {
        let mut v = Vec::with_capacity(32 + 32 + 2 + 3 + 8);
        v.extend_from_slice(stream_id.as_bytes());
        v.extend_from_slice(node_id.as_bytes());
        v.extend_from_slice(&port.to_be_bytes());
        v.push(node_class as u8);
        v.push(assigned_trees.0);
        v.push(flags.to_byte());
        v.extend_from_slice(&timestamp_us.to_be_bytes());
        v
    }
}

impl Encode for RegisterPeer {
    fn encoded_len(&self) -> usize {
        Self::LEN
    }
    fn encode<B: BufMut>(&self, buf: &mut B) {
        self.stream_id.encode(buf);
        self.validation.encode(buf);
        buf.put_u16(self.port);
        buf.put_u8(Self::PROTOCOL_UDP);
        buf.put_u8(self.node_class as u8);
        buf.put_u8(self.assigned_trees.0);
        buf.put_u8(self.flags.to_byte());
        put_reserved(buf, 2);
        self.signature.encode(buf);
    }
}

impl Decode for RegisterPeer {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        let stream_id = StreamId::decode(buf)?;
        let validation = ValidationBlock::decode(buf)?;
        let port = get_u16(buf, "RegisterPeer.port")?;
        let proto = get_u8(buf, "RegisterPeer.protocol")?;
        if proto != Self::PROTOCOL_UDP {
            return Err(WireError::InvalidField {
                field: "RegisterPeer.protocol",
                value: proto as u64,
            });
        }
        let node_class = NodeClass::from_code(get_u8(buf, "RegisterPeer.class")?)?;
        let assigned_trees = TreeSet(get_u8(buf, "RegisterPeer.assigned_trees")?);
        let flags = PeerFlags::from_byte(get_u8(buf, "RegisterPeer.flags")?)?;
        skip_reserved(buf, 2, "RegisterPeer.reserved")?;
        let signature = SignatureBytes::decode(buf)?;
        Ok(Self {
            stream_id,
            validation,
            port,
            node_class,
            assigned_trees,
            flags,
            signature,
        })
    }
}

/// GET_PEERS request (0x0B): `[VB 152][K_s 32][StarvedTrees 1][WantedTrees 1][ReqFlags 1][Reserved 1]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetPeers {
    /// Requester's validation block.
    pub validation: ValidationBlock,
    /// Stream.
    pub stream_id: StreamId,
    /// Trees the requester failed to find a parent in.
    pub starved_trees: TreeSet,
    /// Trees the requester wants relays for; empty = membership query.
    pub wanted_trees: TreeSet,
    /// Bit 0 `WANT_RELAY_CAPABLE`.
    pub want_relay_capable: bool,
}

impl GetPeers {
    /// Encoded length.
    pub const LEN: usize = ValidationBlock::LEN + 32 + 4;
}

impl Encode for GetPeers {
    fn encoded_len(&self) -> usize {
        Self::LEN
    }
    fn encode<B: BufMut>(&self, buf: &mut B) {
        self.validation.encode(buf);
        self.stream_id.encode(buf);
        buf.put_u8(self.starved_trees.0);
        buf.put_u8(self.wanted_trees.0);
        buf.put_u8(self.want_relay_capable as u8);
        put_reserved(buf, 1);
    }
}

impl Decode for GetPeers {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        let validation = ValidationBlock::decode(buf)?;
        let stream_id = StreamId::decode(buf)?;
        let starved_trees = TreeSet(get_u8(buf, "GetPeers.starved")?);
        let wanted_trees = TreeSet(get_u8(buf, "GetPeers.wanted")?);
        let want_relay_capable = get_u8(buf, "GetPeers.flags")? & 1 != 0;
        skip_reserved(buf, 1, "GetPeers.reserved")?;
        Ok(Self {
            validation,
            stream_id,
            starved_trees,
            wanted_trees,
            want_relay_capable,
        })
    }
}

/// GET_PEERS response (0x0B, same code, distinguished by direction):
/// `[Guardian NodeID 32][Count 1][RespFlags 1][StreamRecordLength 2][PeerRecord × Count][Stream Record]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetPeersResponse {
    /// Responding guardian.
    pub guardian: NodeId,
    /// Bit 0 `SAMPLED`.
    pub sampled: bool,
    /// Records (≤ 20).
    pub records: Vec<PeerRecord>,
    /// The publisher's signed Stream Record, if the guardian holds one.
    pub stream_record: Option<StreamRecord>,
}

impl Encode for GetPeersResponse {
    fn encoded_len(&self) -> usize {
        32 + 1
            + 1
            + 2
            + self.records.iter().map(|r| r.encoded_len()).sum::<usize>()
            + self
                .stream_record
                .as_ref()
                .map(|s| s.encoded_len())
                .unwrap_or(0)
    }
    fn encode<B: BufMut>(&self, buf: &mut B) {
        self.guardian.encode(buf);
        buf.put_u8(self.records.len() as u8);
        buf.put_u8(self.sampled as u8);
        buf.put_u16(
            self.stream_record
                .as_ref()
                .map(|s| s.encoded_len() as u16)
                .unwrap_or(0),
        );
        for r in &self.records {
            r.encode(buf);
        }
        if let Some(s) = &self.stream_record {
            s.encode(buf);
        }
    }
}

impl Decode for GetPeersResponse {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        let guardian = NodeId::decode(buf)?;
        let count = get_u8(buf, "GetPeersResponse.count")?;
        let sampled = get_u8(buf, "GetPeersResponse.flags")? & 1 != 0;
        let sr_len = get_u16(buf, "GetPeersResponse.sr_len")? as usize;
        let mut records = Vec::with_capacity(count as usize);
        for _ in 0..count {
            records.push(PeerRecord::decode(buf)?);
        }
        let stream_record = if sr_len == 0 {
            None
        } else {
            let mut sub = crate::codec::get_vec(buf, sr_len, "GetPeersResponse.stream_record")?;
            let mut cur = &sub[..];
            let sr = StreamRecord::decode(&mut cur)?;
            sub.clear();
            Some(sr)
        };
        Ok(Self {
            guardian,
            sampled,
            records,
            stream_record,
        })
    }
}

/// Either direction of GET_PEERS, decoded by shape: a request always starts with
/// a validation block whose length makes it exactly [`GetPeers::LEN`] bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GetPeersFrame {
    /// Request.
    Request(GetPeers),
    /// Response.
    Response(GetPeersResponse),
}

impl GetPeersFrame {
    /// Decode from a complete payload.
    pub fn from_payload(p: &[u8]) -> Result<Self> {
        if p.len() == GetPeers::LEN {
            Ok(GetPeersFrame::Request(GetPeers::from_slice_exact(p)?))
        } else {
            Ok(GetPeersFrame::Response(GetPeersResponse::from_slice_exact(
                p,
            )?))
        }
    }
}

impl Encode for GetPeersFrame {
    fn encoded_len(&self) -> usize {
        match self {
            GetPeersFrame::Request(r) => r.encoded_len(),
            GetPeersFrame::Response(r) => r.encoded_len(),
        }
    }
    fn encode<B: BufMut>(&self, buf: &mut B) {
        match self {
            GetPeersFrame::Request(r) => r.encode(buf),
            GetPeersFrame::Response(r) => r.encode(buf),
        }
    }
}

// ---------------------------------------------------------------- FIND_NODE / FIND_VALUE (App D §D.4.6)

/// FIND_NODE (0x0C) / FIND_VALUE (0x0D) request: `[VB 152][Target Key 32]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindRequest {
    /// Requester's validation block.
    pub validation: ValidationBlock,
    /// Lookup target (a NodeID or a `K_s`).
    pub target: NodeId,
}

impl FindRequest {
    /// Encoded length.
    pub const LEN: usize = ValidationBlock::LEN + 32;
}

impl Encode for FindRequest {
    fn encoded_len(&self) -> usize {
        Self::LEN
    }
    fn encode<B: BufMut>(&self, buf: &mut B) {
        self.validation.encode(buf);
        self.target.encode(buf);
    }
}

impl Decode for FindRequest {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        Ok(Self {
            validation: ValidationBlock::decode(buf)?,
            target: NodeId::decode(buf)?,
        })
    }
}

/// What a FIND response carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FindResult {
    /// `ResultKind = 0`: the closest contacts the responder knows.
    Nodes(Vec<PeerRecord>),
    /// `ResultKind = 1`: the value stored under the target (a Stream Record).
    Value(Box<StreamRecord>),
}

/// FIND_NODE / FIND_VALUE response:
/// `[Responder NodeID 32][ResultKind 1][Count 1][Records …]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindResponse {
    /// Responding node.
    pub responder: NodeId,
    /// Result.
    pub result: FindResult,
}

impl FindResponse {
    const KIND_NODES: u8 = 0;
    const KIND_VALUE: u8 = 1;
}

impl Encode for FindResponse {
    fn encoded_len(&self) -> usize {
        32 + 2
            + match &self.result {
                FindResult::Nodes(v) => v.iter().map(|r| r.encoded_len()).sum::<usize>(),
                FindResult::Value(s) => s.encoded_len(),
            }
    }
    fn encode<B: BufMut>(&self, buf: &mut B) {
        self.responder.encode(buf);
        match &self.result {
            FindResult::Nodes(v) => {
                buf.put_u8(Self::KIND_NODES);
                buf.put_u8(v.len() as u8);
                for r in v {
                    r.encode(buf);
                }
            }
            FindResult::Value(s) => {
                buf.put_u8(Self::KIND_VALUE);
                buf.put_u8(1);
                s.encode(buf);
            }
        }
    }
}

impl Decode for FindResponse {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        let responder = NodeId::decode(buf)?;
        let kind = get_u8(buf, "FindResponse.kind")?;
        let count = get_u8(buf, "FindResponse.count")?;
        let result = match kind {
            Self::KIND_NODES => {
                let mut v = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    v.push(PeerRecord::decode(buf)?);
                }
                FindResult::Nodes(v)
            }
            Self::KIND_VALUE => {
                if count != 1 {
                    return Err(WireError::InvalidField {
                        field: "FindResponse.count",
                        value: count as u64,
                    });
                }
                FindResult::Value(Box::new(StreamRecord::decode(buf)?))
            }
            other => {
                return Err(WireError::InvalidField {
                    field: "FindResponse.kind",
                    value: other as u64,
                })
            }
        };
        Ok(Self { responder, result })
    }
}

/// Either direction of FIND_NODE / FIND_VALUE, decoded by shape: a request is
/// always exactly [`FindRequest::LEN`] bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FindFrame {
    /// Request.
    Request(FindRequest),
    /// Response.
    Response(FindResponse),
}

impl FindFrame {
    /// Decode from a complete payload.
    pub fn from_payload(p: &[u8]) -> Result<Self> {
        if p.len() == FindRequest::LEN {
            Ok(FindFrame::Request(FindRequest::from_slice_exact(p)?))
        } else {
            Ok(FindFrame::Response(FindResponse::from_slice_exact(p)?))
        }
    }
}

impl Encode for FindFrame {
    fn encoded_len(&self) -> usize {
        match self {
            FindFrame::Request(r) => r.encoded_len(),
            FindFrame::Response(r) => r.encoded_len(),
        }
    }
    fn encode<B: BufMut>(&self, buf: &mut B) {
        match self {
            FindFrame::Request(r) => r.encode(buf),
            FindFrame::Response(r) => r.encode(buf),
        }
    }
}

// ---------------------------------------------------------------- STORE_RECORD / STORE_RECORD_ACK (Ch2 §2.3.3)

/// STORE_RECORD (0x1A): `[VB 152][Stream Record]`, publisher → guardian, once per segment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreRecord {
    /// Publisher's validation block.
    pub validation: ValidationBlock,
    /// The signed record (boxed: it is the largest payload in [`crate::Frame`]).
    pub record: Box<StreamRecord>,
}

impl Encode for StoreRecord {
    fn encoded_len(&self) -> usize {
        ValidationBlock::LEN + self.record.encoded_len()
    }
    fn encode<B: BufMut>(&self, buf: &mut B) {
        self.validation.encode(buf);
        self.record.encode(buf);
    }
}

impl Decode for StoreRecord {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        Ok(Self {
            validation: ValidationBlock::decode(buf)?,
            record: Box::new(StreamRecord::decode(buf)?),
        })
    }
}

/// Trees covered by the per-tree counters of [`StoreRecordAck`] (`M_max`).
pub const ACK_TREES: usize = 6;

/// STORE_RECORD_ACK (0x1B):
/// `[Guardian 32][Accepted 1][RespFlags 1][QueryCount 2][ActiveCount 4][RelayCount 4]
///  [PerTreeRelayCount 6×2][StarvedCount 6×2]`. All counts are raw (Ch2 §2.3.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreRecordAck {
    /// Responding guardian.
    pub guardian: NodeId,
    /// Whether the record was accepted (signature valid and not older than held).
    pub accepted: bool,
    /// Reserved flags.
    pub resp_flags: u8,
    /// Distinct GET_PEERS senders in the last 10 s.
    pub query_count: u16,
    /// Active registrations.
    pub active_count: u32,
    /// Active RELAY-class registrations.
    pub relay_count: u32,
    /// Active relay registrations per tree (bit m−1 set).
    pub per_tree_relay: [u16; ACK_TREES],
    /// Distinct queriers with `StarvedTrees` bit m−1 set in the last 10 s.
    pub starved: [u16; ACK_TREES],
}

impl StoreRecordAck {
    /// Encoded length.
    pub const LEN: usize = 32 + 1 + 1 + 2 + 4 + 4 + 2 * ACK_TREES + 2 * ACK_TREES;
}

impl Encode for StoreRecordAck {
    fn encoded_len(&self) -> usize {
        Self::LEN
    }
    fn encode<B: BufMut>(&self, buf: &mut B) {
        self.guardian.encode(buf);
        buf.put_u8(self.accepted as u8);
        buf.put_u8(self.resp_flags);
        buf.put_u16(self.query_count);
        buf.put_u32(self.active_count);
        buf.put_u32(self.relay_count);
        for c in self.per_tree_relay {
            buf.put_u16(c);
        }
        for c in self.starved {
            buf.put_u16(c);
        }
    }
}

impl Decode for StoreRecordAck {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        let guardian = NodeId::decode(buf)?;
        let accepted = match get_u8(buf, "StoreRecordAck.accepted")? {
            0 => false,
            1 => true,
            v => {
                return Err(WireError::InvalidField {
                    field: "StoreRecordAck.accepted",
                    value: v as u64,
                })
            }
        };
        let resp_flags = get_u8(buf, "StoreRecordAck.flags")?;
        let query_count = get_u16(buf, "StoreRecordAck.query_count")?;
        let active_count = crate::codec::get_u32(buf, "StoreRecordAck.active_count")?;
        let relay_count = crate::codec::get_u32(buf, "StoreRecordAck.relay_count")?;
        let mut per_tree_relay = [0u16; ACK_TREES];
        for c in per_tree_relay.iter_mut() {
            *c = get_u16(buf, "StoreRecordAck.per_tree_relay")?;
        }
        let mut starved = [0u16; ACK_TREES];
        for c in starved.iter_mut() {
            *c = get_u16(buf, "StoreRecordAck.starved")?;
        }
        Ok(Self {
            guardian,
            accepted,
            resp_flags,
            query_count,
            active_count,
            relay_count,
            per_tree_relay,
            starved,
        })
    }
}
