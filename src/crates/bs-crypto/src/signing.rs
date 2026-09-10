//! Ed25519 signing and verification over the pre-images the wire crate defines.

use bs_wire::{PublicKeyBytes, SignatureBytes};
use ed25519_dalek::{Signature, SigningKey, VerifyingKey};

/// Errors from signature and identity checks.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CryptoError {
    /// Signature did not verify.
    #[error("signature verification failed")]
    BadSignature,
    /// Public key bytes are not a valid Ed25519 point.
    #[error("malformed public key")]
    BadPublicKey,
    /// A Merkle proof did not reach the root.
    #[error("merkle proof mismatch")]
    MerkleMismatch,
    /// Proof-of-work below the required difficulty.
    #[error("proof of work insufficient: {have} bits, need {need}")]
    Pow {
        /// Bits achieved.
        have: u32,
        /// Bits required.
        need: u32,
    },
    /// NodeID does not equal `Blake3(PK ‖ StaticNonce)`.
    #[error("node id does not match public key and static nonce")]
    NodeIdMismatch,
}

/// Something that can produce Ed25519 signatures.
pub trait Signer {
    /// Sign `message` (already the exact pre-image; no hashing is applied here).
    fn sign(&self, message: &[u8]) -> SignatureBytes;
    /// Public key.
    fn public_key(&self) -> PublicKeyBytes;
}

/// Verification helper.
pub struct Verifier;

impl Verifier {
    /// Verify `signature` over `message` under `public_key`.
    pub fn verify(
        public_key: &PublicKeyBytes,
        message: &[u8],
        signature: &SignatureBytes,
    ) -> Result<(), CryptoError> {
        let vk = VerifyingKey::from_bytes(public_key.as_bytes())
            .map_err(|_| CryptoError::BadPublicKey)?;
        let sig = Signature::from_bytes(signature.as_bytes());
        vk.verify_strict(message, &sig)
            .map_err(|_| CryptoError::BadSignature)
    }
}

impl Signer for SigningKey {
    fn sign(&self, message: &[u8]) -> SignatureBytes {
        SignatureBytes(ed25519_dalek::Signer::sign(self, message).to_bytes())
    }
    fn public_key(&self) -> PublicKeyBytes {
        PublicKeyBytes(self.verifying_key().to_bytes())
    }
}
