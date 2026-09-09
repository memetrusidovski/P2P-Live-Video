# SOLUTION-039: Collusion Symmetry Is Judged per Tree

**Closes:** ISSUE-039 (Medium)
**Lives in:** `protocol/chapter5/5.3_reputation_auditing/1_graph_collusion_auditing.md`; `protocol/chapter5/5.2_proof_of_upload/3_pou_frame.md` (`TreeID` in the receipt)
**Class:** A detector calibrated on a topology the placement rule itself violates (recurring pattern #5)

---

## The problem in one line

If $A$ relays $T_1$ to $B$ and $B$ relays $T_3$ to $A$, the two flows are the same bitrate in opposite directions — aggregate ratio exactly $1.0$ — and an honest relay in a swarm of 20–1000 relays carries several such edges, so the symmetry detector flagged honest mutual parents throughout the mid-size range the small-swarm guard does not cover.

## The decision

Receipts carry `TreeID`, and symmetry is tested **per tree**: only bidirectional flow *within one tree* in one window is suspicious, because a tree edge has one direction. `PULL` receipts, which are legitimately bidirectional, are excluded from the test and bounded by the per-counterparty credit cap instead (SOLUTION-024).

## Why this and not the alternatives

*   **Raising $N_{\text{collusion}}$** to where mutual edges are rare needs $N_{\text{relay}} \gtrsim 5000$ to bring the expected count under $0.1$ — a guard so high the detector would be off for most streams.
*   **Deriving the tree from `(SegmentSeq, BlockIndex)` via the slicing matrix** instead of carrying a byte works for tree receipts but not across a resize, when the same block index may move between trees; one byte is cheaper than the ambiguity.

## Defects found during verification

*   Expected symmetric edges per honest node, $K_v^2 (M-1)/N_{\text{relay}}$: 10 at 50 relays, 2.5 at 200, 0.5 at 1000. The worked table in §5.3.1 had checked degree 1, degree 40 and asymmetric flows — never the symmetric honest case the placement rule produces. It now does.
*   A false flag fed `REPUTATION_AUDIT_GOSSIP` with no evidence check (ISSUE-035); with evidence-carrying accusations the same false flag would have been *verifiable* evidence — real receipts, real symmetry — and would still have evicted honest relays. The per-tree rule is therefore load-bearing for SOLUTION-035, not only a false-positive fix.

## The generalisable lesson

**A heuristic that says "honest traffic never looks like X" must be checked against every topology the protocol's own placement rules generate.** The mutual-parent pair is not an edge case; it is the expected shape of a mid-sized forest.

## Residual risk

Within-tree bidirectional flow can occur honestly across a churn event (parent evicted, rejoins under its former child within 60 s); the ratio test then also has to pass, and three independent accusers are still needed to evict.

## Validation owed (Chapter 8)

*   False-flag rate for honest relays at $N_{\text{relay}} \in \{20, 50, 200, 1000\}$ with the per-tree test; it should be $\approx 0$.
