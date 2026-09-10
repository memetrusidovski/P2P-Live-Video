//! bs-crypto tests. Section references are to the protocol specification.

use std::net::{IpAddr, Ipv4Addr};

use bs_crypto::pow::{self, Difficulty};
use bs_crypto::*;
use bs_wire::*;

fn ident(seed: u8) -> Identity {
    Identity::from_seed([seed; 32], Difficulty::TEST)
}

/// Ch2 §2.2.1: NodeID = Blake3(PK ‖ N_static) with C1 leading zero bits.
#[test]
fn static_puzzle_mints_node_id() {
    let id = ident(1);
    pow::verify_static(
        &id.public_key(),
        id.static_nonce(),
        &id.node_id(),
        Difficulty::TEST.c1,
    )
    .unwrap();
    assert!(leading_zero_bits(id.node_id().as_bytes()) >= Difficulty::TEST.c1);
    // Wrong nonce → mismatch.
    assert!(pow::verify_static(&id.public_key(), id.static_nonce() + 1, &id.node_id(), 1).is_err());
}

/// Ch2 §2.2.1: dynamic puzzle binds NodeID to the observed IP.
#[test]
fn dynamic_puzzle_is_ip_bound() {
    let id = ident(2);
    let ip = IpAddr::V4(Ipv4Addr::new(203, 0, 113, 9));
    let nonce = id.solve_dynamic(ip, 6);
    pow::verify_dynamic(&id.node_id(), ip, nonce, 6).unwrap();
    let other = IpAddr::V4(Ipv4Addr::new(203, 0, 113, 10));
    // Overwhelmingly likely to fail for a different IP at 6 bits; assert on the
    // property that verification depends on IP by checking many nonces.
    let mut fails = 0;
    for n in nonce..nonce + 64 {
        if pow::verify_dynamic(&id.node_id(), other, n, 6).is_err() {
            fails += 1;
        }
    }
    assert!(fails > 50);
}

/// Ch2 §2.2.1 tiers: 8/10/12/14 at N < 50 / 10^3 / 10^5 / ≥ 10^5.
#[test]
fn dynamic_tiers() {
    assert_eq!(Difficulty::dynamic_tier(2), 8);
    assert_eq!(Difficulty::dynamic_tier(49), 8);
    assert_eq!(Difficulty::dynamic_tier(50), 10);
    assert_eq!(Difficulty::dynamic_tier(999), 10);
    assert_eq!(Difficulty::dynamic_tier(1000), 12);
    assert_eq!(Difficulty::dynamic_tier(99_999), 12);
    assert_eq!(Difficulty::dynamic_tier(100_000), 14);
    assert_eq!(Difficulty::dynamic_tier(1_000_000), 14);
}

/// Ch2 §2.2.1 "Discounts Do Not Stack": the table's 512× reduction must not happen.
#[test]
fn discounts_are_alternatives_not_addends() {
    let d = Difficulty::SPEC;
    // Relay at scale: full tier.
    assert_eq!(d.dynamic_required(1_000_000, NodeClass::Relay, false), 14);
    // Leaf: one rung down.
    assert_eq!(d.dynamic_required(1_000_000, NodeClass::Leaf, false), 12);
    // Reconnect: tier/2 = 7 off → 7, but floored at 8.
    assert_eq!(d.dynamic_required(1_000_000, NodeClass::Relay, true), 8);
    // Leaf + reconnect: max(2, 7) = 7 off → floor 8, NOT 14-2-7=5.
    assert_eq!(d.dynamic_required(1_000_000, NodeClass::Leaf, true), 8);
    // Verifier grace band: tier-2, then discount, floored.
    assert_eq!(d.dynamic_accepted(1_000_000, NodeClass::Relay, false), 12);
    assert_eq!(d.dynamic_accepted(1_000_000, NodeClass::Leaf, false), 10);
    assert_eq!(d.dynamic_accepted(1_000_000, NodeClass::Leaf, true), 8);
    // Cold start: everything floors at 8.
    assert_eq!(d.dynamic_required(2, NodeClass::Leaf, true), 8);
}

/// Ch2 §2.2.3: validation block signs FrameType ‖ Timestamp ‖ body, verifies against
/// the observed IP, and rejects a replay under a different frame type.
#[test]
fn validation_block_roundtrip_and_type_binding() {
    let id = ident(3);
    let ip = IpAddr::V4(Ipv4Addr::new(198, 51, 100, 4));
    let dyn_nonce = id.solve_dynamic(ip, 4);
    let body = [0xAB; 32];
    let vb = id.validation_block(FrameType::PING, dyn_nonce, 1_700_000_000_000_000, &body);
    Identity::verify_validation_block(&vb, FrameType::PING, &body, ip, Difficulty::TEST, 4)
        .unwrap();
    assert!(matches!(
        Identity::verify_validation_block(&vb, FrameType::PROBE, &body, ip, Difficulty::TEST, 4),
        Err(CryptoError::BadSignature)
    ));
    let mut tampered = body;
    tampered[0] ^= 1;
    assert!(Identity::verify_validation_block(
        &vb,
        FrameType::PING,
        &tampered,
        ip,
        Difficulty::TEST,
        4
    )
    .is_err());
    // Encodes to exactly 152 bytes.
    assert_eq!(vb.to_vec().len(), 152);
}

/// Ch4 §4.1.2: Merkle proofs verify for every leaf; a flipped byte fails; the
/// worked example {h1, h_{2,3}} for B_0 of 4 blocks has 2 siblings.
#[test]
fn merkle_proofs() {
    let blocks: Vec<Vec<u8>> = (0..12u8).map(|i| vec![i; 16 * 1024]).collect();
    let tree = MerkleTree::from_blocks(blocks.iter().map(|b| b.as_slice()));
    assert_eq!(tree.leaf_count(), 12);
    assert_eq!(tree.depth(), 4, "12 blocks pad to 16 leaves → 4 levels");
    for (i, b) in blocks.iter().enumerate() {
        let proof = tree.proof(i);
        assert_eq!(proof.len(), 4);
        MerkleTree::verify_block(&tree.root(), b, i, &proof).unwrap();
        let mut bad = b.clone();
        bad[100] ^= 0xFF;
        assert!(MerkleTree::verify_block(&tree.root(), &bad, i, &proof).is_err());
        // Wrong index fails too.
        assert!(MerkleTree::verify_block(&tree.root(), b, (i + 1) % 12, &proof).is_err());
    }
    let four = MerkleTree::from_blocks(blocks[..4].iter().map(|b| b.as_slice()));
    let p0 = four.proof(0);
    assert_eq!(p0[0], hash_block(&blocks[1]), "first sibling is h_1");
    let single = MerkleTree::from_blocks([blocks[0].as_slice()]);
    assert_eq!(single.depth(), 0);
    assert_eq!(single.root(), hash_block(&blocks[0]));
}

/// Ch1 §1.2.1: rendezvous assignment is deterministic and ranks every tree.
#[test]
fn rendezvous_assignment() {
    let id = ident(4).node_id();
    let rank = rendezvous::rendezvous_rank(&id, 6);
    assert_eq!(rank.len(), 6);
    let mut sorted = rank.iter().map(|t| t.0).collect::<Vec<_>>();
    sorted.sort();
    assert_eq!(sorted, vec![1, 2, 3, 4, 5, 6]);
    assert_eq!(rendezvous::assigned_tree(&id, 6), rank[0]);
    assert_eq!(
        rendezvous::assigned_tree(&id, 6),
        rendezvous::assigned_tree(&id, 6)
    );
}

/// Ch2 §2.3.1: StreamID = Blake3(publisher pubkey).
#[test]
fn stream_id_derivation() {
    let id = ident(5);
    let sid = stream_id(&id.public_key());
    assert_eq!(
        sid.as_bytes(),
        blake3::hash(id.public_key().as_bytes()).as_bytes()
    );
}

/// Signatures over spec pre-images verify and reject tampering.
#[test]
fn sign_verify() {
    let id = ident(6);
    let msg = b"manifest body bytes";
    let sig = id.sign(msg);
    Verifier::verify(&id.public_key(), msg, &sig).unwrap();
    assert!(Verifier::verify(&id.public_key(), b"other", &sig).is_err());
    assert!(Verifier::verify(&ident(7).public_key(), msg, &sig).is_err());
}
