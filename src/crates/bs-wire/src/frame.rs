//! The [`Frame`] enum: header + typed payload, with whole-frame encode/decode.

use bytes::{Buf, BufMut, Bytes};

use crate::codec::{get_vec, Decode, Encode};
use crate::error::{Result, WireError};
use crate::frame_type::FrameType;
use crate::frames::*;
use crate::header::FrameHeader;

/// A complete frame. Typed variants exist for every layout this crate
/// implements; everything else is carried as [`Frame::Raw`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum Frame {
    Ping(Ping),
    Pong(Pong),
    Join(Join),
    Probe(Probe),
    ProbeResponse(ProbeResponse),
    Neighbor(Neighbor),
    Accepted(Accepted),
    Disconnect(Disconnect),
    DrainNotice(DrainNotice),
    StreamEnd(StreamEnd),
    ChokeState(ChokeState),
    ManifestRequest(ManifestRequest),
    PullRequest(PullRequest),
    Manifest(Manifest),
    ManifestUpdate(ManifestUpdate),
    BlockProof(BlockProof),
    RaptorQSymbol(RaptorQSymbol),
    BlockTransmission(BlockTransmission),
    RegisterPeer(RegisterPeer),
    GetPeers(GetPeersFrame),
    StreamDescriptor(StreamDescriptor),
    /// A registered frame type whose layout is not yet implemented.
    Raw {
        /// Frame type.
        frame_type: FrameType,
        /// Payload bytes after the header.
        payload: Bytes,
    },
}

impl Frame {
    /// The frame type code.
    pub fn frame_type(&self) -> FrameType {
        match self {
            Frame::Ping(_) => FrameType::PING,
            Frame::Pong(_) => FrameType::PONG,
            Frame::Join(_) => FrameType::JOIN,
            Frame::Probe(_) => FrameType::PROBE,
            Frame::ProbeResponse(_) => FrameType::PROBE_RESPONSE,
            Frame::Neighbor(_) => FrameType::NEIGHBOR,
            Frame::Accepted(_) => FrameType::ACCEPTED,
            Frame::Disconnect(_) => FrameType::DISCONNECT,
            Frame::DrainNotice(_) => FrameType::DRAIN_NOTICE,
            Frame::StreamEnd(_) => FrameType::STREAM_END,
            Frame::ChokeState(_) => FrameType::CHOKE_STATE,
            Frame::ManifestRequest(_) => FrameType::MANIFEST_REQUEST,
            Frame::PullRequest(_) => FrameType::PULL_REQUEST,
            Frame::Manifest(_) => FrameType::MANIFEST,
            Frame::ManifestUpdate(_) => FrameType::MANIFEST_UPDATE,
            Frame::BlockProof(_) => FrameType::BLOCK_PROOF,
            Frame::RaptorQSymbol(_) => FrameType::RAPTORQ_SYMBOL,
            Frame::BlockTransmission(_) => FrameType::BLOCK_TRANSMISSION,
            Frame::RegisterPeer(_) => FrameType::REGISTER_PEER,
            Frame::GetPeers(_) => FrameType::GET_PEERS,
            Frame::StreamDescriptor(_) => FrameType::STREAM_DESCRIPTOR,
            Frame::Raw { frame_type, .. } => *frame_type,
        }
    }

    /// Payload length without the header.
    pub fn payload_len(&self) -> usize {
        match self {
            Frame::Ping(f) => f.encoded_len(),
            Frame::Pong(f) => f.encoded_len(),
            Frame::Join(f) => f.encoded_len(),
            Frame::Probe(f) => f.encoded_len(),
            Frame::ProbeResponse(f) => f.encoded_len(),
            Frame::Neighbor(f) => f.encoded_len(),
            Frame::Accepted(f) => f.encoded_len(),
            Frame::Disconnect(f) => f.encoded_len(),
            Frame::DrainNotice(f) => f.encoded_len(),
            Frame::StreamEnd(f) => f.encoded_len(),
            Frame::ChokeState(f) => f.encoded_len(),
            Frame::ManifestRequest(f) => f.encoded_len(),
            Frame::PullRequest(f) => f.encoded_len(),
            Frame::Manifest(f) => f.encoded_len(),
            Frame::ManifestUpdate(f) => f.encoded_len(),
            Frame::BlockProof(f) => f.encoded_len(),
            Frame::RaptorQSymbol(f) => f.encoded_len(),
            Frame::BlockTransmission(f) => f.encoded_len(),
            Frame::RegisterPeer(f) => f.encoded_len(),
            Frame::GetPeers(f) => f.encoded_len(),
            Frame::StreamDescriptor(f) => f.encoded_len(),
            Frame::Raw { payload, .. } => payload.len(),
        }
    }

    /// The header this frame encodes with.
    pub fn header(&self) -> Result<FrameHeader> {
        FrameHeader::new(self.frame_type(), self.payload_len())
    }

    fn encode_payload<B: BufMut>(&self, buf: &mut B) {
        match self {
            Frame::Ping(f) => f.encode(buf),
            Frame::Pong(f) => f.encode(buf),
            Frame::Join(f) => f.encode(buf),
            Frame::Probe(f) => f.encode(buf),
            Frame::ProbeResponse(f) => f.encode(buf),
            Frame::Neighbor(f) => f.encode(buf),
            Frame::Accepted(f) => f.encode(buf),
            Frame::Disconnect(f) => f.encode(buf),
            Frame::DrainNotice(f) => f.encode(buf),
            Frame::StreamEnd(f) => f.encode(buf),
            Frame::ChokeState(f) => f.encode(buf),
            Frame::ManifestRequest(f) => f.encode(buf),
            Frame::PullRequest(f) => f.encode(buf),
            Frame::Manifest(f) => f.encode(buf),
            Frame::ManifestUpdate(f) => f.encode(buf),
            Frame::BlockProof(f) => f.encode(buf),
            Frame::RaptorQSymbol(f) => f.encode(buf),
            Frame::BlockTransmission(f) => f.encode(buf),
            Frame::RegisterPeer(f) => f.encode(buf),
            Frame::GetPeers(f) => f.encode(buf),
            Frame::StreamDescriptor(f) => f.encode(buf),
            Frame::Raw { payload, .. } => buf.put_slice(payload),
        }
    }

    /// Encode header + payload into `buf`.
    pub fn encode_into<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        self.header()?.encode(buf);
        self.encode_payload(buf);
        Ok(())
    }

    /// Encode header + payload into a new buffer.
    pub fn to_bytes(&self) -> Result<Bytes> {
        let mut v = Vec::with_capacity(FrameHeader::LEN + self.payload_len());
        self.encode_into(&mut v)?;
        Ok(Bytes::from(v))
    }

    /// Decode one frame from the front of `buf` (header included), advancing past it.
    /// Returns [`WireError::Truncated`] if the buffer does not yet hold the whole frame,
    /// which lets a stream reader wait for more bytes.
    pub fn decode<B: Buf>(buf: &mut B) -> Result<Frame> {
        let header = FrameHeader::decode(buf)?;
        let len = header.payload_len as usize;
        if buf.remaining() < len {
            return Err(WireError::Truncated {
                needed: len - buf.remaining(),
                context: "Frame.payload",
            });
        }
        let payload = get_vec(buf, len, "Frame.payload")?;
        Self::decode_payload(header.frame_type, Bytes::from(payload))
    }

    /// Decode a payload whose type is already known.
    pub fn decode_payload(ty: FrameType, payload: Bytes) -> Result<Frame> {
        let mut p = &payload[..];
        let f = match ty {
            FrameType::PING => Frame::Ping(Ping::from_slice_exact(p)?),
            FrameType::PONG => Frame::Pong(Pong::from_slice_exact(p)?),
            FrameType::JOIN => Frame::Join(Join::from_slice_exact(p)?),
            FrameType::PROBE => Frame::Probe(Probe::from_slice_exact(p)?),
            FrameType::PROBE_RESPONSE => Frame::ProbeResponse(ProbeResponse::from_slice_exact(p)?),
            FrameType::NEIGHBOR => Frame::Neighbor(Neighbor::from_slice_exact(p)?),
            FrameType::ACCEPTED => Frame::Accepted(Accepted::from_slice_exact(p)?),
            FrameType::DISCONNECT => Frame::Disconnect(Disconnect::from_slice_exact(p)?),
            FrameType::DRAIN_NOTICE => Frame::DrainNotice(DrainNotice::from_slice_exact(p)?),
            FrameType::STREAM_END => Frame::StreamEnd(StreamEnd::from_slice_exact(p)?),
            FrameType::CHOKE_STATE => Frame::ChokeState(ChokeState::from_slice_exact(p)?),
            FrameType::MANIFEST_REQUEST => {
                Frame::ManifestRequest(ManifestRequest::from_slice_exact(p)?)
            }
            FrameType::PULL_REQUEST => Frame::PullRequest(PullRequest::from_slice_exact(p)?),
            FrameType::MANIFEST => Frame::Manifest(Manifest::from_slice_exact(p)?),
            FrameType::MANIFEST_UPDATE => {
                Frame::ManifestUpdate(ManifestUpdate::from_slice_exact(p)?)
            }
            FrameType::BLOCK_PROOF => Frame::BlockProof(BlockProof::from_slice_exact(p)?),
            FrameType::RAPTORQ_SYMBOL => Frame::RaptorQSymbol(RaptorQSymbol::from_slice_exact(p)?),
            FrameType::BLOCK_TRANSMISSION => {
                let len = p.len();
                Frame::BlockTransmission(BlockTransmission::decode_with_len(&mut p, len)?)
            }
            FrameType::REGISTER_PEER => Frame::RegisterPeer(RegisterPeer::from_slice_exact(p)?),
            FrameType::GET_PEERS => Frame::GetPeers(GetPeersFrame::from_payload(p)?),
            FrameType::STREAM_DESCRIPTOR => {
                Frame::StreamDescriptor(StreamDescriptor::from_slice_exact(p)?)
            }
            other => Frame::Raw {
                frame_type: other,
                payload,
            },
        };
        Ok(f)
    }

    /// Decode a complete datagram / slice holding exactly one frame.
    pub fn from_slice(bytes: &[u8]) -> Result<Frame> {
        let mut cur = bytes;
        let f = Self::decode(&mut cur)?;
        if !cur.is_empty() {
            return Err(WireError::TrailingBytes(cur.len()));
        }
        Ok(f)
    }
}
