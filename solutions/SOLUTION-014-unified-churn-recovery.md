# SOLUTION-014: Unified Churn Recovery (Election as a Layer Over Promotion)

**Closes:** ISSUE-014 (High)
**Lives in:** `protocol/chapter1/1.2_multi_forest_overlays/3_topology_healing.md`, `protocol/chapter3/3.3_churn_recovery/1_recovery_timeline.md`, `appendix_b_parameters.md`
**Class:** Structural — two algorithms for one job

---

## The problem in one line

Ch1 §1.2.3 healed a dead parent by deterministic sibling election; Ch3 §3.3 healed the same failure by independent passive-set promotion. Both claimed the same 250 ms budget, produced different topologies, never referenced each other, and the election had no tie-break, no Deputy-refusal path and no capacity check.

## The decision

**Passive-set promotion (Ch3 §3.3) is the canonical primitive. Sibling election is a coordination layer over it, and every failure path degrades to it.**

The election answers only *who* re-attaches the subtree. The Deputy then runs Ch3 §3.3 to find its own parent — it is coordinator-selection, not a rival algorithm.

## Why this framing rather than picking one

Both mechanisms solve real and *different* problems. Passive-set promotion is correct for a single orphan and needs no shared state. Sibling election exists because $k$ orphans of one dead parent would otherwise stampede the same passive-set candidates and shred the subtree. Deleting either loses something.

Making one a layer over the other gives a property neither had alone: **the worst case is never worse than plain HyParView recovery.** Every degradation — no roster, stale roster, leaf-only self, Deputy unresponsive within $\tau_{\text{deputy}} = 45\text{ ms}$ — lands on the primitive that would have run anyway. The election can only help.

The supporting details the original election lacked, now specified: a sequence-numbered child roster as shared input, ties broken by lowest NodeID, leaf-class nodes excluded, and a bounded fallback timer.

## Defect found: the roster was $O(k^2)$ and physically impossible at scale

The roster listed **every** child, $35$ bytes each, sent to all $k$ children every second:

| Children $k$ | Roster traffic | Share of uplink |
| ---: | ---: | ---: |
| 100 | 2.8 Mbps | 2.80% — already past $\text{CDO} \le 2\%$ |
| 1,000 | 280 Mbps | 28% |
| 10,000 | 28 Gbps | **280% — impossible** |

A super node cannot distribute its own child roster. This fix and SOLUTION-007 (which created 10,000-child relays) were written independently and are directly incompatible at scale.

**Resolution: the roster carries only the top $R_{\text{roster}} = 8$ children** by $K_v$ descending, NodeID ascending, leaf-class excluded. Cost becomes a flat 0.22% of uplink at *every* fan-out.

Nothing is lost, and that is the point worth keeping: the roster's only job is to identify the *highest*-capacity sibling, so children ranked below the top 8 were never candidates. The full list was carrying information the algorithm never reads.

## Second defect: the election created the herd it was designed to prevent

With $k = 10{,}000$ orphans and a Deputy holding 10 free slots, 9,990 send a `RELAY_JOIN_REQUEST`, get nothing, wait out 45 ms, and fall back to Ch3 §3.3 regardless. The election would have **added 45 ms of latency to 99.9% of the subtree** while swapping a stampede on the passive set for a stampede on one Deputy.

Resolution: orphans partition deterministically across the roster's top entries.

$$D = \min\left(R_{\text{roster}},\ \lceil k / K_v^{\text{deputy}} \rceil\right), \qquad \text{deputy\_index}(o) = \text{Blake3}(NodeID_o) \bmod D$$

And crucially — when even $D$ deputies cannot absorb the orphans, the surplus **skips the election entirely rather than waiting**. Membership in that surplus is computed from the same shared roster, so it costs no coordination, and it means the 45 ms Deputy wait is only ever paid by an orphan that had a real chance of being served.

## The generalisable lesson

**A mechanism that batches work onto one coordinator must check that the coordinator can absorb the batch — using capacity data it already publishes.** The roster carried $K_v$ for every child and the election read it only to rank candidates, never to ask whether the winner could actually hold the subtree. The information needed to prevent the failure was already on the wire.

More broadly, this issue and SOLUTION-007 show the same pattern from opposite ends: a fix validated at one scale, composed with a fix validated at another, breaking in the overlap. The roster was sized for a 10-child home relay; multi-tree assignment created 10,000-child servers. Neither document was wrong in isolation.

## Validation owed (Chapter 8)

* Recovery time distribution for a dead parent with $k = 10, 100, 1{,}000, 10{,}000$ children — the multi-deputy partition should hold the 250 ms budget across all of them.
* Whether $R_{\text{roster}} = 8$ gives enough deputy diversity at high fan-out, or whether it should scale with $k$ up to a cap.
* Deputy-election consistency under roster staleness during rapid churn: how often do orphans disagree because they hold different roster sequence numbers?
