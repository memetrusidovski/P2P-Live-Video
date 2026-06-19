# ISSUE-001: JOINING State Machine Hard Blocks Streams With Fewer Than 5 Peers

**Status:** Open  
**Priority:** Critical  
**Component:** Chapter 1 — Peer Lifecycle / State Machine  
**Affects:** All streams at N < 5  
**File:** `protocol/chapter1/1.3_peer_lifecycle/1_transition_model.md`, `2_algorithmic_core_loop.md`

---

## Summary

The peer state machine has a hardcoded transition condition requiring `Active Set size ≥ 4` before a peer can advance from `JOINING (0x02)` to `CONNECTING (0x03)`. At N < 5 total peers (streamer + fewer than 4 viewers), no viewer can ever accumulate 4 active peers, so every viewer is permanently stuck in JOINING and never receives a single video frame. This is a complete stream failure, not a degradation.

---

## Detailed Description

The state machine definition in `1_transition_model.md` and the core loop in `2_algorithmic_core_loop.md` both specify:

```
JOINING (0x02) → CONNECTING (0x03)
Trigger: Active Set size ≥ TargetActiveSize (= 4)
```

The `TargetActiveSize` is derived from `c_a = 8` (Appendix B), with the transition firing at half that value (4). This value is hardcoded — there is no adaptive fallback.

**What happens at small N:**

| Total Peers (streamer + viewers) | Max achievable Active Set size | Advances past JOINING? |
|---|---|---|
| 2 | 1 | No — blocked |
| 3 | 2 | No — blocked |
| 4 | 3 | No — blocked |
| 5 | 4 | Yes — barely passes |
| 6+ | 4+ | Yes |

The HyParView `FORWARD_JOIN` random walk (TTL_active ≈ 4) propagates the join request through the swarm to build up the active set. At N=2, there is only 1 other peer to propagate to — the walk terminates immediately with a set of size 1.

This means every stream at launch (N=2 when the first viewer joins) is completely broken by the state machine before any other component is even reached.

---

## Impact

- **N=2, 3, 4:** 100% failure. No viewer can receive any video.
- **N=5:** Barely passes, but any single peer drop causes a viewer to fall back below the threshold and get stuck again.
- Every stream starts at N=2. This is a universal cold-start failure affecting 100% of streams.

---

## Proposed Fix

Make the JOINING threshold adaptive to the current known swarm size:

```python
# In 2_algorithmic_core_loop.md — JOINING case:

case JOINING:
    TriggerHyParViewHandshakes()
    swarm_size = QueryDHT_KnownPeerCount(StreamID)
    joining_threshold = max(1, min(4, swarm_size - 1))
    If Size(ActiveSet) >= joining_threshold then:
        State <- CONNECTING
```

The formula `max(1, min(4, swarm_size - 1))` means:
- N=2 → threshold = 1 (only 1 other peer exists, pass immediately)
- N=3 → threshold = 2
- N=4 → threshold = 3
- N≥5 → threshold = 4 (normal behaviour resumes)

The DHT peer count (`QueryDHT_KnownPeerCount`) is already fetched during DISCOVERY, so this adds no new network round-trips — just use the count returned from the stream registration lookup.

---

## Effort

Small. Single conditional change in the core loop. No protocol wire format changes needed.
