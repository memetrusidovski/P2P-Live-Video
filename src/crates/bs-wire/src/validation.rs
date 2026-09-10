//! The 152-byte S/Kademlia ID validation block (Ch2 §2.2.3).

use bytes::{Buf, BufMut};

use crate::codec::{get_u64, Decode, Encode};
use crate::consts::VALIDATION_BLOCK_LEN;
use crate::error::Result;
use crate::frame_type::FrameType;
use crate::types::{NodeId, PublicKeyBytes, SignatureBytes};

/// `[NodeID 32][PK 32][StaticNonce 8][DynamicNonce 8][Timestamp 8 µs][Signature 64]`.
///
/// Carries **no address**: verifiers check the dynamic proof-of-work against the
/// observed UDP source address, never a self-declared field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValidationBlock {
    /// `Blake3(PK ‖ StaticNonce)`.
    pub node_id: NodeId,
    /// Ed25519 public key.
    pub public_key: PublicKeyBytes,
    /// Static puzzle nonce.
    pub static_nonce: u64,
    /// Dynamic (IP-bound) puzzle nonce.
    pub dynamic_nonce: u64,
    /// Microseconds since the Unix epoch.
    pub timestamp_us: u64,
    /// Packet signature over [`ValidationBlock::signed_message`].
    pub signature: SignatureBytes,
}

impl ValidationBlock {
    /// Encoded length.
    pub const LEN: usize = VALIDATION_BLOCK_LEN;

    /// The pre-image of the packet signature.
    ///
    /// The spec says the signer "signs the hash of the packet payload and
    /// timestamp". This implementation fixes it as
    /// `FrameType(1B) ‖ Timestamp(8B BE) ‖ body`, where `body` is every payload
    /// byte after the validation block; `bs-crypto` hashes it with Blake3 and signs
    /// the digest. Including the frame type stops a PING signature being replayed
    /// as a PROBE.
    pub fn signed_message(frame_type: FrameType, timestamp_us: u64, body: &[u8]) -> Vec<u8> {
        let mut m = Vec::with_capacity(9 + body.len());
        m.push(frame_type as u8);
        m.extend_from_slice(&timestamp_us.to_be_bytes());
        m.extend_from_slice(body);
        m
    }
}

impl Encode for ValidationBlock {
    fn encoded_len(&self) -> usize {
        Self::LEN
    }
    fn encode<B: BufMut>(&self, buf: &mut B) {
        self.node_id.encode(buf);
        self.public_key.encode(buf);
        buf.put_u64(self.static_nonce);
        buf.put_u64(self.dynamic_nonce);
        buf.put_u64(self.timestamp_us);
        self.signature.encode(buf);
    }
}

impl Decode for ValidationBlock {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        Ok(Self {
            node_id: NodeId::decode(buf)?,
            public_key: PublicKeyBytes::decode(buf)?,
            static_nonce: get_u64(buf, "ValidationBlock.static_nonce")?,
            dynamic_nonce: get_u64(buf, "ValidationBlock.dynamic_nonce")?,
            timestamp_us: get_u64(buf, "ValidationBlock.timestamp")?,
            signature: SignatureBytes::decode(buf)?,
        })
    }
}
