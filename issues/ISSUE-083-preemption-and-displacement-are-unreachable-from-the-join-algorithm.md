# ISSUE-083: Rank Admission cases 2 and 3 are unreachable: the join algorithm's probe filter drops every K_avail = 0 candidate before a NEIGHBOR is sent

**Status:** Open
**Priority:** High
**Component:** Ch1 §1.2.2 §2.2 step 2 and Rank Admission cases 2–3; Ch1 §1.2.5 §5.5
**Affects:** Every contended tree; the whole incentive axiom and the leaf service floor
**File:** `protocol/chapter1/1.2_multi_forest_overlays/2_parent_selection_algorithm.md`, `protocol/chapter1/1.2_multi_forest_overlays/5_node_classes.md`

---

## Summary

The Rank Admission Rule decides what a parent does with a `NEIGHBOR` when it has no free slot: preempt a lower-ranked child (case 2) or displace a pure subscriber for a leaf (case 3). The join algorithm never sends that `NEIGHBOR`: step 2 keeps a candidate only `if probe_response.success and probe_response.available_slots > 0`, so a full parent is scored out before any request. The only way a request reaches a full parent is the race between probe and request. ISSUE-061 names one instance (a relay of the tree); the gap is general.

## Detailed Description

1.  Joiner $J$ at rank 1.25 and every tree-5 candidate at $K_{\text{avail}} = 0$: the round fails and counts toward the shed rule; no `NEIGHBOR` is sent, although $1.25 > 1.25 \times 0$ would preempt any rank-0 child.
2.  Leaf $L$ and a saturated $L_0$ parent with leaves below the leaf share: the same filter; §5.5's "a leaf is served whenever it asks and the share is not yet full" never gets an ask.
3.  Race probability at $N = 10^4$ with 2,000 joiners in 10 s: 800 `NEIGHBOR`/s over 3,876 depth-3 relays $= 0.2$/s per relay $\times 160$ ms probe-to-request window $\approx 3\%$ per attempt.
4.  Two readings of the text diverge: the admission rule assumes requests reach full parents; the pseudocode forbids it. SOLUTION-023 ("Preemption is what turns rank into placement") assumes the first.

## Impact

Rank buys nothing in a full tree, and leaves get the floor only by luck. The implementation fixes only the ISSUE-061 relay case; its leaf-share code has no caller.

## Proposed Fix

*   In step 2, keep a `SERVING` candidate with $K_{\text{avail}} = 0$ when the requester is leaf-class and the tree carries $L_0$, or holds a `RANK_PROOF` and the tree carries an enhancement layer; score it with `CapacityCredit = 0` and request it after every free-slot candidate.
*   Count a candidate as saturated only on `REJECTED_SATURATED`, never on the probe alone.
*   Touches Ch1 §1.2.2 §2.2 pseudocode and the admission rule; resolve together with ISSUE-061.

## Effort

Small.

Found by the flash-crowd walk (`audits/traces/flash-crowd.md`). Cluster with ISSUE-061.
