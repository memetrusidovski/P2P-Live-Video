# 5.2 Cryptographic Proof-of-Upload (PoU)

This subchapter introduces the non-repudiable ledger system used to objectively rank the value and historical contribution of peers.

## Deep Dive Topics:
*   [1. Receipt Cryptography](1_receipt_cryptography.md): Non-repudiable Ed25519 PoU payload structure.
*   [2. Contribution Score and Its Presentation](2_exponential_decay_scoring.md): $\Theta_A$ over verified bytes, the rank $\Theta^{\text{rate}}/B$, and the `RANK_PROOF` commit-then-sample bundle.
*   [3. PoU Frame Layout](3_pou_frame.md): `PROOF_OF_UPLOAD` binary layout.
