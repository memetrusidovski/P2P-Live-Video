# ISSUE-078: E = ⌈2ρK⌉ at K = 16 leaves 1.4 to 9 % of blocks per hop unrecoverable from the push, so 9 to 50 % of blocks at depth 7 are pulled and the PULL reserve is exceeded from ρ ≈ 2 %

**Status:** Open
**Priority:** High
**Component:** Ch4 §4.2.2 rules 2–3; Ch1 §1.2.1 §1.3 The Two Budgets ($r_{\text{pull}}$); App B Overhead_fec
**Affects:** Every link with loss above about 1 %; the spec's own "clean" 2 % example; all cellular links
**File:** `protocol/chapter4/4.2_fec_raptorq/2_systematic_dispersion.md`, `protocol/chapter4/4.2_fec_raptorq/1_generator_matrix_math.md`, `protocol/chapter1/1.2_multi_forest_overlays/1_graph_theory_and_slicing.md`, `protocol/appendix_b_parameters.md`

---

## Summary

Rule 2 sizes parity as $E = \max(1, \lceil \text{clamp}(2\rho K, 0.05K, 0.30K) \rceil)$ and rule 3 promises that loss "within the measured $\rho$ budget" is reconstructed locally, "completely eliminating retransmission-induced latency spikes". With $K = 16$ the binomial tail says otherwise: a block fails whenever more than $E$ of its $16 + E$ symbols drop, and that happens to a few percent of blocks per hop. Losses are independent per hop because every relay re-encodes, so the pulled share compounds with depth and exceeds the 10 % PULL reserve at the spec's own clean-link example.

## Detailed Description

1.  Per-hop block failure $P(\text{lost} > E)$ plus the RaptorQ decode residual: $\rho = 0.5\%$: $E = 1$, $0.41\%$; $\rho = 1\%$: $1.38\%$; $\rho = 2\%$ (the §4.2.2 "clean link" example, $E = 1$): $4.71\%$; $\rho = 5\%$: $E = 2$, $5.99\%$; $\rho = 15\%$ (the "cellular" example, $E = 5$): $8.39\%$.
2.  At depth 7 the share of blocks pulled is $1 - (1-p)^7$: $2.8 / 9.3 / 28.7 / 35.1 / 45.9\%$ for the five loss rates.
3.  PULL ingress per viewer: $9.3\% \times 6$ Mbps $= 0.56$ Mbps at $\rho = 1\%$; $1.7$ Mbps at $2\%$, against a PULL reserve of $r_{\text{pull}} u_v = 1.0$ Mbps on the 10 Mbps relay. The reserve is also what repair and late-join backfill draw on.
4.  Parity for a $10^{-3}$ per-hop failure target: $E = 2$ at $\rho \le 1\%$ (12 % overhead), 5 at 5 % (31 %), 10 at 15 % (62 %). The 30 % ceiling cannot reach it above about 5 % loss.

## Impact

On ordinary lossy links a quarter to a half of every deep viewer's stream arrives by PULL, which the budgets size for a few percent. Deep viewers starve first, and the loop of Ch4 §4.2.2 rule 5 (more loss, more parity, fewer slots) cannot see loss that the pull path absorbs.

## Proposed Fix

*   Size $E$ from the binomial tail at a stated per-hop block-failure target (about $10^{-3}$) in Ch4 §4.2.2 and App B, or raise $K$ (64-symbol blocks halve the relative spread at the cost of the store-and-forward term of ISSUE-077).
*   Add the FEC residue times depth as a named consumer of the PULL reserve in Ch1 §1.2.1 §1.3, and restate rule 3 as a bound, not an elimination.
*   Touches Ch4 §4.2.1–§4.2.2, Ch1 §1.2.1, App B.

## Effort

Medium.

Found by the media-pipeline walk (`audits/traces/media-pipeline-latency.md`). Cluster with ISSUE-077.
