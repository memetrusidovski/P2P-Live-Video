# 4.1 Segment Serialization and Blake3 Merkle-Trees

This subchapter explains how raw video bytes are chunked, serialized, and cryptographically structured for zero-trust P2P validation.

## Deep Dive Topics:
*   [1. GOP and Serialization](1_gop_serialization.md): 1.0s closed-GOP constraints and bitrates.
*   [2. Progressive Blake3 Hashing](2_progressive_blake3_hashing.md): Zero-trust Merkle tree block verification.
*   [3. Verification Frame Layout](3_verification_frame.md): `BLOCK_TRANSMISSION` packet layout with proof paths.
