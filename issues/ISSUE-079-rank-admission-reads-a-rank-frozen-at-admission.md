# ISSUE-079: Rank Admission reads a child's rank frozen at admission, so r_min is stale or zero and a lower contributor preempts a higher one

**Status:** Open
**Priority:** Medium
**Component:** Ch1 §1.2.2 Rank Admission Rule case 2; Ch5 §5.2.2 RANK_PROOF (once per admission attempt); App D §D.4.17; SOLUTION-023 residual risk
**Affects:** Every contended enhancement tree; every relay admitted into a free slot without a proof
**File:** `protocol/chapter1/1.2_multi_forest_overlays/2_parent_selection_algorithm.md`, `protocol/chapter5/5.2_proof_of_upload/2_exponential_decay_scoring.md`, `protocol/appendix_d_frame_registry.md`

---

## Summary

Case 2 preempts "the parent's lowest-ranked current child" when a joiner's rank exceeds $1.25\,r_{\min}$. A child's rank is known to the parent only from the `RANK_PROOF` it presented at admission (Ch5 §5.2.2: "sent once per admission attempt"), and nothing refreshes it. SOLUTION-023 accepted this because "decay in $\Theta$ makes it preemptible within minutes", but the rank that admission consumes is the undecayed 60-segment rate, and the decayed $\Theta_A$ has no consumer. The recorded rank is stale in both directions, and a child admitted into a free slot without a proof is recorded at 0 for as long as it sits.

## Detailed Description

1.  Child $C_9$ admitted at segment 0 on 144 receipts: rank $144 \times 131{,}072 \times 8 / 60 / 6 \times 10^6 = 0.42$. By segment 30 it serves 8 children: 430 receipts, rank $1.25$.
2.  Joiner $J$ at segment 30 with 189 receipts: rank $0.55$. Stale rule: $0.55 > 1.25 \times 0.42 = 0.525$, $C_9$ is preempted. Actual ranks: $0.55 < 1.25 \times 1.25 = 1.56$, $J$ is refused.
3.  A child admitted at rank 0 (free slot, no proof) is preempted by any $r > 0$, including a forged $0.01$, one per tree per drain window.
4.  $\varphi(59\text{ s}) = e^{-0.295} = 0.745$ is computed by no consumer; the decay argument of SOLUTION-023 does not reach the admission rule.

## Impact

The rule that implements "contribution buys placement" ranks sitting children by a number that is wrong within a minute of admission. Long-serving relays are displaced by newcomers with lower delivery, and the forest's shallow positions churn.

## Proposed Fix

*   A child re-presents `RANK_PROOF` on the tree stream on a cadence (for example every 30 segments) or when its rank moves by more than the preemption margin; the parent replaces the recorded rank on verification.
*   Record a proof-less admission as rank *unknown*, not 0, and treat unknown as not preemptible until a proof is seen.
*   Touches Ch1 §1.2.2 case 2, Ch5 §5.2.2, App D §D.4.17 triggers, SOLUTION-023's residual list.

## Effort

Small.

Found by the incentive walk (`audits/traces/incentive-timeline.md`).
