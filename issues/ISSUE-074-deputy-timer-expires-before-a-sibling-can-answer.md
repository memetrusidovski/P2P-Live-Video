# ISSUE-074: τ_deputy = 45 ms expires before a sibling can answer on any path over about 22 ms RTT, so the Deputy election never succeeds at the reference RTT

**Status:** Open
**Priority:** Medium
**Component:** Ch1 §1.2.3 §3.3 Phase 3; App B τ_deputy; Ch3 §3.3.1 re-attach cost
**Affects:** Every orphan set whose siblings are more than 22 ms apart; the whole swarm at the spec's own 80 ms reference RTT
**File:** `protocol/chapter1/1.2_multi_forest_overlays/3_topology_healing.md`, `protocol/appendix_b_parameters.md`, `protocol/chapter3/3.3_churn_recovery/1_recovery_timeline.md`, `protocol/appendix_d_frame_registry.md`

---

## Summary

An orphan sends `NEIGHBOR(Priority = HIGH)` to the elected Deputy and arms a fixed 45 ms timer (Ch1 §1.2.3 Phase 3, App B). Siblings are not generally in one another's Active Sets (App D §D.4.15), so the Deputy's answer costs a QUIC handshake plus one request: about 2 RTT, or 1 RTT with a resumption ticket (Ch3 §3.3.1). The timer is shorter than the reply on every path over about 22 ms, and the election silently falls back to the path it was meant to improve on.

## Detailed Description

1.  Deputy reply time: $2\,RTT$ without a ticket, $1\,RTT$ with one. At RTT = 20 / 80 / 250 ms: 40 / 160 / 500 ms (no ticket), 20 / 80 / 250 ms (ticket).
2.  The timer fires at $\tau_{\text{evict}} + 10 + 45$ ms = 275 / 385 / 640 ms; `ACCEPTED` arrives at 270 / 500 / 1,095 ms. Break-even RTT: 22.5 ms without a ticket, 45 ms with one.
3.  The §3.3 timeline ("T = 250 ms: Node C answers with ACCEPTED") is computed for a local branch with $\tau_{\text{evict}} = 200$ ms; the RTT-scaled eviction of SOLUTION-022 was never applied to the timer that follows it.
4.  Nothing says what an orphan does with an `ACCEPTED` that lands after it fell back: it then holds two parents in one tree, or the Deputy holds a slot that no receipt will ever justify until the 3-segment deadline.

## Impact

At the reference RTT the subtree is never preserved; every orphan pays the 45 ms wait and then runs the plain passive-set promotion, a thundering herd on the same candidates that the election exists to prevent. Recovery still completes, so playback is not broken; the mechanism is dead weight.

## Proposed Fix

*   Scale the timer with the path: $\tau_{\text{deputy}} = \max(45\text{ ms}, 2\,SRTT + \tau_{\text{sched}})$ using the orphan's own path as the proxy, in Ch1 §1.2.3 and App B.
*   Or dial the Deputy and the first pool standby concurrently and keep whichever accepts first; release the other with `DISCONNECT_CHOKE`.
*   State the handling of a late `ACCEPTED` (release with `DISCONNECT(TreeID, CHOKE)`).
*   Cluster with the joint-constraint pattern of ISSUE-022; touches Ch1 §1.2.3, App B, Ch3 §3.3.1.

## Effort

Small.

Found by the parent-loss walk (`audits/traces/parent-loss-repair.md`).
