# 1. Peer State Transition Model

To maintain the stability of the multi-forest overlay under high churn and network jitter, every node executes a standardized state machine. Each node-level state restricts the packet types a peer can process and defines its current obligations to the swarm; the one per-tree sub-state (`CHURN_REPAIR`) restricts nothing outside its tree.

| State | Code | Description | Next State (Triggers) |
| :--- | :--- | :--- | :--- |
| **BOOTSTRAP** | `0x00` | Instantiating keypairs, validating Node ID, loading local candidate cache | `DISCOVERY` (Initial seed loaded) |
| **DISCOVERY** | `0x01` | Querying S/Kademlia DHT for active stream keys and stream publisher info | `JOINING` (Stream records fetched) |
| **JOINING** | `0x02` | Executing HyParView handshakes, populating Active and Passive sets | `CONNECTING` (Active set size $\ge \theta_{\text{join}}$, see below) |
| **CONNECTING**| `0x03` | Initiating Tree Join handshakes to establish parent connections; anchoring the buffer to `live_edge_segment_id` from the Stream Record (Ch4 §4.3 late-joiner sync) | `ACTIVE` (Parents connected for all **subscribed** trees, live edge anchored — see below) |
| **ACTIVE** | `0x04` | Streaming active video; decoding, verifying, and uploading symbols in every tree that has a parent. Carries the per-tree repair set $\mathcal{T}_{\text{repairing}}$ (below) | `DISCOVERY` (every parent lost, complete disconnect) <br> `TERMINATED` (Verified `STREAM_END` gossip received) |
| **CHURN_REPAIR**| `0x05` | **Per-tree sub-state of `ACTIVE`**, entered for tree $T_m$ when its parent times out: re-routing that slice to a standby from the per-tree pool (Ch3 §3.3). Every other tree keeps streaming, and the node keeps forwarding, serving PULLs, sending rosters and issuing receipts | `ACTIVE` for $T_m$ (re-route successful) <br> `DISCOVERY` (the last remaining parent is lost) <br> `TERMINATED` (Verified `STREAM_END` gossip received) |
| **TERMINATED**| `0x06` | Shutting down connections, releasing sockets, broadcasting disconnect | (None) |

## Adaptive JOINING Threshold

The `JOINING` → `CONNECTING` transition fires when the Active Set reaches the adaptive threshold $\theta_{\text{join}}$, computed from the swarm size $N$ learned during `DISCOVERY` (the peer count returned by the stream registration lookup — no extra round-trip is required):

$$\theta_{\text{join}} = \max(1, \min(4, N - 1))$$

| Total peers $N$ (source + viewers) | $\theta_{\text{join}}$ | Behaviour |
| :---: | :---: | :--- |
| 2 | 1 | First viewer connects directly to the source |
| 3 | 2 | |
| 4 | 3 | |
| $\ge 5$ | 4 | Full threshold (half of $c_a = 8$) |

A fixed threshold of 4 would permanently block every stream at $N < 5$: with only $N - 1$ other peers in existence, the HyParView `FORWARD_JOIN` random walk can never populate an Active Set of 4, and every viewer would be stuck in `JOINING` before receiving a single frame. The adaptive threshold restores normal behaviour at $N \ge 5$ while making cold-start (every stream begins at $N = 2$) work.

## The Subscribed Tree Set

`CONNECTING → ACTIVE` and `CHURN_REPAIR → ACTIVE` are gated on parents for the **subscribed** tree set $\mathcal{T}_{\text{sub}} \subseteq \{1 \ldots M\}$, **not** on all $M$ trees.

The distinction is load-bearing. Capacity adaptation (Ch1 §1.1.5) lets a peer *shed* a tree it cannot join — dropping the corresponding SVC enhancement layer instead of stalling — and Ch1 §1.1.5 states plainly that insufficient swarm capacity is "the common case rather than the exception". A transition condition of "parents connected for all $M$ slices" is therefore unsatisfiable for any peer that has shed a tree: it would sit in `CONNECTING` forever, never reach `ACTIVE`, and never render a frame — the exact failure the shedding mechanism exists to prevent.

Subscription is by **layer**: the peer holds a layer prefix $\mathcal{L}_{\text{sub}} = \{L_0 \ldots L_j\}$ and $\mathcal{T}_{\text{sub}}$ is the set of trees the current slicing matrix (Ch1 §1.2.4) assigns to those layers:

$$\mathcal{T}_{\text{sub}} = \{\, m : \text{layer}(m) \in \mathcal{L}_{\text{sub}} \,\}, \qquad L_0 \in \mathcal{L}_{\text{sub}} \text{ always}$$

*   Shedding layer $l$ (Ch1 §1.1.5 §5.3) removes $l$ and every layer above it from $\mathcal{L}_{\text{sub}}$, and with them every tree carrying those layers; a successful re-join of *all* of a layer's trees after the 10 s hysteresis restores it.
*   A `MANIFEST_UPDATE` that changes $M$ (Ch1 §1.2.4) leaves $\mathcal{L}_{\text{sub}}$ unchanged and recomputes $\mathcal{T}_{\text{sub}}$ from the new matrix — shed state is carried by layer, so it survives a resize in which the tree indices carrying a layer change.
*   **The base layer is never shed and its trees never leave $\mathcal{T}_{\text{sub}}$.** A peer that cannot obtain a parent in every $L_0$ tree does not enter `ACTIVE` at all — it keeps retrying, and the source's base-layer reserve (Ch1 §1.1.5 §5.4) is the mechanism of last resort that serves it. This is what makes "the base layer never fails" a property of the state machine and not only of the scheduler.
*   **A relay's assigned trees are always joined, subscribed or not.** A `RELAY`-class node's join set is $\mathcal{T}_{\text{join}} = \mathcal{T}_{\text{sub}} \cup \mathcal{T}_{\text{assigned}}$, where $\mathcal{T}_{\text{assigned}}$ is its rendezvous assignment plus any coverage grant (Ch1 §1.2.1). It holds a parent in every assigned tree and forwards that tree to its children whether or not it renders the layer the tree carries (Ch1 §1.1.5 §5.3 item 4). The `ACTIVE` transition is gated on $\mathcal{T}_{\text{sub}}$ alone; joining an assigned-but-unrendered tree proceeds in the background and never blocks playback. A leaf-class node has $\mathcal{T}_{\text{assigned}} = \emptyset$.

## Repair Is Per Tree, Not Per Node

A node holds up to $M$ parents and its failures arrive one tree at a time, so the lifecycle must not have one repair state for the whole node. An earlier version did: `CHURN_REPAIR` was a node-level state in which the core loop ran only the re-route, and `ProcessStreamingBuffers()` ran only in `ACTIVE`. A relay whose tree-3 parent died therefore **stopped decoding and forwarding tree-1 data to its own children** until every slice was restored; those children saw $> \tau_{\text{ping}}$ of silence, probed, evicted it, entered their own repair, and stopped forwarding to *their* children — one dead parent at depth $d$ became a churn wave down every tree of every affected subtree, one detection period per level, instead of the $1/M$ loss the edge-disjoint forest exists to guarantee.

`CHURN_REPAIR` is therefore a **sub-state of `ACTIVE` scoped to a tree**. The node keeps

$$\mathcal{T}_{\text{repairing}} \subseteq \mathcal{T}_{\text{sub}}$$

and for every tree *not* in it continues, without interruption: receiving and verifying blocks, forwarding them to its children, answering `PULL_REQUEST`s, sending `ROSTER`s, issuing and consuming receipts, and sending heartbeats. Repair of tree $m$ runs the Ch3 §3.3 procedure for that tree alone and removes $m$ from $\mathcal{T}_{\text{repairing}}$ on success. The node leaves `ACTIVE` only when its **last** parent is gone (`DISCOVERY`) or on `STREAM_END`. Losing every $L_0$ tree's parent stalls *playback* — the decoder has no base layer — but does not stop the node relaying the trees it still receives; playback and relay duty are independent.

`AllSlicesRestored()` — now $\mathcal{T}_{\text{repairing}} = \emptyset$ — is evaluated over $\mathcal{T}_{\text{sub}}$ for the same reason. The Ch3 §3.3.2 algorithm was always written per slice; this section now agrees with it.

### $\theta_{\text{join}}$ Is a Progress Floor, Not a Target

$\theta_{\text{join}}$ decides only *when a peer may stop waiting* — it is not the size the Active Set should settle at. Two rules follow, and both matter in the real deployment case where $N$ is read from a record the peer cannot fully trust:

*   **Keep growing after the transition.** A peer that advances at $\theta_{\text{join}} = 1$ continues running `FORWARD_JOIN` and `SHUFFLE` toward $c_a = 8$ throughout `CONNECTING` and `ACTIVE`. A small $\theta_{\text{join}}$ is a cold-start concession, never a completed membership view.
*   **A low $N$ must not become an eclipse foothold.** `swarm_size` comes from the publisher's signed Stream Record (Ch2 §2.3.1), but the `GET_PEERS` response carrying it is served by DHT guardians, so an adversary who is able to answer the lookup can under-report $N$ and drive $\theta_{\text{join}}$ to 1 — leaving the victim attached to a single attacker-chosen peer. Three existing mechanisms bound this and are load-bearing here: the record is signed by $SK_{\text{Publisher}}$ (a forged $N$ requires the publisher's key, not merely a hostile guardian), the lookup itself runs over $\alpha = 3$ disjoint paths (Ch2 §2.1.2), and the continued growth above dilutes any single-peer view within seconds. A peer that reaches `ACTIVE` while $|\mathcal{A}| < 2$ **must** keep at least one independent DHT re-query in flight until a second distinct parent is established.

## State Transition Diagram

```text
                  +-------------------+
                  |   0x00 BOOTSTRAP  |
                  +-------------------+
                            |  (Seed node loaded)
                            v
                  +-------------------+
                  |   0x01 DISCOVERY  | <--------------------+
                  +-------------------+                      |
                            |  (Stream records retrieved)    |
                            v                                |
                  +-------------------+                      |
                  |   0x02 JOINING    |                      |
                  +-------------------+                      |
                            |  (Active set populated)        |
                            v                                | (Complete
                  +-------------------+                      |  disconnect)
                  |  0x03 CONNECTING  |                      |
                  +-------------------+                      |
                            |  (Parents connected)           |
                            v                                |
                  +------------------------------------------+   |
                  |               0x04 ACTIVE                | --+ (every parent lost)
                  |                                          |
                  |  per tree m in T_sub:                    |
                  |   streaming(m) <---> 0x05 CHURN_REPAIR(m)|
                  |   (parent timeout)   (re-route success)  |
                  |                                          |
                  |  forwarding, PULL service, rosters and   |
                  |  receipts continue for every tree that   |
                  |  still has a parent                      |
                  +------------------------------------------+
                        |                     |
     (Graceful quit /   |                     | (Verified STREAM_END gossip)
      App close)        v                     v
                  +-------------------+
                  |  0x06 TERMINATED  |
                  +-------------------+
```

## Broadcaster Failure: STREAM_END Frame

When the broadcaster intends to end a stream, it gossips a signed `STREAM_END` frame to its immediate children before closing its socket. Children verify the signature against the pinned $PK_{\text{Source}}$ and forward it to their subtrees using the same HyParView epidemic gossip mechanism (Chapter 3). Any peer receiving a valid `STREAM_END` immediately transitions to `TERMINATED`.

```text
STREAM_END Frame (Type 0x04):
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|   Version     |  Type (0x04)  |          Payload Length       |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                               |
|                   StreamID Key K_s (32 bytes)                 |
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|              Final Segment Sequence Number (4 bytes)          |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|            Timestamp (8-byte microsecond integer)             |
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                               |
|         Ed25519 Source Signature (64 bytes)                   |
|         Sign_{SK_Source}(StreamID || FinalSeqNum || Timestamp)|
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

If the broadcaster disconnects abruptly without sending `STREAM_END`, peers will exhaust their per-tree repair attempts across all $M$ slices. Once every parent is lost, the node falls to `DISCOVERY`; if no DHT re-query returns records (the stream's DHT entries expire after $\tau_{\text{ttl}} = 180\text{ s}$), it transitions to `TERMINATED`.
