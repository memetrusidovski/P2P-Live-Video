# ISSUE-007: Orthogonal Placement Rule Restricts Super Nodes to 1 of 6 Trees

**Status:** Open  
**Priority:** High  
**Component:** Chapter 1 — Multi-Forest Overlay / Stream Slicing  
**Affects:** High-bandwidth nodes (1 Gbps+), dedicated relay servers (10 Gbps)  
**File:** `protocol/chapter1/1.2_multi_forest_deep_dive/1_graph_theory_and_slicing.md`

---

## Summary

The Orthogonal Placement Rule assigns every peer to relay in exactly one tree, determined by a deterministic hash. A node with 10 Gbps upload capacity is locked to one tree and uses all 10 Gbps for that one slice — but the other 5 trees get no benefit from this node. To get full super-node coverage across all 6 trees, 6 separate high-bandwidth servers are needed. A single powerful server cannot serve as a backbone for the entire stream.

---

## Detailed Description

From `1_graph_theory_and_slicing.md`:

> A node v is only allowed to act as an Interior Node in exactly **one** tree Ta. In all other M−1 trees, node v must act as a Leaf Node.

The deterministic assignment:
```
a = (Blake3(NodeID) mod M) + 1
```

**What this means for a 10 Gbps node:**

With B_m = 1 Mbps per slice, `K_v = floor(10000 / 1) = 10,000` upload slots.

The node serves 10,000 children in tree `a` at 1 Mbps each = 10 Gbps used. Correct and efficient for that tree. But the node is a leaf in trees 1, 2, 3, 4, 5 (all except `a`). It downloads from 5 parents but uploads nothing in those trees. 5/6 of the stream's tree coverage gets no help from this server.

**The capacity waste:**

If the goal is to provide CDN-like backbone infrastructure via 6 rented servers:
- Server 1: hash → tree 2. Covers tree 2.
- Server 2: hash → tree 2. Also covers tree 2 (hash collision — birthday problem).
- Server 3: hash → tree 5. Covers tree 5.
- ...

Due to random hash assignment, some trees get multiple super nodes while others get none. With 6 servers, expected unique tree coverage is `6 × (1 - (5/6)^6) ≈ 3.8 trees`. You'd need ~14 servers to have high confidence all 6 trees are covered.

**What a 10 Gbps node could do if the rule were relaxed:**

Relay all 6 slices simultaneously:
```
6 slices × 1666 children × 1 Mbps = 10 Gbps
```

One server could be the backbone for the entire stream at large N, rather than needing 6.

---

## Impact

- Infrastructure operators renting high-bandwidth servers for reliability get 1/6 the expected value per server.
- The hash-based tree assignment makes it impossible to deliberately assign a server to a specific tree — you get what the hash gives you.
- Running 6 super nodes to cover all trees means 6× the cost and operational overhead.

---

## Proposed Fix

**Capacity-proportional multi-tree assignment for high-bandwidth nodes.**

Replace the hard single-tree assignment with a capacity-based assignment that allows nodes with excess bandwidth to relay in multiple trees:

```python
def compute_tree_assignments(node_id, u_v, B_m, M, B):
    # How many full slices can this node carry simultaneously?
    max_trees = min(M, floor(u_v / B))    # B = full stream bitrate (6 Mbps)
    
    if max_trees == 1:
        # Standard node: original single-tree deterministic assignment
        return [(Blake3(node_id) % M) + 1]
    else:
        # Super node: assign to multiple trees, spread by hash
        primary = (Blake3(node_id) % M) + 1
        assignments = [primary]
        for i in range(1, max_trees):
            next_tree = ((primary - 1 + i) % M) + 1   # round-robin from primary
            assignments.append(next_tree)
        return assignments
```

For a 10 Gbps node with B=6 Mbps: `max_trees = floor(10000 / 6000) = 1`. Only 1 tree.

Wait — that's the same result. The issue is that the per-slice capacity K_v = floor(u_v / B_m) = 10,000 but the full stream requires 6 Mbps, not 1 Mbps. If B_m = 1 Mbps and u_v = 10 Gbps, the node can relay 10 Gbps / 6 Mbps = 1666 full-stream-equivalent children, but across all 6 trees.

Revised formula:
```python
max_trees = min(M, floor(u_v / B_m))    # how many trees can carry children in
per_tree_slots = floor(u_v / (max_trees * B_m))  # slots distributed across trees
```

For a 10 Gbps node: `max_trees = min(6, 10000) = 6`. Per-tree slots = `floor(10000 / (6 × 1)) = 1666` children per tree. Total upload = 6 × 1666 × 1 Mbps = 9996 Mbps ≈ 10 Gbps. ✓

This means one 10 Gbps server becomes a Layer-1 relay for all 6 trees, serving 1666 children per tree. A single server now provides full stream backbone coverage.

**Threshold for multi-tree eligibility:**
```
u_v ≥ B × 2    (at least 2× the full stream bitrate)
```

At B=6 Mbps, nodes with ≥12 Mbps upload get 2 trees. Nodes with ≥36 Mbps get 6 trees.

The `Orthogonal Placement Rule` comment about preventing hotspots is satisfied by the per-tree slot limit — the node can't give more than `K_v / max_trees` slots to any single tree child.

---

## Effort

Medium-High. Changes the core tree assignment algorithm, the capacity advertisement format (must advertise tree list, not single tree), and the DHT peer registration (must index peer by all assigned trees). Parent selection algorithm is unchanged — each tree still scores independently.
