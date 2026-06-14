# 3. Topology Healing: The Sub-250ms Sibling Election Protocol

## 3.1 The Vulnerability of Trees
The primary flaw of any tree-based distribution network is its fragility. Consider a simple branch:
`Source -> Node A -> Node B -> Node C`
If `Node A` abruptly disconnects (power failure, closed app), `Node B` and `Node C` are immediately starved of video data.

In a traditional P2P system, `Node B` would realize `A` is dead, query the DHT for a new parent, probe them, and connect. This process takes 1 to 3 seconds. For a live stream with a tight 3-second playout deadline, this delay results in undeniable video buffering.

## 3.2 The Sibling Election Solution
To achieve sub-250ms healing, we completely bypass the DHT and global routing. Instead, we use **Localized Deterministic Sibling Election**. 

Because `Node A` was an active relay, it maintained an Active Set of its children (e.g., `Node B`, `Node C`, and `Node D`). Through the HyParView gossip protocol (Chapter 3), these children are aware of each other—they exist in each other's Passive Sets.

When `Node A` fails, the children do not look upward for a new parent; they look *laterally* at each other. They elect one sibling to take `Node A`'s place.

## 3.3 The Millisecond Execution Timeline

```text
[ T = 0ms ] Node A (Parent) physically loses internet connection.
```

### Phase 1: Detection (T = 100ms - 200ms)
*   `T = 100ms`: Nodes B, C, and D realize they haven't received a video packet or keepalive ping from `Node A`. They all send an active `PING_PROBE` to `A`.
*   `T = 200ms`: No `PONG` is received. `Node A` is officially declared dead by all children simultaneously.

### Phase 2: Deterministic Election (T = 205ms)
*   Nodes B, C, and D need a new parent. Instead of fighting, they execute a deterministic election algorithm based on their locally known scores.
*   Every node broadcasts its local Capacity Score $K_v$ via gossip. 
*   Suppose the scores are: `B = 80`, `C = 150`, `D = 20`.
*   Because `C` has the highest score, **Node C is mathematically elected as the Deputy**. Since all nodes have the same data, they reach this consensus instantly without exchanging extra packets.

### Phase 3: Re-attachment (T = 210ms - 250ms)
*   **Node C's Action:** Node C immediately realizes it is the Deputy. It queries its Passive Set for higher-layer nodes, finds one, and requests promotion to become a Relay.
*   **Node B and D's Action:** Nodes B and D immediately send `RELAY_JOIN_REQUEST` packets directly to Node C.
*   `T = 250ms`: Node C accepts B and D as children. The tree branch is restored.

## 3.4 Visualizing the Healing Sequence

**Before Failure (T < 0ms)**
```text
           [ Source (Layer 0) ]
                    |
           [ Node A (Layer 1) ]  <-- (Fails!)
           /        |         \
        [B]        [C]        [D]  <-- Layer 2 (Siblings)
```

**Election & Healing (T = 250ms)**
```text
           [ Source (Layer 0) ]
                    |
           [ Node C (Layer 1) ]  <-- (Elected Deputy, migrates up!)
           /                  \
        [B]                  [D]  <-- Layer 2 (Re-attached to C)
```

## 3.5 Fallback to Swarm (PULL)
What happens to the video chunks that were supposed to be delivered during that 250ms gap? 
This is where the **Hybrid Push-Pull** mechanism (Chapter 4) saves the stream. 
1. The new tree structure ensures future chunks (from T=250ms onward) are correctly pushed.
2. For the chunks missed between 0ms and 250ms, Nodes B, C, and D revert to Swarm mode. They query their Passive Set neighbors (who belong to different tree branches and were unaffected by A's death) using Bitfields, and PULL the missing packets via RaptorQ FEC symbols before the video decoder starves.
