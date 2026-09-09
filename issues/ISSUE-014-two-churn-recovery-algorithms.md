# ISSUE-014: Two Competing Churn-Recovery Algorithms for the Same 250 ms Budget

**Status:** Resolved  
**Priority:** High  
**Component:** Ch1 §1.2.3 (Topology Healing) / Ch3 §3.3 (Churn Recovery)  
**Affects:** Every parent-failure event; implementation divergence  
**File:** `protocol/chapter1/1.2_multi_forest_overlays/3_topology_healing.md`, `protocol/chapter3/3.3_churn_recovery/`

---

## Summary

Ch1.2.3 heals a broken tree branch by **deterministic lateral sibling election** (orphans elect the highest-capacity sibling as "Deputy", who re-attaches the subtree; DHT bypassed). Ch3.3 and Appendix A.2 heal the same failure by **independent passive-set promotion** (each orphan promotes its own best standby within 250 ms). The two produce different topologies, are never cross-referenced, and the sibling election is underspecified: it assumes all siblings hold identical score data and reach consensus "instantly", has no tie-break, no Deputy-refusal path, and no capacity check.

## Proposed Fix

Unify into one two-stage procedure with **passive-set promotion (ch3.3) as the canonical primitive** and sibling election retained as a coordination layer that reduces to it:

1. **Child roster:** parents distribute a sequence-numbered roster (child NodeIDs, advertised K_v, node class) to all children, piggybacked on the 1 s gossip cycle (`τ_roster = 1000 ms`).
2. **Deterministic election:** on parent death (T = 200 ms), each orphan holding a fresh roster (< 5 s) locally computes the Deputy: max K_v, ties broken by lowest NodeID (byte-lexicographic). Same signed snapshot ⇒ same result, no messages needed.
3. **Deputy runs ch3.3** passive-set promotion to find *its* new parent — the election is coordinator-selection over ch3.3, not a rival algorithm.
4. **Non-deputies** send `RELAY_JOIN_REQUEST` ≡ `NEIGHBOR(Priority=HIGH, TreeID=m)` to the Deputy and arm a **45 ms fallback timer** (`τ_deputy`); no `ACCEPTED` by T = 250 ms → independent ch3.3 promotion.
5. No roster / stale roster / leaf-only self → skip election, straight to ch3.3. Leaf-class nodes are marked in the roster and never electable.

---

## Resolution

Unified per the proposed fix — passive-set promotion is the canonical primitive; sibling election is a coordination layer over it:

- `protocol/chapter1/1.2_multi_forest_overlays/3_topology_healing.md` — §3.2 rewritten: child-roster mechanism (sequence-numbered `(NodeID, K_v, NodeClass)` list piggybacked on the 1 s gossip cycle; stale after 5 s) as the shared election input; §3.3 Phase 2 gains the deterministic tie-break (max K_v, then lowest NodeID) and leaf-class exclusion; Phase 3 states the Deputy runs Ch3 §3.3 to find its own parent, non-deputies send RELAY_JOIN_REQUEST ≡ NEIGHBOR(HIGH, TreeID) with a 45 ms fallback timer, and every failure path (no roster / stale / Deputy unresponsive / leaf-only self) degrades to independent Ch3 §3.3 promotion.
- `protocol/chapter3/3.3_churn_recovery/1_recovery_timeline.md` — new "Relationship to Tree Healing" section declaring this timeline the canonical primitive and the election a multi-orphan coordination layer.
- `protocol/appendix_b_parameters.md` — added τ_roster = 1000 ms (stale 5 s) and τ_deputy = 45 ms.
