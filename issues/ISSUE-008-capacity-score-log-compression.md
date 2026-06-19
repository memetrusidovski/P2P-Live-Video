# ISSUE-008: CapacityScore Log Compression Undersells High-Bandwidth Nodes in Parent Selection

**Status:** Open  
**Priority:** Low  
**Component:** Chapter 1 — Parent Selection Algorithm  
**Affects:** Swarms with mixed bandwidth peers; super node deployments  
**File:** `protocol/chapter1/1.2_multi_forest_deep_dive/2_parent_selection_algorithm.md`

---

## Summary

The parent scoring function uses a natural log to compress capacity into the score, intentionally preventing high-bandwidth nodes from monopolising all child connections. However, the compression is so aggressive that a node with 200× more bandwidth scores only 2.3× higher. In swarms with super nodes, this means peers don't preferentially route to high-bandwidth nodes as strongly as they should, potentially leading to underuse of available backbone capacity.

---

## Detailed Description

From `2_parent_selection_algorithm.md`, with weights `w1=1000.0`:

```
CapacityScore(p) = ln(1 + K_avail) × R
Score(p, i) = (w1 × CapacityScore(p)) - (w2 × RTT) - (w3 × exp(λ × h_p))
```

**Actual computed scores for a new joiner at the same hop depth and RTT:**

| Node type | Upload | K_avail | CapacityScore × w1 | Score advantage vs 10 Mbps node |
|---|---|---|---|---|
| Mobile/slow | 10 Mbps | 10 | ln(11) × 1000 = **2397** | baseline |
| Home cable | 50 Mbps | 50 | ln(51) × 1000 = **3930** | +1.64× |
| 1 Gbps fiber | 1000 Mbps | 1000 | ln(1001) × 1000 = **6910** | +2.88× |
| 10 Gbps server | 10000 Mbps | 10000 | ln(10001) × 1000 = **9210** | +3.84× |

A 10 Gbps server has 1000× more bandwidth than a 10 Mbps node but only a 3.84× score advantage. The RTT penalty (`w2 = 1.0 per ms`) heavily dilutes this: a 10 Gbps server that is 100ms further away (`-100` penalty) drops to a score of 9110, while a 10 Mbps local node at 5ms is 2392. The server still wins, but a 10 Gbps server that is 200ms away (score 9010) vs. a 50 Mbps node at 10ms (score 3920) — the server wins by 2.3× despite 200× bandwidth, which the log was supposed to prevent hoarding but may be too conservative.

**The design intent note:**

> *We use the natural log so that a peer with 100 slots is scored higher than one with 10, but not 10 times higher, encouraging load distribution.*

This is correct reasoning for preventing all peers from piling onto a single super node. But the log is so flat that peers may route to medium-bandwidth nodes when a much better high-bandwidth node is nearby and available — especially once the super node approaches capacity (K_avail drops) and its score drops sharply.

**Near-capacity score collapse:**

A 10 Gbps server that has filled 90% of its slots (K_avail = 1000 remaining):
```
Score = ln(1001) × 1000 = 6910
```

A 50 Mbps node at 50% capacity (K_avail = 25):
```
Score = ln(26) × 1000 = 3258
```

The 10 Gbps server still wins (6910 vs 3258), but the gap has narrowed significantly. This is actually intentional load balancing — but it means a nearly-full super node competes nearly evenly with a much weaker node.

---

## Impact

- Low practical impact at normal scale — super nodes still win the scoring in most scenarios.
- At high load (near-capacity super nodes), peers may route to weaker nodes unnecessarily, bypassing available backbone capacity.
- Does not cause failure, only suboptimal routing efficiency.

---

## Proposed Fix

Replace `ln(1 + K_avail)` with a slightly less aggressive compression that preserves load-distribution intent but better differentiates high-bandwidth nodes:

**Option A: Square-root compression (less flat than log)**

```
CapacityScore(p) = sqrt(K_avail) × R
```

| Node | K_avail | Score × w1 | vs 10 Mbps |
|---|---|---|---|
| 10 Mbps | 10 | sqrt(10) × 1000 = 3162 | baseline |
| 50 Mbps | 50 | sqrt(50) × 1000 = 7071 | +2.24× |
| 1 Gbps | 1000 | sqrt(1000) × 1000 = 31623 | +10× |
| 10 Gbps | 10000 | sqrt(10000) × 1000 = 100000 | +31.6× |

This gives a 31.6× advantage to a 10 Gbps node over a 10 Mbps node — much more representative of the actual bandwidth difference, while still not being the raw 1000× that would cause total monopolisation.

**Option B: Two-tier scoring with a linear bonus above a threshold**

```
if K_avail > 100:
    CapacityScore = ln(1 + K_avail) × R + 0.5 × ln(K_avail / 100) × R
else:
    CapacityScore = ln(1 + K_avail) × R    # unchanged for normal nodes
```

This preserves existing behaviour for normal nodes and adds a gentle linear bonus for super nodes only.

Option A is simpler and more principled. The choice of sqrt vs log is a tuning parameter that should be validated with simulation data.

---

## Effort

Very low. Single formula change. Should be validated via the Chapter 8 simulation framework before deploying.
