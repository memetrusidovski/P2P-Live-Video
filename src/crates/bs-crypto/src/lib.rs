//! BitStream cryptography: node identities, proof-of-work puzzles, Blake3 Merkle
//! trees, signatures over wire structures, and the derived values the protocol
//! computes from hashes (stream ids, XOR distance, rendezvous tree assignment).
//!
//! Everything here is deterministic and free of I/O. Randomness is supplied by
//! the caller through `rand_core` traits so the simulator can seed it.

#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms)]

pub mod identity;
pub mod merkle;
pub mod pow;
pub mod rendezvous;
pub mod signing;

pub use identity::Identity;
pub use merkle::{hash_block, MerkleTree};
pub use pow::{Difficulty, PowError};
pub use signing::{CryptoError, Signer, Verifier};

/// Blake3 digest of `data` as a wire [`Hash`](bs_wire::Hash).
pub fn blake3_hash(data: &[u8]) -> bs_wire::Hash {
    bs_wire::Hash(*blake3::hash(data).as_bytes())
}

/// `StreamID = Blake3(PublisherPubKey)` (Ch2 §2.3.1).
pub fn stream_id(publisher_pubkey: &bs_wire::PublicKeyBytes) -> bs_wire::StreamId {
    bs_wire::StreamId(*blake3::hash(publisher_pubkey.as_bytes()).as_bytes())
}

/// XOR distance between two 256-bit keys (Ch2 §2.1.1).
pub fn xor_distance(a: &[u8; 32], b: &[u8; 32]) -> [u8; 32] {
    let mut out = [0u8; 32];
    for i in 0..32 {
        out[i] = a[i] ^ b[i];
    }
    out
}

/// Number of leading zero bits of a 256-bit value.
pub fn leading_zero_bits(h: &[u8; 32]) -> u32 {
    let mut n = 0;
    for b in h {
        if *b == 0 {
            n += 8;
        } else {
            n += b.leading_zeros();
            break;
        }
    }
    n
}
