//! S/Kademlia static and dynamic proof-of-work puzzles (Ch2 §2.2.1).
//!
//! * Static: `Blake3(PK ‖ N_static)` has `C1` leading zero bits. Its output **is**
//!   the NodeID.
//! * Dynamic: `Blake3(NodeID ‖ IP ‖ N_dynamic)` has `C2` leading zero bits, where
//!   `IP` is the external address as observed by the verifier (4 or 16 bytes).

use core::net::IpAddr;

use bs_wire::{NodeClass, NodeId, PublicKeyBytes};

use crate::leading_zero_bits;

/// Failure to satisfy a puzzle.
pub type PowError = crate::CryptoError;

/// Difficulty parameters (Appendix B §B.1.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Difficulty {
    /// Static puzzle bits `C1` (default 16).
    pub c1: u32,
    /// Absolute floor for the dynamic puzzle `C2_min` (8).
    pub c2_min: u32,
}

impl Difficulty {
    /// Specification defaults.
    pub const SPEC: Difficulty = Difficulty { c1: 16, c2_min: 8 };
    /// Cheap settings for tests and simulation (identities mint in microseconds).
    pub const TEST: Difficulty = Difficulty { c1: 4, c2_min: 2 };

    /// The dynamic tier for a swarm size, before discounts: 8/10/12/14 bits at
    /// `N < 50 / 10^3 / 10^5 / >= 10^5`.
    pub fn dynamic_tier(swarm_size: u32) -> u32 {
        match swarm_size {
            0..=49 => 8,
            50..=999 => 10,
            1000..=99_999 => 12,
            _ => 14,
        }
    }

    /// Effective dynamic difficulty a **solver** must meet:
    /// `max(C2_min, tier(N) - max(δ_class, δ_reconnect))`. Discounts are alternatives,
    /// never additive (Ch2 §2.2.1 "Discounts Do Not Stack").
    pub fn dynamic_required(
        &self,
        swarm_size: u32,
        node_class: NodeClass,
        same_subnet_reconnect: bool,
    ) -> u32 {
        let tier = Self::dynamic_tier(swarm_size);
        let d_class = if node_class.is_leaf() { 2 } else { 0 };
        let d_reconnect = if same_subnet_reconnect { tier / 2 } else { 0 };
        self.c2_min
            .max(tier.saturating_sub(d_class.max(d_reconnect)))
            .min(self.scale_cap())
    }

    /// What a **verifier** accepts: one tier (2 bits) below the tier it observes,
    /// then the claimed discount, floored at `C2_min`. The grace band is applied
    /// before the discount and never composed with it.
    pub fn dynamic_accepted(
        &self,
        observed_swarm_size: u32,
        claimed_class: NodeClass,
        reconnect_verified: bool,
    ) -> u32 {
        let tier = Self::dynamic_tier(observed_swarm_size).saturating_sub(2);
        let d_class = if claimed_class.is_leaf() { 2 } else { 0 };
        let d_reconnect = if reconnect_verified { tier / 2 } else { 0 };
        self.c2_min
            .max(tier.saturating_sub(d_class.max(d_reconnect)))
            .min(self.scale_cap())
    }

    /// In `TEST` mode, cap dynamic difficulty so simulations never grind.
    fn scale_cap(&self) -> u32 {
        if self.c1 < Difficulty::SPEC.c1 {
            self.c1
        } else {
            u32::MAX
        }
    }
}

/// `Blake3(PK ‖ N_static)`.
pub fn static_hash(pk: &PublicKeyBytes, nonce: u64) -> [u8; 32] {
    let mut h = blake3::Hasher::new();
    h.update(pk.as_bytes());
    h.update(&nonce.to_be_bytes());
    *h.finalize().as_bytes()
}

/// Solve the static puzzle: find the smallest nonce from `start` whose hash has
/// `c1` leading zero bits. Returns `(nonce, NodeID)`.
pub fn solve_static(pk: &PublicKeyBytes, c1: u32, start: u64) -> (u64, NodeId) {
    let mut nonce = start;
    loop {
        let h = static_hash(pk, nonce);
        if leading_zero_bits(&h) >= c1 {
            return (nonce, NodeId(h));
        }
        nonce = nonce.wrapping_add(1);
    }
}

/// Verify the static puzzle and that `node_id` is its output.
pub fn verify_static(
    pk: &PublicKeyBytes,
    nonce: u64,
    node_id: &NodeId,
    c1: u32,
) -> Result<(), PowError> {
    let h = static_hash(pk, nonce);
    if &h != node_id.as_bytes() {
        return Err(PowError::NodeIdMismatch);
    }
    let have = leading_zero_bits(&h);
    if have < c1 {
        return Err(PowError::Pow { have, need: c1 });
    }
    Ok(())
}

fn ip_bytes(ip: IpAddr) -> Vec<u8> {
    match ip {
        IpAddr::V4(a) => a.octets().to_vec(),
        IpAddr::V6(a) => a.octets().to_vec(),
    }
}

/// `Blake3(NodeID ‖ IP ‖ N_dynamic)`.
pub fn dynamic_hash(node_id: &NodeId, ip: IpAddr, nonce: u64) -> [u8; 32] {
    let mut h = blake3::Hasher::new();
    h.update(node_id.as_bytes());
    h.update(&ip_bytes(ip));
    h.update(&nonce.to_be_bytes());
    *h.finalize().as_bytes()
}

/// Solve the dynamic puzzle for the given external IP and required bits.
pub fn solve_dynamic(node_id: &NodeId, ip: IpAddr, c2: u32, start: u64) -> u64 {
    let mut nonce = start;
    loop {
        if leading_zero_bits(&dynamic_hash(node_id, ip, nonce)) >= c2 {
            return nonce;
        }
        nonce = nonce.wrapping_add(1);
    }
}

/// Verify the dynamic puzzle against the **observed** source IP.
pub fn verify_dynamic(
    node_id: &NodeId,
    observed_ip: IpAddr,
    nonce: u64,
    c2: u32,
) -> Result<(), PowError> {
    let have = leading_zero_bits(&dynamic_hash(node_id, observed_ip, nonce));
    if have < c2 {
        return Err(PowError::Pow { have, need: c2 });
    }
    Ok(())
}
