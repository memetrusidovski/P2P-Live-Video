# ISSUE-006: Empty Passive Set During Early Growth Makes Churn Recovery Slow

**Status:** Open  
**Priority:** Medium  
**Component:** Chapter 3 — HyParView Overlay Membership  
**Affects:** Streams at N < ~35 peers  
**File:** `protocol/chapter3/3.1_neighbor_sets/1_memory_structures.md`, `protocol/chapter3/3.3_churn_recovery/1_recovery_timeline.md`

---

## Summary

The sub-250ms churn recovery system relies on a Passive Set of 32 standby peer addresses (`c_p = 32`, Appendix B) to immediately reconnect when a parent drops. At N < ~35 total peers, the passive set cannot fill to target size. When a viewer drops at small N, the CHURN_REPAIR state finds an empty passive set and must fall all the way back to DHT re-lookup, which takes 1–3 seconds instead of 250ms — long enough to cause visible buffering.

---

## Detailed Description

From `1_memory_structures.md`:

> **Passive Set (P):** A larger pool of inactive neighbor addresses (size `c_p ≈ 32`) stored as a standby backup. No sockets are open to these nodes; they are cached as candidate standbys.

From `1_recovery_timeline.md`:

> **T = 210ms:** Peer i queries its local Passive Set P, selects the standby peer with the highest historical contribution score, and sends an urgent NEIGHBOR promotion request.
> **T = 250ms:** Standby peer accepts the handshake. Sub-stream transmission resumes.

This 250ms recovery only works if `|P| > 0`. With N total peers, the maximum passive set size is `N - 1 - |A|`. Given `c_a = 8`:

| Total Peers (N) | Max Passive Set Size | 250ms Recovery Possible? |
|---|---|---|
| 5 | 5 - 1 - 4 = 0 | No — falls to DHT (1–3s) |
| 10 | 10 - 1 - 8 = 1 | Barely (1 candidate) |
| 20 | 20 - 1 - 8 = 11 | Partial |
| 35 | 35 - 1 - 8 = 26 | Near-target |
| 41+ | 32 (capped at c_p) | Full — works as designed |

The FORWARD_JOIN propagation (`2_random_walk_propagation.md`) adds peers to passive sets as the walk traverses the swarm. With `TTL_active ≈ 4` hops and N=10 peers, the walk exhausts the swarm in 2–3 hops with very few unique nodes encountered. The passive set stays near-empty.

**Failure scenario at N=8:**

```
T=0ms   : Viewer A loses parent P (power failure).
T=210ms : A queries Passive Set → empty.
T=210ms : A falls back to CHURN_REPAIR → DISCOVERY path.
T=210ms : A re-queries DHT for active peers.
T=1200ms: DHT lookup completes. New parent found.
T=1400ms: QUIC handshake to new parent. Stream resumes.
```

The viewer experiences ~1.4 seconds of buffering for a churn event that at large N resolves in 250ms.

---

## Impact

- Small streams experience CDN-level churn recovery times (1–3s) rather than the 250ms the protocol promises.
- At N=5, even a single viewer disconnect can leave another viewer with an empty passive set and no fast recovery path.
- This compounds with ISSUE-001 (JOINING threshold) — after recovering, the viewer may need to re-run the full JOINING state machine again.

---

## Proposed Fix

**Option A: DHT-backed emergency peer list as passive set fallback.**

When `|P| < minimum_useful_size (= 3)`, the CHURN_REPAIR path should skip the 250ms passive set lookup and go directly to a fast DHT peer list query — but using a cached result from the last DISCOVERY phase rather than a fresh lookup, cutting the time from 1–3s to ~200ms.

```python
# In CHURN_REPAIR state:
if Size(PassiveSet) < 3:
    # Use cached DHT peer list from last DISCOVERY (already in memory)
    candidates = GetCachedDHTPeers(StreamID)
else:
    candidates = GetTopScoredPeers(PassiveSet)
SendNeighborRequest(best_candidate(candidates))
```

**Option B: Source/relay nodes maintain an extended peer list for redistribution.**

The broadcaster and any relay nodes above Layer 2 maintain a full list of all active peers (they're at the top of the tree, they see all joins). When a viewer in CHURN_REPAIR can't find passive set candidates, it queries its current active parent for a peer list — a single round-trip that costs ~RTT ms instead of a full DHT walk.

Option A is lower effort. Option B is more architecturally correct.

---

## Effort

Low (Option A). The cached DHT peer list is already in memory from DISCOVERY — this is a routing change, not a data collection change.
