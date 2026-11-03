# ISSUE-075: A parent that heartbeats but never delivers is never evicted, and an unparented relay still accepts children, so orphaned relays can adopt each other and starve a subtree without bound

**Status:** Open
**Priority:** High
**Component:** Ch3 §3.3.2 active probing; Ch3 §3.3.1 standby pick; Ch1 §1.2.2 Rank Admission item 1
**Affects:** Every churn event in which two or more relays of one tree lose their parents together; Ch8 §8.3 scenario A at any $N$
**File:** `protocol/chapter3/3.3_churn_recovery/2_active_probing.md`, `protocol/chapter3/3.3_churn_recovery/1_recovery_timeline.md`, `protocol/chapter1/1.2_multi_forest_overlays/2_parent_selection_algorithm.md`

---

## Summary

Eviction counts any packet ("`LastReceivedTime` counts *any* packet from the parent: media, heartbeat or control", Ch3 §3.3.2), the standby pick reads gossiped $R$ and $K_{\text{avail}}$ without a `PROBE` (Ch3 §3.3.1), and admission item 1 accepts into any free slot. Two relays of tree $m$ that lose their parents at once and rank each other first therefore accept each other, heartbeat each other every 100 ms, and never deliver a block. No rule evicts, no depth changes, no migration fires.

## Detailed Description

1.  Churn storm (Ch8 §8.3 A): 30 % of every per-tree pool's surviving relays are unparented in $m$ at $t \approx 220$ ms. A mutual first pick is a 2-node cycle; a 3-node cycle $A \to B \to C \to A$ needs no mutual pick.
2.  After `ACCEPTED` both sides send heartbeats at $\tau_{\text{ping}} = 100$ ms, so $\tau_{\text{evict}}$ never expires. `BLOCK_PROOF`s do not flow, so `SenderHopDepth` never propagates and the depth rule of Ch1 §1.2.2 never fires.
3.  `TreeState = UNPARENTED` exists in `PROBE_RESPONSE` (§D.4.7) but the standby pick of Ch3 §3.3.1 sends `NEIGHBOR` without probing, and Rank Admission item 1 does not refuse a `NEIGHBOR` for a tree the responder has no parent in.
4.  Starvation duration in the spec: unbounded. The reference implementation bounds it with a data-stall rule and an `ACCEPTED`-side cycle guard (`src/crates/bs-core/src/node.rs` 1135-1145, 2709-2735) that the spec does not have.

## Impact

Every descendant of the cycle in tree $m$ starves until the parents leave on their own. On a base-layer tree that is a frozen player for the whole subtree, with no timer that ends it.

## Proposed Fix

*   A relay that is `UNPARENTED` in $m$ answers `NEIGHBOR(m)` with `DISCONNECT(m, REJECTED_SATURATED)` or a new `REJECTED_UNPARENTED` reason (App D §D.4.3b).
*   A per-tree data-stall deadline: a parent that pushed no block of $m$ for $N$ segments is lost for $m$ even if it heartbeats; name $N$ in App B.
*   A cycle guard at `ACCEPTED`: never accept as a child the node that is your own parent in $m$, nor any node in your parent chain as far as `ROSTER` and `BLOCK_PROOF` depth reveal it.
*   Touches Ch3 §3.3.1-§3.3.2, Ch1 §1.2.2, App D §D.4.3b, App B.

## Effort

Medium: one refusal reason, one timer, one guard.

Found by the parent-loss walk (`audits/traces/parent-loss-repair.md`). Pattern 8 of the recurring list.
