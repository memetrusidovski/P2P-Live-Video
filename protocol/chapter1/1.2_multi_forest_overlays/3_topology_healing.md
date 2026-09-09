# 3. Topology Healing: The Deterministic Sibling Election Protocol

## 3.1 The Vulnerability of Trees
The primary flaw of any tree-based distribution network is its fragility. Consider a simple branch:
`Source -> Node A -> Node B -> Node C`
If `Node A` abruptly disconnects (power failure, closed app), `Node B` and `Node C` are immediately starved of video data.

In a traditional P2P system, `Node B` would realize `A` is dead, query the DHT for a new parent, probe them, and connect. This process takes 1 to 3 seconds. For a live stream with a tight 3-second playout deadline, this delay results in undeniable video buffering.

## 3.2 The Sibling Election Solution
To heal a branch within one detection window plus a single handshake — about $250$ ms on a local path, RTT-scaled elsewhere (Ch3 §3.3) — we avoid a fresh DHT walk entirely. Instead, we use **Localized Deterministic Sibling Election**, built as a *coordination layer on top of* the canonical churn-recovery primitive — HyParView passive-set promotion (Chapter 3 §3.3). The election decides **who** re-attaches the subtree; the actual re-attachment mechanism is always Ch3 §3.3, and every failure path degrades gracefully to it.

### The Child Roster (Election Input)
Deterministic election requires that all siblings compute from the *same data*. To guarantee this, every relay parent sends each of its children, **per tree**, a `ROSTER` frame (0x18, Appendix D §D.4.15) every $\tau_{\text{roster}} = 1000\text{ ms}$: a sequence-numbered list of `(Child NodeID, K_avail in this tree, NodeClass, Flags, address)` entries plus the parent's total child count $k$ in that tree. Because every sibling holds the same roster snapshot (identified by its sequence number), they can each run the election locally and reach identical results **without exchanging a single election message**.

A roster older than $5\text{ s}$ is considered stale and must not be used for election.

#### The Roster Lists Relays of This Tree Only

A replacement parent for tree $T_m$ must be a `RELAY`-class node whose assignment includes $m$ — every other node has out-degree zero in $T_m$ by the Orthogonal Placement Rule (§1.3). The dead parent's children in $T_m$ are *subscribers* of $m$; only about $1/M$ of them relay it. A roster that ranked all children by raw $K_v$ would elect a Deputy that relays some *other* tree with probability $\approx 1 - 1/M$, every orphan would send it a `RELAY_JOIN_REQUEST(TreeID = m)` it cannot honour, wait out $\tau_{\text{deputy}}$, and fall back — the election would be a net 45 ms cost in the common case. With $k = 8$ children at $M = 6$ the probability that *no* child relays $m$ at all is $(5/6)^8 = 23\%$.

The roster therefore contains only children with bit $m - 1$ of their `AssignedTrees` set, ordered by their advertised $K_{\text{avail}}$ **in tree $m$** (their free slots against $T_m$'s declared bitrate, §1.3) descending, ties broken by lowest NodeID. Multi-tree super nodes appear in the rosters of every tree they relay. Leaf-class children never appear. When no child relays $m$, the roster is empty and every orphan goes straight to Ch3 §3.3 without arming the timer — the 23% case costs nothing.

#### The Roster Is Bounded, Not Complete

A roster listing *every* child costs $\approx 43$–$55 \cdot k$ bytes and is sent to all $k$ children each second — $O(k^2)$ traffic, which is unaffordable at exactly the fan-out multi-tree super nodes exist to provide:

| Children $k$ | Full roster | Share of the node's uplink |
| ---: | ---: | ---: |
| 10 | 0.03 Mbps | 0.34% |
| 100 | 3.4 Mbps | 3.4% — already past the $\text{CDO} \le 2\%$ target |
| 1,000 | 344 Mbps | 34% |
| 10,000 | 34 Gbps | **344% — physically impossible** |

The roster therefore carries only the **top $R_{\text{roster}} = 8$ qualifying children**. Cost becomes $\le 55 \cdot R_{\text{roster}} + 12 \approx 450$ bytes per child per second — a flat $\approx 0.36\%$ of uplink at every fan-out ($8$ children on a 10 Mbps link: $29$ kbps; $10{,}000$ on 10 Gbps: $36$ Mbps).

Nothing is lost, because the roster's only job is to identify the Deputies: the election picks the *highest*-capacity qualifying siblings, so children ranked below the top 8 were never candidates. Determinism is preserved — every child receives the identical ordered list, so every child computes the identical result. The total child count $k$ travels in the frame's `ChildCount` field, since the bounded list no longer reveals it.

#### Multiple Deputies at High Fan-Out

A single Deputy cannot absorb an arbitrary number of orphans: it has its own free slots $K_{\text{avail}}^{(1)}$ in this tree, which the roster publishes. With $k = 10{,}000$ orphans and a Deputy holding 10 free slots, 9,990 of them would send a `RELAY_JOIN_REQUEST`, receive nothing, wait out $\tau_{\text{deputy}} = 45\text{ ms}$ and fall back to Ch3 §3.3 anyway — the election would have *added* 45 ms of latency for 99.9% of the subtree while replacing a thundering herd on the passive set with a thundering herd on one Deputy.

Orphans therefore partition deterministically across the roster's entries. Let the roster hold $E \le R_{\text{roster}}$ entries with free slots $K^{(1)} \ge K^{(2)} \ge \dots \ge K^{(E)}$ and $C = \sum_{i \le E} K^{(i)}$ their total. Every orphan $o$ computes, from the shared roster:

$$D = \min\left(E,\ \left\lceil \frac{k}{K^{(1)}} \right\rceil\right), \qquad \text{deputy\_index}(o) = \text{Blake3}(NodeID_o) \bmod D$$

and sends its `RELAY_JOIN_REQUEST` to deputy $\text{deputy\_index}(o)$. Each deputy independently runs Ch3 §3.3 to find its own new parent, exactly as the single-Deputy case does.

When even all $E$ deputies cannot absorb the orphans ($k > C$), the surplus **skips the election entirely** and goes straight to Ch3 §3.3 without arming the timer. Membership in the surplus is decided from the same shared roster without any orphan knowing its index among the $k$: orphan $o$ is surplus iff

$$\frac{\text{Blake3}(NodeID_o) \bmod 2^{16}}{2^{16}} > \frac{C}{k}$$

so that in expectation exactly $C$ orphans contend for $C$ slots, and the 45 ms Deputy wait is only ever paid by an orphan that had a genuine chance of being served.

## 3.3 The Millisecond Execution Timeline

```text
[ T = 0ms ] Node A (Parent) physically loses internet connection.
```

### Phase 1: Detection (T = 100 ms to $\tau_{\text{evict}}$)
*   `T = 100ms`: Nodes B, C, and D realize they have received neither a packet nor a heartbeat from `Node A` for $\tau_{\text{ping}}$. They all send an active `PING_PROBE` to `A`.
*   `T = τ_evict`: No `PONG` is received within the RTT-scaled deadline $\tau_{\text{evict}} = \max(200, 2\tau_{\text{ping}} + SRTT + 4\,RTTVAR)$ ms (Ch3 §3.3) — $200$–$250$ ms for nearby siblings. `Node A` is declared dead by each child within a few RTTVAR of the others. The timeline below uses $\tau_{\text{evict}} = 200$ ms for a local branch; every later phase shifts with it.

### Phase 2: Deterministic Election (T = 205ms)
*   Each orphan holding a fresh roster locally computes the **Deputy**: the roster's first entry — the sibling that relays this tree with the most free slots in it, **ties broken by lowest NodeID** (byte-lexicographic comparison). Leaf-class nodes and relays of other trees (see node classes, §5) are excluded from the roster and are never electable. Where the subtree is too large for one Deputy to absorb, the orphan instead computes its assigned deputy index from the shared roster (§3.2) — the rule is the same, evaluated over $D$ deputies rather than one.
*   Suppose the tree-$m$ roster records free slots `B = 8`, `C = 15`, `D = 2`. Node C is elected Deputy by every sibling independently — same snapshot, same rule, same result.
*   Any orphan with **no roster, a stale roster, an empty roster (no sibling relays this tree), or that is in the surplus** skips the election entirely and runs the standard Ch3 §3.3 promotion from its per-tree pool on its own. Leaf-class orphans are subscribers like any other and take part as orphans; they are simply never Deputies.

### Phase 3: Re-attachment (T = 210ms - 250ms)
*   **Node C's Action (the Deputy):** C runs the canonical Ch3 §3.3 procedure to find *its own* new parent — query the Passive Set (or the cached DISCOVERY peer list at small $N$), promote the best candidate, connect. The election is thus coordinator-selection over Ch3 §3.3, not a rival algorithm.
*   **Node B and D's Action:** B and D immediately send `RELAY_JOIN_REQUEST` — i.e., `NEIGHBOR (0x05)` with `Priority = HIGH` and `TreeID = m` (Appendix D) — directly to Node C at the address the roster carries, and arm a **Deputy-response timer $\tau_{\text{deputy}} = 45\text{ ms}$**. C is known to relay $T_m$ and to have free slots there, so the request is one it can honour.
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
