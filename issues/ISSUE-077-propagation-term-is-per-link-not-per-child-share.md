# ISSUE-077: The depth proof's transmission term is per link, not per child share; a full relay spends one block period per hop and glass-to-glass is 4.7 to 5.0 s

**Status:** Open
**Priority:** Medium
**Component:** Ch1 §1.1.3 propagation model and glass-to-glass budget; Ch4 §4.1.2 per-hop cost; SOLUTION-013, SOLUTION-033
**Affects:** Every path through a relay that holds its slots; the stated 4.0 s figure at every $N$
**File:** `protocol/chapter1/1.1_scale_latency/3_logarithmic_scaling.md`, `protocol/chapter4/4.1_segment_serialization/2_progressive_blake3_hashing.md`, `protocol/chapter1/1.2_multi_forest_overlays/1_graph_theory_and_slicing.md`

---

## Summary

The budget takes $\tau_{\text{trans}} \approx 13$ ms for a 16 KB block "over a 10 Mbps active stream link". A relay with $K_v = 10$ children on that link must put ten copies of the block on its uplink before its last child holds it, and verify-before-forward forbids cut-through (Ch4 §4.1.2). The per-hop term is $K_v \cdot \text{Size}_{\text{block}} \cdot \Omega / u_v \approx 0.9\,\text{Size}_{\text{block}} / B_m$, independent of $u_v$: about 150 ms, not 13.

## Detailed Description

1.  $K_v(0.75\text{ Mbps}) = \lfloor 0.9 \times 10 / (0.75 \times 1.1528) \rfloor = 10$ (Ch1 §1.2.1 §1.3 worked value). Block on the wire at $E = 1$: $17 \times 1{,}094 + 200 \approx 18.8$ KB $= 150$ kbit.
2.  $10 \times 150$ kbit / 10 Mbps = 150 ms to the last child, 82 ms to the mean child. Per hop: $40 + 150 + 20 = 210$ ms (worst), 142 ms (mean), against 73 ms in §1.1.3.
3.  Seven hops: $70 + 6 \times 210 = 1{,}330$ ms worst, 922 ms mean, against 511 ms. Glass-to-glass on the worst path: $4.79$–$4.99$ s with the table's pacing term, against the stated $4.0$–$4.2$ s.
4.  At the nominal 1 Mbps slice and fan-out 8 the term is 118 ms per hop and glass-to-glass $4.6$–$4.8$ s. SOLUTION-013's per-hop table carries the same 13 ms.

## Impact

The 3–5 s objective still holds, with 0.01–0.2 s of margin instead of the stated 0.8–1.0 s. One repair cycle, which the budget claims to leave room for, no longer fits on the worst path.

## Proposed Fix

*   Restate $\tau_{\text{trans}}$ as the child-share form in Ch1 §1.1.3 and SOLUTION-013 and re-sum the budget.
*   Recover margin by ordering each block's emission to the relay-class children of the tree first (about $K_v/M$ of the children), so the compounding path sees about 30 ms per hop and only leaves see the full share.
*   Touches Ch1 §1.1.3, Ch4 §4.1.2, SOLUTION-033's budget table.

## Effort

Small for the restatement; medium for the emission order.

Found by the media-pipeline walk (`audits/traces/media-pipeline-latency.md`).
