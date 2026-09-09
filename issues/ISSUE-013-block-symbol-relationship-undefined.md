# ISSUE-013: 16 KB Merkle Block vs 1024 B RaptorQ Symbol Relationship Is Undefined

**Status:** Resolved  
**Priority:** High  
**Component:** Ch4 — Media Distribution (4.1 serialization / 4.2 FEC / 4.3 push-pull)  
**Affects:** The entire data plane; zero-trust verify-then-forward requirement  
**File:** `protocol/chapter4/4.1_segment_serialization/`, `protocol/chapter4/4.2_fec_raptorq/`

---

## Summary

Merkle verification and PoU receipts operate on 16 KB blocks; FEC and loss recovery operate on 1024 B RaptorQ symbols. The spec never defines how the two units map: whether RaptorQ encodes per-block or per-segment, whether symbols carry Merkle proofs, or how a peer verifies a symbol before forwarding it — despite ch4.1's zero-trust requirement that nothing unverified is relayed. Ch1.2.3 says churn gaps are healed "via RaptorQ FEC symbols" while ch5.2 issues PoU receipts per Merkle-verified block; without a defined mapping these are incompatible readings of the same data path.

## Proposed Fix

**One RaptorQ source block per 16 KB Merkle block: K = 16 source symbols of 1024 B.**

- The `SBN` field of `RAPTORQ_SYMBOL` (0x12) *is* the Merkle block index within the segment (fix the sentence in ch4.2.3 claiming SBN maps to a segment).
- The verification boundary is the **block**. Symbols are a link-local loss-recovery encoding: never individually Merkle-verified, never forwarded raw. Interior relays operate **decode → Merkle-verify → re-encode → push**, generating fresh symbols per downstream link with parity rate E from that child's reported loss ρ (integer-rounded, `E = max(1, ...)` since 5% of K=16 < 1).
- Merkle proofs travel once per block: in `BLOCK_TRANSMISSION` (0x10) on the pull path, and in a new `BLOCK_PROOF` (0x13) frame preceding the block's symbols on the push path. The signed segment manifest travels as `MANIFEST` (0x11).
- The last block of a segment is zero-padded to 16 KB for hashing/FEC; true length rides in the manifest's `SegmentByteLength`.
- PoU receipts remain per-block (unchanged, consistent with ch5).

Rejected alternative: per-symbol Merkle proofs (6×32 B on a 1 KB payload ≈ 19% overhead — a non-starter).

---

## Resolution

Applied the one-source-block-per-Merkle-block design:

- `protocol/chapter4/4.1_segment_serialization/2_progressive_blake3_hashing.md` — new "Blocks vs. Symbols: The Verification Boundary" section: K=16 mapping, SBN = Merkle block index, symbols never individually verified or forwarded, relays operate decode → verify → re-encode → push with per-link parity, failed blocks flag the sender; PoU stays per verified block.
- `protocol/chapter4/4.2_fec_raptorq/3_symbol_packet_layout.md` — SBN redefined correctly (was "maps to a segment"), ESI<16 = source slices, padding rule for the final block.
- `protocol/chapter4/4.2_fec_raptorq/2_systematic_dispersion.md` — parity is per block per downstream link using child-reported ρ; `E = max(1, ceil(clamp(2ρK, 0.05K, 0.30K)))` with worked examples at K=16.
- `protocol/chapter4/4.3_hybrid_push_pull/1_buffer_sliding_timeline.md` — wire units per zone (push: BLOCK_PROOF + symbol datagrams; pull: PULL_REQUEST → BLOCK_TRANSMISSION or repair symbols).
- `protocol/appendix_d_frame_registry.md` — MANIFEST (0x11) and BLOCK_PROOF (0x13) layouts (created under ISSUE-012).
- `protocol/appendix_b_parameters.md` — added T_symbol=1024B, K_block=16, E_min=1; FEC overhead corrected to adaptive 5–30%.
