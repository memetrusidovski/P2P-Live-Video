# Chapter 1: Protocol Foundations and Mathematical Architecture

### 1.1 Scale and Latency Bounds of Million-Peer Swarms

Let $N$ represent the number of concurrent viewers in a live streaming swarm, and $B$ represent the playback bitrate of the video stream. In a traditional centralized CDN, the origin or edge servers must support a total egress bandwidth of:
$$W_{\text{centralized}} = N \cdot B$$

In our decentralized model, we assume each peer $i$ has a maximum upload capacity $u_i$ and a download capacity $d_i$, where $d_i \ge B$ for all participating nodes. The system is sustainable if and only if the aggregate upload capacity exceeds the aggregate demand:
$$\sum_{i=1}^{N} u_i \ge N \cdot B$$

For live video streaming, the propagation delay (latency) of a chunk from the source (Layer 0) to the deepest leaf node must be tightly bounded. If $D$ represents the maximum overlay hop diameter and $RTT_{\text{avg}}$ is the average round-trip time between peers, the propagation latency $L$ is bounded by:
$$L \approx D \cdot \left( \frac{\text{ChunkSize}}{B} + RTT_{\text{avg}} \right)$$

To support $N = 1,000,000$ concurrent users with an end-to-end latency budget $L \le 5$ seconds, we must restrict $D$ to a logarithmic function of the swarm size:
$$D \le c \log_k(N)$$
where $k$ is the average degree (out-degree) of each relay node, and $c$ is a topology coefficient.

```text
                                [ Source (Layer 0) ]
                                 /      |      \
                            [Peer A] [Peer B] [Peer C]   <-- Layer 1 (Hop 1)
                            /   |      |   \      \
                         [...] [...]  [...] [...] [...]  <-- Layer 2 (Hop 2)
```

### 1.2 Mathematical Formulation of Dynamic Multi-Forest Overlays

To optimize latency, the swarm does not form a single, monolithic distribution tree. Instead, the video stream is split into $M$ independent sub-streams (slices). The overlay network is structured as a **Multi-Forest** consisting of $M$ independent, self-healing distribution trees:
$$\mathcal{F} = \{T_1, T_2, \dots, T_M\}$$

For any tree $T_m \in \mathcal{F}$:
1.  The broadcaster is the root (Layer 0).
2.  A peer $i$ occupies a specific depth layer $d_i(m)$ in tree $T_m$.
3.  **Orthogonal Placement Rule:** To maximize contribution, a peer $i$ that occupies an interior node position (high-tier relay) in tree $T_a$ must be assigned a leaf node position (consumer) in the remaining trees $T_b$ ($b \ne a$):
    $$\forall i, \sum_{m=1}^{M} \mathbb{I}(\text{is\_interior}(i, T_m)) \le 1$$

This ensures that the upload burden of forwarding the full stream bitrate $B$ is evenly distributed across different subsets of peers, avoiding single-peer network card saturation.

### 1.3 State-Machine Representation of Peer Life Cycle

Each node in the network transitions through a well-defined state machine to preserve overlay stability:

```text
   [ Bootstrapping ] ---> [ Discovery (DHT) ] ---> [ Gossip Joining ]
                                                           |
                                                           v
   [ Active Mesh ] <--- [ Optimization ] <--- [ Parent Connection ]
         |
         +---> [ Churn Repair (Timeout) ] ---> [ Fallback Relay ]
         |
         +---> [ Terminated (Graceful/Abrupt) ]
```

*   **Bootstrapping:** Initiating S/Kademlia cryptographic ID generation and loading seed nodes.
*   **Discovery (DHT):** Querying the DHT to find stream publishers.
*   **Gossip Joining:** Exchanging peer lists using HyParView protocols to populate connection buckets.
*   **Parent Connection:** Establishing $k$ parent relationships for incoming sub-streams.
*   **Active Mesh:** Actively downloading, verifying, and uploading video symbols.
*   **Optimization:** Continuously measuring neighbor RTTs and moving closer to the source if capacity scores increase.
*   **Churn Repair:** Triggered when keepalives fail; immediately promotes candidate peers to avoid buffer depletion.
