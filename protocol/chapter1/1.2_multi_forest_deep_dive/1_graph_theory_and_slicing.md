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

To ensure fairness, a node $v$ is only allowed to act as an **Interior Node** (a relay that uploads data) in exactly **one** tree $T_a \in \mathcal{F}$. In all other $M-1$ trees, node $v$ must act as a **Leaf Node** (a sink that only downloads data).

Let $\text{out-degree}(v, T_m)$ be the number of children node $v$ has in tree $T_m$. The constraint is:
$$\exists! \ a \in [1, M] \text{ such that } \text{out-degree}(v, T_a) \le K_v$$
$$\forall b \ne a, \quad \text{out-degree}(v, T_b) = 0$$

### Deterministic Tree Assignment
To decide *which* tree $T_a$ a node should relay for, the node computes a deterministic hash modulo $M$:
$$a = \left( \text{Blake3}(NodeID) \pmod M \right) + 1$$
This guarantees that across a swarm of $N$ nodes, the relay burden is perfectly distributed uniformly across all $M$ trees, with exactly $\frac{N}{M}$ relays available per slice.

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
