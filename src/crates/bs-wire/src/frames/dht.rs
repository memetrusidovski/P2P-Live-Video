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
