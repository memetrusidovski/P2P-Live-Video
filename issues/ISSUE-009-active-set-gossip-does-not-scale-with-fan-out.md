# ISSUE-009: Active Set Gossip Cap (c_a=8) Does Not Scale With Tree Fan-Out

**Status:** Open  
**Priority:** Medium  
**Component:** Chapter 3 — HyParView Gossip / Chapter 4 — Media Distribution  
**Affects:** Super nodes with large tree fan-out (K_v >> c_a)  
**File:** `protocol/chapter3/3.1_neighbor_sets/1_memory_structures.md`, `protocol/chapter4/4.3_hybrid_push_pull/2_bitfield_frames.md`

---

## Summary

The HyParView Active Set is capped at `c_a = 8` peers (Appendix B). This governs gossip — exchange of bitfield availability maps, membership state, and control metadata. A super node acting as relay for 10,000 tree children can only maintain open gossip connections with 8 of them. The remaining 9,992 tree children cannot receive direct bitfield state from their relay parent, degrading their ability to issue accurate PULL requests for missing blocks.

---

## Detailed Description

The protocol separates two connection planes:

1. **Tree relay connections (data plane):** Parent pushes slice data unconditionally to all K_v children. One-directional, no TFT, no gossip. A 10 Gbps relay pushes to 10,000 children fine — this scales with bandwidth, not active set size.

2. **Active Set connections (control plane):** HyParView gossip, bitfield exchange, membership updates. Capped at `c_a = 8`. TFT runs on this layer.

**The problem: bitfield state propagation**

From `2_bitfield_frames.md`, peers exchange buffer state bitfields to enable PULL requests for missing blocks. When a child misses a block (UDP packet drop), it:
1. Checks its Active Set peers' bitfields to find who has the missing block.
2. Sends a PULL request to the best candidate.

If a tree child is NOT in its relay parent's Active Set of 8, the child cannot directly query the parent's bitfield. The child must find the missing block via its own 8 gossip peers — who may be on different tree branches and may not have received that block at all.

**At normal scale (N=50, K_v=10):** Each relay's 10 tree children can likely all fit within the relay's Active Set of 8, plus the relay has 8 gossip connections. Manageable.

**At super node scale (K_v=10,000):** The relay's Active Set of 8 covers 0.08% of its tree children. The 99.92% of children who are not in the relay's Active Set must find missing blocks via their own gossip peers — a multi-hop search that takes longer and may fail if the block is rare.

**Concrete scenario:**

Super node S at Layer 1 pushes to 10,000 children. A UDP queue burst drops 5% of blocks for 500 children simultaneously. Those 500 children need to PULL the missing blocks. None of them are in S's Active Set. They query their own 8 gossip peers — most of whom are also tree children of S with the same missing blocks. The PULL requests propagate outward searching for an uncorrelated peer who received the blocks via a different tree branch, adding 2–4 gossip hops and 80–160ms of additional delay.

The FEC RaptorQ parity (10% overhead) absorbs a 5% drop rate locally, so this is not catastrophic. But the bitfield propagation inefficiency means PULL requests are slower and less accurate for large-fan-out nodes.

---

## Impact

- At N < 1000 where K_v is modest, this is a non-issue.
- At large N with super nodes (K_v > 100), PULL request accuracy degrades.
- FEC absorbs most packet loss, so this manifests as slightly higher PULL latency rather than playback failure.

---

## Proposed Fix

**Introduce a relay-broadcast channel separate from HyParView gossip.**

Super nodes (nodes where K_v > c_a × 2) maintain a lightweight one-to-many broadcast channel to all tree children for bitfield state:

```
relay_broadcast_bitfield_interval = 100ms   (same as τ_gossip but broadcast, not peer-to-peer)
```

This is a UDP multicast or targeted unicast burst to all K_v children, carrying only the bitfield (which block indices the relay currently holds). It does not carry membership state or TFT state — those remain on the Active Set gossip channel.

Children receiving the relay broadcast bitfield can issue accurate PULL requests directly to the relay without needing it to be in their Active Set.

**Alternative (lower effort): Scale c_a for relay nodes.**

```python
if is_relay_node:
    c_a_effective = min(64, max(8, floor(K_v / 10)))
else:
    c_a_effective = 8    # unchanged for standard nodes
```

For K_v=10,000: c_a_effective = min(64, 1000) = 64. This keeps 64 tree children in the gossip layer — still only 0.64% but 8× better than the baseline. A practical compromise with no protocol wire format changes.

The relay broadcast approach is architecturally correct but adds protocol complexity. The scaled c_a is the better starting point.

---

## Effort

Low (scaled c_a approach). Medium (relay broadcast channel — requires new frame type).
