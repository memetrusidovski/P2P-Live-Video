//! A node's cryptographic identity: Ed25519 keypair plus the static-puzzle nonce
//! that mints its NodeID, and helpers to produce validation blocks.

use core::net::IpAddr;

use bs_wire::{FrameType, NodeId, PublicKeyBytes, SignatureBytes, ValidationBlock};
use ed25519_dalek::SigningKey;
use rand_core::CryptoRngCore;

use crate::pow::{self, Difficulty};
use crate::signing::{CryptoError, Signer, Verifier};

/// Keypair + minted NodeID.
#[derive(Clone)]
pub struct Identity {
    key: SigningKey,
    node_id: NodeId,
    static_nonce: u64,
}

impl core::fmt::Debug for Identity {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Identity({})", self.node_id.short())
    }
}

impl Identity {
    /// Generate a fresh keypair and solve the static puzzle at `difficulty.c1`.
    pub fn generate<R: CryptoRngCore>(rng: &mut R, difficulty: Difficulty) -> Self {
        let key = SigningKey::generate(rng);
        Self::from_key(key, difficulty)
    }

    /// Deterministic identity from a 32-byte seed (tests and simulation).
    pub fn from_seed(seed: [u8; 32], difficulty: Difficulty) -> Self {
        Self::from_key(SigningKey::from_bytes(&seed), difficulty)
    }

    fn from_key(key: SigningKey, difficulty: Difficulty) -> Self {
        let pk = key.public_key();
        let (static_nonce, node_id) = pow::solve_static(&pk, difficulty.c1, 0);
        Self {
            key,
            node_id,
            static_nonce,
        }
    }

    /// NodeID.
    pub fn node_id(&self) -> NodeId {
        self.node_id
    }
    /// Public key.
    pub fn public_key(&self) -> PublicKeyBytes {
        self.key.public_key()
    }
    /// Static nonce.
    pub fn static_nonce(&self) -> u64 {
        self.static_nonce
    }
    /// Secret seed bytes (for persistence).
    pub fn secret_bytes(&self) -> [u8; 32] {
        self.key.to_bytes()
    }

    /// Solve the dynamic puzzle for an external IP at `c2` bits.
    pub fn solve_dynamic(&self, ip: IpAddr, c2: u32) -> u64 {
        pow::solve_dynamic(&self.node_id, ip, c2, 0)
    }

    /// Build a signed validation block for a pre-session frame whose payload after
    /// the block is `body`.
    pub fn validation_block(
        &self,
        frame_type: FrameType,
        dynamic_nonce: u64,
        timestamp_us: u64,
        body: &[u8],
    ) -> ValidationBlock {
        let pre = ValidationBlock::signed_message(frame_type, timestamp_us, body);
        let digest = blake3::hash(&pre);
        ValidationBlock {
            node_id: self.node_id,
            public_key: self.public_key(),
            static_nonce: self.static_nonce,
            dynamic_nonce,
            timestamp_us,
            signature: self.sign(digest.as_bytes()),
        }
    }

    /// Verify a received validation block: static puzzle, dynamic puzzle against
    /// the observed source IP, and the packet signature.
    pub fn verify_validation_block(
        vb: &ValidationBlock,
        frame_type: FrameType,
        body: &[u8],
        observed_ip: IpAddr,
        difficulty: Difficulty,
        c2_required: u32,
    ) -> Result<(), CryptoError> {
        pow::verify_static(&vb.public_key, vb.static_nonce, &vb.node_id, difficulty.c1)?;
        pow::verify_dynamic(&vb.node_id, observed_ip, vb.dynamic_nonce, c2_required)?;
        let pre = ValidationBlock::signed_message(frame_type, vb.timestamp_us, body);
        let digest = blake3::hash(&pre);
        Verifier::verify(&vb.public_key, digest.as_bytes(), &vb.signature)
    }
}

impl Signer for Identity {
    fn sign(&self, message: &[u8]) -> SignatureBytes {
        self.key.sign(message)
    }
    fn public_key(&self) -> PublicKeyBytes {
        self.key.public_key()
    }
}
