# 1. Graph-Theoretic Foundations & Slice Assignment

## 1.1 The Directed Graph Formulation

To rigorously define the Multi-Forest overlay, we model the entire peer-to-peer live streaming swarm as a directed graph $G = (V, E)$, where:
*   $V$ is the set of all active nodes (peers), including the root broadcaster node $S$.
*   $E$ is the set of active transport connections (edges) where data flows from a parent to a child. Let $e = (u, v)$ represent an edge where node $u$ uploads to node $v$.

The core engineering challenge in P2P streaming is that while a node's downstream capacity $d_v$ is generally large (e.g., $1000\text{ Mbps}$), its upstream capacity $u_v$ is often severely constrained (e.g., $10\text{ Mbps}$). If the video bitrate is $B = 6\text{ Mbps}$, a standard tree structure forces a node $v$ to upload the full $6\text{ Mbps}$ to each child. A node with $u_v = 10\text{ Mbps}$ can only support $k = 1$ child, creating an incredibly deep, fragile, and high-latency chain.

## 1.2 Edge-Disjoint Spanning Trees

To solve this, we divide the stream bitrate $B$ into $M$ independent sub-streams (slices), such that the bitrate of each slice is $B_m = \frac{B}{M}$. If $M = 6$, then $B_m = 1\text{ Mbps}$.

We then decompose the global graph $G$ into a **Multi-Forest**, which is a collection of $M$ directed spanning trees:
$$\mathcal{F} = \{T_1, T_2, \dots, T_M\}$$
such that $T_m \subseteq G$ for all $m \in [1, M]$.

Ideally, these trees are **edge-disjoint**, meaning that if an edge $e = (u, v)$ exists in tree $T_a$, it cannot exist in tree $T_b$. This prevents duplicate traffic on any single network socket.

## 1.3 The Orthogonal Placement Rule (Capacity Balancing)

The most critical algorithmic constraint of the Multi-Forest is the **Orthogonal Placement Rule**. It mathematically guarantees that no single node is overwhelmed with upload requests across multiple slices.

Every node $v \in V$ computes its total active upload slots $K_v$ based on its measured physical upload capacity $u_v$ and the slice bitrate $B_m$:
$$K_v = \lfloor \frac{u_v}{B_m} \rfloor$$
*(e.g., if $u_v = 10\text{ Mbps}$ and $B_m = 1\text{ Mbps}$, the node has $K_v = 10$ slots).*

This rule applies to **`RELAY`-class nodes only**. Battery-, data-, or reachability-constrained devices declare themselves leaf-class and bear no relay duty in any tree — see [5. Node Classes and Relay Eligibility](5_node_classes.md), which also corrects the relay density used in capacity calculations to $N_{\text{relay}}/M$ rather than $N/M$.

To ensure fairness, a *standard* node $v$ acts as an **Interior Node** (a relay that uploads data) in exactly **one** tree $T_a \in \mathcal{F}$. In all other $M-1$ trees, node $v$ acts as a **Leaf Node** (a sink that only downloads data).

Let $\text{out-degree}(v, T_m)$ be the number of children node $v$ has in tree $T_m$. The constraint for a standard node is:
$$\exists! \ a \in [1, M] \text{ such that } \text{out-degree}(v, T_a) \le K_v$$
$$\forall b \ne a, \quad \text{out-degree}(v, T_b) = 0$$

### Deterministic Tree Assignment
To decide *which* tree $T_a$ a node should relay for, the node computes a deterministic hash modulo $M$:
$$a = \left( \text{Blake3}(NodeID) \pmod M \right) + 1$$
This guarantees that across a swarm of $N$ nodes, the relay burden is statistically distributed uniformly across all $M$ trees, with approximately $\frac{N}{M}$ relays available per slice.

### Capacity-Proportional Multi-Tree Assignment (Super Nodes)

The single-tree rule exists to stop *ordinary* peers from being overwhelmed — but applied to a high-capacity node it wastes most of that node's value. A 10 Gbps server locked to one tree spends its entire budget on one slice while the other $M-1$ trees get nothing from it; covering all trees with hash-assigned single-tree servers would take ~14 servers (birthday-problem coverage) where one should suffice.

A node whose upload can carry at least two *full-stream-equivalent* loads therefore relays in multiple trees:

$$\text{max\_trees}(v) = \min\left(M,\ \max\left(1, \left\lfloor \frac{u_v}{B} \right\rfloor\right)\right), \qquad \text{per\_tree\_slots}(v) = \left\lfloor \frac{u_v}{\text{max\_trees}(v) \cdot B_m} \right\rfloor$$

Tree assignments spread round-robin from the hash-derived primary tree, preserving deterministic dispersion:

```python
def compute_tree_assignments(node_id, u_v, B_m, M, B):
    max_trees = min(M, max(1, floor(u_v / B)))       # full-stream-equivalents carried
    if max_trees == 1:
        return [(Blake3(node_id) % M) + 1]           # standard single-tree rule
    primary = (Blake3(node_id) % M) + 1
    return [((primary - 1 + i) % M) + 1 for i in range(max_trees)]
```

Worked examples ($B = 6$ Mbps, $B_m = 1$ Mbps, $M = 6$):

| Node upload $u_v$ | max_trees | per_tree_slots | Effect |
| :---: | :---: | :---: | :--- |
| 10 Mbps | 1 | 10 | Unchanged standard behaviour |
| 12 Mbps | 2 | 6 | First multi-tree tier ($u_v \ge 2B$) |
| 36 Mbps | 6 | 6 | Interior in every tree |
| 10 Gbps | 6 | 1666 | One server backbones the whole forest (~1666 children/tree) |

The anti-hotspot intent of the Orthogonal Placement Rule is preserved by the **per-tree slot cap**: a multi-tree node can never devote more than $K_v / \text{max\_trees}$ slots to any single tree. Multi-tree nodes register in the DHT under *each* assigned tree so per-slice `GET_PEERS` discovery finds them, and their capacity advertisements carry the full assigned-tree list rather than a single tree ID.

## 1.3.1 Dynamic Forest Sizing

$M$ is **not a fixed constant** — it scales with the current swarm size $N$ up to the ceiling $M_{\text{max}} = 6$ (Appendix B). A fixed $M = 6$ at small $N$ would leave most trees without any hash-assigned relay until $N \approx 18$ (a birthday-problem coverage effect), forcing the source to push all six slices directly to every viewer — full CDN load exactly when the streamer's upload budget matters most.

The source monitors the swarm size (from its DHT stream record, Chapter 2.3) and publishes the current forest size on the following ladder:

| Swarm size $N$ | $M$ | Slice bitrate $B_m$ (at $B = 6$ Mbps) |
| :---: | :---: | :---: |
| $N < 6$ | 1 | 6 Mbps (no splitting overhead) |
| $6 \le N < 12$ | 2 | 3 Mbps |
| $12 \le N < 18$ | 3 | 2 Mbps |
| $18 \le N < 24$ | 4 | 1.5 Mbps |
| $24 \le N < 30$ | 5 | 1.2 Mbps |
| $N \ge 30$ | 6 | 1 Mbps (full protocol) |

At $N = 2$, $M = 1$ and the source pushes the full stream to its single viewer — well within a home upload budget. At $N \ge 30$ the full forest is active and the swarm is self-sustaining. The current $M$ is carried in the stream manifest's `num_trees` field; transitions are announced via the `MANIFEST_UPDATE` frame with a 5-second migration window (see [4. Stream Slicing Architecture](4_stream_slicing_architecture.md), §4.5).

## 1.4 Visualizing the 3-Node, 3-Slice Graph

Consider a swarm with $M = 3$ slices. We have the Root Source ($S$) and three Peers ($P_1, P_2, P_3$). 
By the deterministic hash rule:
*   $P_1$ is assigned to relay **Tree 1** (Slice 1).
*   $P_2$ is assigned to relay **Tree 2** (Slice 2).
*   $P_3$ is assigned to relay **Tree 3** (Slice 3).

```text
       TREE 1 (Slice 1)           TREE 2 (Slice 2)           TREE 3 (Slice 3)
       ----------------           ----------------           ----------------
         [ Source ]                 [ Source ]                 [ Source ]
             |                          |                          |
       (Uploads S1)               (Uploads S2)               (Uploads S3)
             |                          |                          |
             v                          v                          v
        [ PEER 1 ]                 [ PEER 2 ]                 [ PEER 3 ] 
       /          \               /          \               /          \
  (Relays S1) (Relays S1)    (Relays S2) (Relays S2)    (Relays S3) (Relays S3)
     /              \           /              \           /              \
    v                v         v                v         v                v
[PEER 2]          [PEER 3] [PEER 1]          [PEER 3] [PEER 1]          [PEER 2]
(Leaf)            (Leaf)   (Leaf)            (Leaf)   (Leaf)            (Leaf)
```

### Analysis of the Graph:
*   **Total Data Downloaded by $P_1$:** Slice 1 (from Source) + Slice 2 (from $P_2$) + Slice 3 (from $P_3$) = $3 \times B_m = B$. It receives the full video stream.
*   **Total Data Uploaded by $P_1$:** It only uploads Slice 1 to $P_2$ and $P_3$. Upload volume = $2 \times B_m$.
*   By strictly isolating relay duties, we prevent $P_1$ from having to upload all 3 slices to both peers (which would require $6 \times B_m$ capacity).
