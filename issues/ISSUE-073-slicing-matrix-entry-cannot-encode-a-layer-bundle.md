# ISSUE-073: The slicing-matrix entry cannot encode a multi-layer bundle, so the M = 2 rung every stream starts on is undefined on the wire

**Status:** Open
**Priority:** High
**Component:** App D §D.4.8 slicing matrix; Ch2 §2.3.3 Stream Record; Ch1 §1.2.4 §4.2.1 (case M < L)
**Affects:** Every stream from its first segment until $N_{\text{relay}} \ge 12$; any ladder with more encoder layers than trees
**File:** `protocol/appendix_d_frame_registry.md`, `protocol/chapter2/2.3_stream_registration/3_registration_frames.md`, `protocol/chapter1/1.2_multi_forest_overlays/4_stream_slicing_architecture.md`

---

## Summary

Ch1 §1.2.4 §4.2.1 puts bundles $\{L_0{+}L_1\}$ and $\{L_2\}$ on the two trees of the $M = 2$ rung. The 7-byte matrix entry of App D §D.4.8 carries one `Layer` byte, a `StripeIndex` and a `StripeCount`, and no field says which other layers a tree carries. Ch1 §1.2.4 §4.5 forbids inferring the mapping. The first rung of every stream therefore has no wire encoding, and two implementers will encode it differently.

## Detailed Description

1.  On the reference ladder ($L = 3$) at $M = 2$ the rule produces trees $(T_1, \{L_0, L_1\}, 3000)$ and $(T_2, \{L_2\}, 3000)$ (§4.2.1 table, row $M = 2$).
2.  The matrix entry is `[TreeID 1B][Layer 1B][StripeIndex 1B][StripeCount 1B][Priority 1B][BitrateKbps 2B]` (§D.4.8). One `Layer` value per tree. The Stream Record fixed part (Ch2 §2.3.3) has no `LayerCount`.
3.  Per 250 ms chunk at 6 Mbps: $L_0$ = 3 blocks ($j = 0..2$), $L_1$ = 3 blocks ($j = 3..5$), $L_2$ = 6 blocks ($j = 6..11$). §D.4.8 maps block $j'$ of layer $l$ to "the tree of layer $l$"; an entry $(T_1, \text{Layer } 0)$ names only $j = 0..2$, and $L_1$'s blocks have no tree.
4.  §4.5: "Which layer each tree carries is read from the signed `tree_mapping` ... never inferred." The reference implementation infers bundles from gap contiguity (`src/crates/bs-media/src/mapping.rs` 176-199) using a layer count the Stream Record does not carry; an implementer that emits one entry per (tree, layer) pair produces `NumTrees = 3 \ne M`.

## Impact

At $N < 12$ relays, which is every stream's first minutes and the whole life of a small stream, a viewer written from App D cannot tell which blocks tree 1 carries. Independently written publishers and viewers are not interoperable at the rung where every stream starts.

## Proposed Fix

*   Add a bundle encoding to the matrix entry: a `LayerEnd` byte (inclusive highest layer of the bundle) beside `Layer`, or `StripeCount = 0` marking a bundle whose extent `Layer..LayerEnd` is given; state it in §D.4.8 and the Stream Record layout of Ch2 §2.3.3.
*   Add `LayerCount` to the Stream Record fixed part so a joiner knows the ladder before the first `MANIFEST`.
*   State the bundle encoding in Ch1 §1.2.4 §4.2.1 next to the $M < L$ rule and regenerate `schemas/p2p_live.proto`.
*   Blast radius: §D.4.8, Ch2 §2.3.3, Ch1 §1.2.4; `bs-wire` and `bs-media` in `src/`.

## Effort

Small: one field in two layouts, one sentence in the rule.

Found by the cold-start walk (`audits/traces/cold-start-n2.md`).
