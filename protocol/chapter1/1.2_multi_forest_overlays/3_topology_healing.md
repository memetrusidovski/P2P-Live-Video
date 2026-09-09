# 3. Topology Healing: The Sub-250ms Sibling Election Protocol

## 3.1 The Vulnerability of Trees
The primary flaw of any tree-based distribution network is its fragility. Consider a simple branch:
`Source -> Node A -> Node B -> Node C`
If `Node A` abruptly disconnects (power failure, closed app), `Node B` and `Node C` are immediately starved of video data.

In a traditional P2P system, `Node B` would realize `A` is dead, query the DHT for a new parent, probe them, and connect. This process takes 1 to 3 seconds. For a live stream with a tight 3-second playout deadline, this delay results in undeniable video buffering.

## 3.2 The Sibling Election Solution
To achieve sub-250ms healing, we avoid a fresh DHT walk entirely. Instead, we use **Localized Deterministic Sibling Election**, built as a *coordination layer on top of* the canonical churn-recovery primitive — HyParView passive-set promotion (Chapter 3 §3.3). The election decides **who** re-attaches the subtree; the actual re-attachment mechanism is always Ch3 §3.3, and every failure path degrades gracefully to it.

### The Child Roster (Election Input)
Deterministic election requires that all siblings compute from the *same data*. To guarantee this, every relay parent distributes a **child roster** to all of its children, piggybacked on the 1-second gossip cycle ($\tau_{\text{roster}} = 1000\text{ ms}$, Appendix B): a sequence-numbered list of `(Child NodeID, advertised K_v, NodeClass)` for every child in the tree. Because every sibling holds the same roster snapshot (identified by its sequence number), they can each run the election locally and reach identical results **without exchanging a single election message**.

A roster older than $5\text{ s}$ is considered stale and must not be used for election.

## 3.3 The Millisecond Execution Timeline

```text
[ T = 0ms ] Node A (Parent) physically loses internet connection.
```

### Phase 1: Detection (T = 100ms - 200ms)
*   `T = 100ms`: Nodes B, C, and D realize they haven't received a video packet or keepalive ping from `Node A`. They all send an active `PING_PROBE` to `A`.
*   `T = 200ms`: No `PONG` is received. `Node A` is officially declared dead by all children simultaneously (the same detection window as Ch3 §3.3).

### Phase 2: Deterministic Election (T = 205ms)
*   Each orphan holding a fresh roster locally computes the **Deputy**: the sibling with the highest roster-advertised $K_v$, **ties broken by lowest NodeID** (byte-lexicographic comparison). Leaf-class nodes (see node classes, §5) are marked in the roster and are never electable.
*   Suppose the roster records: `B = 80`, `C = 150`, `D = 20`. Node C is elected Deputy by every sibling independently — same snapshot, same rule, same result.
*   Any orphan with **no roster, a stale roster, or that is itself leaf-only** skips the election entirely and runs the standard Ch3 §3.3 passive-set promotion on its own.

### Phase 3: Re-attachment (T = 210ms - 250ms)
*   **Node C's Action (the Deputy):** C runs the canonical Ch3 §3.3 procedure to find *its own* new parent — query the Passive Set (or the cached DISCOVERY peer list at small $N$), promote the best candidate, connect. The election is thus coordinator-selection over Ch3 §3.3, not a rival algorithm.
*   **Node B and D's Action:** B and D immediately send `RELAY_JOIN_REQUEST` — i.e., `NEIGHBOR (0x05)` with `Priority = HIGH` and `TreeID = m` (Appendix D) — directly to Node C, and arm a **Deputy-response timer $\tau_{\text{deputy}} = 45\text{ ms}$**.
*   `T = 250ms`: Node C answers with `ACCEPTED (0x08)` and the branch is restored.
*   **Fallback:** if an orphan receives no `ACCEPTED` before its timer fires (Deputy saturated, dead, or refusing), it abandons the election and independently runs Ch3 §3.3 passive-set promotion — the same path it would have taken with no election at all. The worst case is therefore never *worse* than plain HyParView recovery; the election's value is that it preserves the subtree and prevents $k$ orphans from thundering-herd onto the same passive-set candidates.

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
