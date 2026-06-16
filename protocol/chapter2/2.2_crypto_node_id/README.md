# 2.2 S/Kademlia Cryptographic ID Generation & Proof-of-Work

This subchapter details the mathematical puzzles a node must solve to generate a valid network identity, preventing massive botnet-style Sybil attacks.

## Deep Dive Topics:
*   [1. Static and Dynamic Puzzles](1_static_dynamic_puzzles.md): Blake3 Proof-of-Work bound to IP addresses.
*   [2. Sybil Defense Math](2_sybil_defense_math.md): Difficulty parameters ($C_1$, $C_2$) and code examples.
*   [3. Validation Frame Layout](3_validation_frame.md): Secure S/Kademlia header byte layout.
