//! NAT traversal and emergent-relay frames: PUNCH_REQUEST (App D §D.4.14),
//! RELAY_PROPOSAL and RELAY_BIND (Ch6 §6.3.3).
//!
//! Both PUNCH_REQUEST forms carry the validation block, on UDP and on the control
//! stream alike: one codec, and the referrer verifies the requester either way.

use bytes::{Buf, BufMut};

use crate::codec::{get_u16, get_u8, put_reserved, skip_reserved, Decode, Encode};
use crate::error::{Result, WireError};
use crate::records::PeerRecord;
use crate::types::{NodeId, StreamId, TreeId};
use crate::validation::ValidationBlock;

/// PUNCH_REQUEST request form, requester to referrer:
/// `[Validation Block][Target NodeID 32][StreamID 32]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PunchRequest {
    /// Requester's validation block.
    pub validation: ValidationBlock,
    /// The CONE peer the requester wants to PROBE.
    pub target: NodeId,
    /// Stream.
    pub stream_id: StreamId,
}
impl PunchRequest {
    /// Encoded length.
    pub const LEN: usize = ValidationBlock::LEN + 64;
}
impl Encode for PunchRequest {
    fn encoded_len(&self) -> usize {
        Self::LEN
    }
    fn encode<B: BufMut>(&self, buf: &mut B) {
        self.validation.encode(buf);
        self.target.encode(buf);
        self.stream_id.encode(buf);
    }
}
impl Decode for PunchRequest {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        Ok(Self {
            validation: ValidationBlock::decode(buf)?,
            target: NodeId::decode(buf)?,
            stream_id: StreamId::decode(buf)?,
        })
    }
}

/// PUNCH_REQUEST forward form, referrer to target:
/// `[Validation Block][Requester PeerRecord 42/54][StreamID 32]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PunchForward {
    /// Referrer's validation block.
    pub validation: ValidationBlock,
    /// The requester, with the address the referrer observed.
    pub requester: PeerRecord,
    /// Stream.
    pub stream_id: StreamId,
}
impl Encode for PunchForward {
    fn encoded_len(&self) -> usize {
        ValidationBlock::LEN + self.requester.encoded_len() + 32
    }
    fn encode<B: BufMut>(&self, buf: &mut B) {
        self.validation.encode(buf);
        self.requester.encode(buf);
        self.stream_id.encode(buf);
    }
}
impl Decode for PunchForward {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        Ok(Self {
            validation: ValidationBlock::decode(buf)?,
            requester: PeerRecord::decode(buf)?,
            stream_id: StreamId::decode(buf)?,
        })
    }
}

/// Either form of PUNCH_REQUEST, told apart by length.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PunchFrame {
    /// Requester to referrer.
    Request(PunchRequest),
    /// Referrer to target.
    Forward(PunchForward),
}
impl PunchFrame {
    /// Decode from a complete payload.
    pub fn from_payload(p: &[u8]) -> Result<Self> {
        if p.len() == PunchRequest::LEN {
            Ok(PunchFrame::Request(PunchRequest::from_slice_exact(p)?))
        } else {
            Ok(PunchFrame::Forward(PunchForward::from_slice_exact(p)?))
        }
    }
}
impl Encode for PunchFrame {
    fn encoded_len(&self) -> usize {
        match self {
            PunchFrame::Request(r) => r.encoded_len(),
            PunchFrame::Forward(f) => f.encoded_len(),
        }
    }
    fn encode<B: BufMut>(&self, buf: &mut B) {
        match self {
            PunchFrame::Request(r) => r.encode(buf),
            PunchFrame::Forward(f) => f.encode(buf),
        }
    }
}

/// `BindFlags` bit 0: the proposer is the broadcaster binding an ingress relay.
pub const BIND_SOURCE_INGRESS: u8 = 0x01;
/// `BindFlags` bit 1 (RELAY_BIND only): the relay declines.
pub const BIND_REJECTED: u8 = 0x02;

/// RELAY_PROPOSAL (0x30): `[Target NodeID 32][Target Port 2][TreeID 1][BindFlags 1]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelayProposal {
    /// The parent the proposer could not reach, or zero to let the relay choose.
    pub target: NodeId,
    /// The target's port.
    pub target_port: u16,
    /// Tree to bridge; 0 = PULL service only.
    pub tree_id: TreeId,
    /// Bind flags.
    pub bind_flags: u8,
}
impl RelayProposal {
    /// Encoded length.
    pub const LEN: usize = 36;
}
impl Encode for RelayProposal {
    fn encoded_len(&self) -> usize {
        Self::LEN
    }
    fn encode<B: BufMut>(&self, buf: &mut B) {
        self.target.encode(buf);
        buf.put_u16(self.target_port);
        buf.put_u8(self.tree_id.0);
        buf.put_u8(self.bind_flags);
    }
}
impl Decode for RelayProposal {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        Ok(Self {
            target: NodeId::decode(buf)?,
            target_port: get_u16(buf, "RelayProposal.port")?,
            tree_id: TreeId(get_u8(buf, "RelayProposal.tree")?),
            bind_flags: get_u8(buf, "RelayProposal.flags")?,
        })
    }
}

/// RELAY_BIND (0x31): `[Client A 32][Client B 32][TreeID 1][BindFlags 1][Reserved 2]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelayBind {
    /// The NAT-blocked peer being bridged.
    pub client_a: NodeId,
    /// The parent the relay obtained a slot at.
    pub client_b: NodeId,
    /// Tree bridged.
    pub tree_id: TreeId,
    /// Bind flags.
    pub bind_flags: u8,
}
impl RelayBind {
    /// Encoded length.
    pub const LEN: usize = 68;
    /// Whether the relay declined.
    pub fn rejected(&self) -> bool {
        self.bind_flags & BIND_REJECTED != 0
    }
}
impl Encode for RelayBind {
    fn encoded_len(&self) -> usize {
        Self::LEN
    }
    fn encode<B: BufMut>(&self, buf: &mut B) {
        self.client_a.encode(buf);
        self.client_b.encode(buf);
        buf.put_u8(self.tree_id.0);
        buf.put_u8(self.bind_flags);
        put_reserved(buf, 2);
    }
}
impl Decode for RelayBind {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        let client_a = NodeId::decode(buf)?;
        let client_b = NodeId::decode(buf)?;
        let tree_id = TreeId(get_u8(buf, "RelayBind.tree")?);
        let bind_flags = get_u8(buf, "RelayBind.flags")?;
        skip_reserved(buf, 2, "RelayBind.reserved")?;
        Ok(Self {
            client_a,
            client_b,
            tree_id,
            bind_flags,
        })
    }
}

#[allow(dead_code)]
fn _unused(_: WireError) {}
