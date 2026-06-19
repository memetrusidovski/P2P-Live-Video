# ISSUE-005: New Relay Nodes Have No Data to Relay for ~1 Second After Joining

**Status:** Open  
**Priority:** Low  
**Component:** Chapter 1 — Multi-Forest Overlay / Chapter 4 — Media Distribution  
**Affects:** Rapidly growing streams; peers who try to join under a freshly joined relay  
**File:** `protocol/chapter1/1.2_multi_forest_deep_dive/2_parent_selection_algorithm.md`, `protocol/chapter4/4.1_segment_serialization/1_gop_serialization.md`

---

## Summary

When a peer joins the swarm and is assigned as a relay for tree `a`, it must first receive and buffer at least one complete GOP (1 second of video, ~750 KB at 6 Mbps) before it can relay anything downstream. During this 1-second warm-up window, any peer that selects it as a parent receives no data and immediately triggers CHURN_REPAIR. During rapid growth phases this warm-up delay compounds across multiple new relays simultaneously.

---

## Detailed Description

Segment constraints from `1_gop_serialization.md`:

> Each 1.0-second segment is serialized into a binary payload. For a 6 Mbps video stream, the average segment size is 750 KB.

A relay cannot begin forwarding a segment until it has received enough blocks of that segment to know which it holds (for bitfield advertising) and to push them downstream. The 16 KB block size means a full segment is ~47 blocks. Until the relay has received the first block of a segment, its bitfield for that segment is empty and it advertises zero availability.

**The sequence of events when viewer A joins under a fresh relay R:**

```
T=0ms   : Relay R joins swarm. Active Set populated. Tree join handshake complete.
T=0ms   : R advertises K_avail upload slots. Parent scoring selects R as best candidate.
T=0ms   : Viewer A joins under R via QUIC tree join handshake. Gets ACCEPTED.
T=0–1000ms : R receives its first segment from its own parent. Buffering.
T=0–1000ms : A receives NO video from R. R has empty bitfield.
T=100ms : A sends PING_PROBE to R (no video received).
T=200ms : A evicts R (no PONG — R is alive but has no data to push).
```

Wait — R will respond to PING with PONG (it's alive), so A won't evict it via the 200ms dead-parent timeout. But A will see zero data arriving during the 1-second warm-up. The buffer scheduler will see missing blocks and attempt PULL from gossip peers. Those peers may also be new relays with empty buffers.

In a burst join scenario where 10 peers join simultaneously and 2–3 of them are new relays, the PULL layer absorbs the missing data (falling back to the source or other tree branches via mesh pull). This works but creates measurable latency spikes and increased PULL traffic.

The deeper problem: R advertises `K_avail` slots as soon as its tree join is accepted, before it has any data. Peers selecting parents see R's high capacity score and preferentially route to it, only to receive nothing for 1 second.

---

## Impact

- During rapid growth phases (stream launch, viral sharing), multiple simultaneous new relays all have warm-up windows. The mesh PULL layer absorbs the gaps but at the cost of latency spikes.
- Low severity in isolation (1 second, covered by the 4-second buffer). Compounds at small N when there are few peers to PULL from.

---

## Proposed Fix

**Delay slot advertisement until the relay has buffered its first complete segment.**

The relay should not advertise `K_avail > 0` in its capacity gossip until it has received and verified at least one full segment (all blocks, Merkle root validated). Until then, it advertises `K_avail = 0`, which causes the parent scoring function to return `CapacityScore = ln(1) × R = 0`, effectively hiding the relay from parent selection until it is ready.

```python
# In parent capacity gossip advertisement:
if len(verified_segment_buffer) == 0:
    advertise_K_avail = 0   # warm-up: hide from parent selection
else:
    advertise_K_avail = floor(u_v / B_m)  # normal advertisement
```

This adds at most 1 second of delay before a new relay appears as a valid parent candidate, which is preferable to peers joining under it and immediately triggering fallback behaviour.

---

## Effort

Very low. Single conditional change in the capacity advertisement logic. No wire format changes.
