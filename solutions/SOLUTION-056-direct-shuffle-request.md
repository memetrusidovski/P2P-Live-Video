# SOLUTION-056: A `SHUFFLE` With `TTL = 0` Over a Session Is a Request Answered by `GOSSIP_EXCHANGE`

**Closes:** ISSUE-056 (Low)
**Lives in:** `protocol/chapter3/3.2_topology_gossip/1_shuffle_neighbor_frames.md` (*Two forms of SHUFFLE*); `protocol/chapter3/3.3_churn_recovery/1_recovery_timeline.md` (tier 3, pseudocode); `appendix_d_frame_registry.md` §D.3; `appendix_b_parameters.md`
**Class:** A request with no defined reply (recurring pattern #4)

---

## The problem in one line

Tier 3 of the repair fallback sent an "urgent `SHUFFLE`" and expected a current answer within one RTT; the frame had no priority field, the registry no reply frame, and three implementations could each have chosen a different answer, two of them timing out against the third.

## The decision

Two forms, distinguished by a field the frame already has. `TTL > 0`: the classic random walk, no reply — fresh records return through the periodic `GOSSIP_EXCHANGE`. `TTL = 0` over an existing session: a **direct shuffle request**, answered on the same session within $\tau_{\text{sched}}$ by a `GOSSIP_EXCHANGE` drawn first from the receiver's pools for the `WantedTrees`. The pseudocode's `priority=HIGH` is removed; urgency is the receiver's obligation to answer promptly.

## Why this and not the alternatives

*   **A new `SHUFFLE_REPLY` frame**: `GOSSIP_EXCHANGE` already carries `WantedTrees` and Peer Records over the HMAC-protected session the tier-3 target holds, and nothing distinguishes what a reply would carry.
*   **Reply to walk-mode shuffles too** (HyParView's original design): requires a connection back to an origin known only by NodeID; the protocol's periodic exchange with active neighbours is the return path.

## Defects found during verification

*   At $N \le 12$ the per-tree pools are structurally thin ($|\mathcal{P}| = 0$ at $N = 5$), so tier 3 is the only sub-second source of current candidates; the interoperability gap sat on the path SOLUTION-006 introduced to avoid a 1–3 s DHT walk.

## The generalisable lesson

**Every request needs a named reply and a deadline for it; "urgent" is not a field.**

## Residual risk

None.

## Validation owed (Chapter 8)

*   Tier-3 answer latency at small $N$ against the 1 RTT the tier table assumes.
