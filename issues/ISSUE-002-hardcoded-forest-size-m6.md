# ISSUE-002: Hardcoded M=6 Forest Size Forces Source to Bear Full CDN Load During Cold-Start

**Status:** Open  
**Priority:** High  
**Component:** Chapter 1 — Multi-Forest Overlay / Stream Slicing  
**Affects:** All streams at N < ~18 peers  
**File:** `protocol/chapter1/1.2_multi_forest_deep_dive/1_graph_theory_and_slicing.md`, `4_stream_slicing_architecture.md`, `protocol/appendix_b_parameters.md`

---

## Summary

The number of distribution trees M is hardcoded at 6 (Appendix B). The Orthogonal Placement Rule assigns each peer to relay exactly one tree via a deterministic hash. This means all 6 trees need relay coverage before the swarm starts meaningfully offloading bandwidth from the source. Due to the birthday problem distribution of hash assignments, approximately 12–18 viewers are needed before all 6 trees statistically have relay coverage. Until then, the source must directly push all 6 slices to every viewer — functioning as a CDN, not a P2P swarm.

---

## Detailed Description

Each peer is assigned to relay exactly one tree:

```
a = (Blake3(NodeID) mod M) + 1
```

With M=6 and B=6 Mbps, each slice is B_m = 1 Mbps. Until tree `a` has a relay, the source pushes that slice directly to all viewers. The source's total upload is:

```
source_upload = (trees_without_relay / M) × B × N_viewers
```

**Source upload load at various N:**

| Viewers (N) | Expected trees with ≥1 relay | Source upload (50% coverage scenario) |
|---|---|---|
| 2 | 1–2 of 6 | ~5 Mbps × 2 = 10 Mbps |
| 6 | 3–4 of 6 (birthday prob.) | ~3 Mbps × 6 = 18 Mbps |
| 12 | ~5 of 6 | ~1 Mbps × 12 = 12 Mbps |
| 18 | ~6 of 6 | P2P relief begins |

A streamer with a typical 50 Mbps upload can only directly serve ~8 viewers at 6 Mbps. With M=6 hardcoded, the swarm provides almost no relief until N≈18, meaning streamers with moderate upload bandwidth hit a wall before the P2P mechanics help.

Additionally, when M=6 is applied at N=5, five of the six trees have either 0 or 1 relay node. A single relay dropout causes that tree to fully collapse to source-direct delivery.

---

## Impact

- Streamers with upload < 50 Mbps face stream degradation or failure at moderate early viewer counts.
- The P2P bandwidth relief the protocol promises is delayed until N≈18.
- Small streams (N < 20) permanently operate in a degraded CDN-like mode despite being "P2P."

---

## Proposed Fix

Make M dynamic, scaling with swarm size. The source monitors the DHT peer count and adjusts M at defined thresholds:

```
M = 1  when  N < 6     (source pushes full stream; no splitting overhead)
M = 2  when  6 ≤ N < 12
M = 3  when  12 ≤ N < 18
M = 4  when  18 ≤ N < 24
M = 5  when  24 ≤ N < 30
M = 6  when  N ≥ 30    (full protocol, all trees active)
```

The current M value is published in the stream manifest (already has a `num_trees` field per `4_stream_slicing_architecture.md`). When M increases:

1. Source broadcasts a `MANIFEST_UPDATE` frame with the new M value.
2. Peers have a 5-second migration window to re-compute their tree assignment (`Blake3(NodeID) mod M_new + 1`) and rejoin the newly created trees.
3. Old trees remain active during the window. Once the new trees are seeded (relay nodes advertising capacity), old trees drain and close.

This means at N=2, M=1 and the source pushes the full 6 Mbps to 1 viewer — well within a home upload budget. At N=30, the full M=6 forest is active and the swarm is self-sustaining.

---

## Effort

Medium. Requires manifest versioning, a `MANIFEST_UPDATE` frame type, and client-side re-join logic at M transitions. The core tree join algorithm is unchanged — only the input M varies.
