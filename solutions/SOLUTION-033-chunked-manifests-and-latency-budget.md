# SOLUTION-033: 250 ms Signing Chunks and an Honest Glass-to-Glass Budget

**Closes:** ISSUE-033 (Medium)
**Lives in:** `protocol/chapter4/4.1_segment_serialization/1_gop_serialization.md` (*Chunks*), `2_progressive_blake3_hashing.md`, `3_verification_frame.md`; `protocol/chapter4/4.2_fec_raptorq/3_symbol_packet_layout.md`; `protocol/chapter4/4.3_hybrid_push_pull/1_buffer_sliding_timeline.md` ($\Delta_{\text{buffer}}$); `protocol/chapter1/1.1_scale_latency/2_propagation_latency.md`, `3_logarithmic_scaling.md`; `protocol/chapter7/7.1_source_pinning/1_genesis_key_anchoring.md`; `appendix_d_frame_registry.md` §D.4.8; `appendix_b_parameters.md`; `README.md`
**Class:** A latency term nobody budgeted because it lived in a different chapter from the budget

---

## The problem in one line

The source signs a Merkle root over the whole 1 s segment, so no block can leave it until the segment is fully encoded — a 1 s barrier that appeared nowhere in the "511 ms, leaving 4.4 s" latency argument; with the 4.0 s buffer the real glass-to-glass was $\approx 5.5$ s plus encoder, above the 3–5 s objective.

## The decision

*   **The signing unit is a 250 ms chunk**, four per segment, each with its own `MANIFEST`, Merkle tree and per-layer block counts. The barrier falls from 1.0 s to 0.25 s.
*   **Global block index encodes the chunk**: `BlockIndex = ChunkIndex << 12 | j`. Every block-addressed frame (`RAPTORQ_SYMBOL.SBN`, `BLOCK_PROOF`, `BLOCK_TRANSMISSION`, `PULL_REQUEST`) is unchanged on the wire; the receiver reads the chunk from the upper bits to pick the right root.
*   **The playout buffer is $\Delta_{\text{buffer}} = 3.0$ s**, down from 4.0, with the PUSH and PULL zones at 1.5 s each.
*   **The budget is stated in full**: encoder $0.1$–$0.3$ + barrier $0.25$ + pacing $\le 0.125$ + propagation $0.51$ + buffer $3.0$ $\approx 4.0$–$4.2$ s. The README's "4.0 s sliding buffer" objective is replaced by the glass-to-glass figure.

## Why this and not the alternatives

*   **Per-block Ed25519 signatures** (64 B per 16 KB, no Merkle tree at all) remove the barrier entirely and shrink `BLOCK_PROOF`. They were seriously considered and rejected for scope: they retire the Merkle machinery that SOLUTION-013 and the pull-path `BLOCK_TRANSMISSION` proof are built on, and touch every verification statement in Chapters 4, 5 and 7. Chunks reach the objective with a contained change; per-block signatures remain the natural next step if 250 ms proves too coarse.
*   **A hash chain** (block $i$'s proof commits to blocks $< i$) lets the source sign progressively but makes verification of block $i$ depend on having every earlier block — bad for a pull path that fetches blocks out of order.
*   **Restating the objective at 5.5–6 s** was the honest fallback. Rejected because the interactive use cases the spec names are the ones the missing second hurts, and the fix is cheap.
*   **A 2.0 s buffer** would give $\approx 3.2$ s glass-to-glass but leaves the PULL zone 0.5 s — one repair round-trip — which Scenario A churn would defeat.

## Defects found during verification

*   The segment-level `MANIFEST` had no way to encode per-chunk block counts, and the Ch7 validator keyed its replay cache on `seq_num` alone; both are now `(SegmentSeq, ChunkIndex)`.
*   Retention arithmetic was inherited from the leaf offset (14 s). With the buffer at 3 s and no leaf offset (SOLUTION-031) it is 8 s: playback window + 2 s anchor lag + PULL margin.
*   The late-joiner anchor table in Ch4 §4.3.1 lists lag sources and omits the barrier; that is correct — the barrier delays the *edge*, not the record's estimate of it — but the earlier text read as if the record lag were the only latency, which the new §1.1.3 table corrects.

## The generalisable lesson

**A latency budget must be assembled end to end in one place, with every term owned by a named section.** Propagation lived in Chapter 1, the barrier in Chapter 4 (implicitly), the buffer in Chapter 4 §4.3 — three chapters, no sum.

## Residual risk

Four manifests per second is 4× the signing and manifest traffic; at $\approx 150$ B each this is negligible, but a chunk at very low bitrates (a 480p-only stream) may hold 2–3 blocks, making the Merkle tree nearly degenerate. Harmless, but the encoder should not go below 250 ms.

## Validation owed (Chapter 8)

*   Measured glass-to-glass at $N = 10^4$–$10^6$ against the table; in particular the encoder term, which is a hardware assumption.
*   PSR against $\Delta_{\text{buffer}} \in \{2.5, 3.0, 3.5\}$ s under Scenario A churn, to confirm 3.0 s is the smallest safe value.
