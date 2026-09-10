//! The 4-byte common frame header (Appendix D §D.1).

use bytes::{Buf, BufMut};

use crate::codec::{get_u16, get_u8, Decode, Encode};
use crate::error::{Result, WireError};
use crate::frame_type::FrameType;

/// The protocol version this implementation speaks. Lives only in the header.
pub const PROTOCOL_VERSION: u8 = 0x01;

/// `[Version 1B][Type 1B][Payload Length 2B]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameHeader {
    /// Protocol version.
    pub version: u8,
    /// Frame type code.
    pub frame_type: FrameType,
    /// Payload length in bytes, excluding this header.
    pub payload_len: u16,
}

impl FrameHeader {
    /// Encoded length.
    pub const LEN: usize = 4;
    /// Largest payload the 16-bit length field can express.
    pub const MAX_PAYLOAD: usize = u16::MAX as usize;

    /// Header for a frame of the given type and payload length.
    pub fn new(frame_type: FrameType, payload_len: usize) -> Result<Self> {
        if payload_len > Self::MAX_PAYLOAD {
            return Err(WireError::PayloadTooLarge {
                frame: frame_type,
                len: payload_len,
                max: Self::MAX_PAYLOAD,
            });
        }
        Ok(Self {
            version: PROTOCOL_VERSION,
            frame_type,
            payload_len: payload_len as u16,
        })
    }
    /// Total frame length, header included.
    pub fn frame_len(&self) -> usize {
        Self::LEN + self.payload_len as usize
    }
}

impl Encode for FrameHeader {
    fn encoded_len(&self) -> usize {
        Self::LEN
    }
    fn encode<B: BufMut>(&self, buf: &mut B) {
        buf.put_u8(self.version);
        buf.put_u8(self.frame_type as u8);
        buf.put_u16(self.payload_len);
    }
}

impl Decode for FrameHeader {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        let version = get_u8(buf, "FrameHeader.version")?;
        if version != PROTOCOL_VERSION {
            return Err(WireError::UnsupportedVersion(version));
        }
        let ty = get_u8(buf, "FrameHeader.type")?;
        let frame_type = FrameType::from_code(ty).ok_or(WireError::UnknownFrameType(ty))?;
        let payload_len = get_u16(buf, "FrameHeader.payload_len")?;
        Ok(Self {
            version,
            frame_type,
            payload_len,
        })
    }
}
