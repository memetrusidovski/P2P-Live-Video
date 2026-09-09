# 1. Memory Structures (Active vs. Passive Sets)

To scale to one million concurrent users under volatile conditions, we avoid global routing states. Instead, each node maintains a localized partial membership view using a customized implementation of the **HyParView** epidemic membership protocol. This ensures that the global connection graph remains a highly connected, small-world network with a low average path length and high clustering coefficient.

Peers divide their local memory views into two distinct, symmetric structures:
*   **Active Set ($\mathcal{A}$):** A small, high-priority set of connected neighbors (size $c_a \approx 8$) with whom the node maintains open, active QUIC streams. Peers exchange keepalives, control metadata, and stream packets with their active set.
*   **Passive Set ($\mathcal{P}$):** A larger pool of inactive neighbor addresses (size $c_p \approx 32$) stored as a standby backup. No sockets are open to these nodes; they are cached as candidate standbys.

```text
  +-----------------------------------------------------------+
  |                   Local Peer Connection State             |
  |                                                           |
  |   +-----------------------+     +---------------------+   |
  |   |   Active Set (A)      |     |   Passive Set (P)   |   |
  |   |   Size: ~8 peers      |     |   Size: ~32 peers   |   |
  |   |   (Active QUIC Sockets)     |   (Memory Cache Only)   |   |
  |   +-----------------------+     +---------------------+   |
  |               ^                            ^              |
  |               | (Connection failure)       |              |
  |               +----------------------------+              |
  |                     Demote failed parent                  |
  +-----------------------------------------------------------+
```

## The Passive Set Is Indexed by Tree

Passive-set entries are Peer Records (Ch2 §2.3.3), so every standby is known by NodeID, class, `AssignedTrees` and reachability before any packet is sent to it. The set is therefore kept as $M$ overlapping **per-tree candidate pools**:

$$\mathcal{P}_m = \{\, p \in \mathcal{P} : \text{class}(p) = \texttt{RELAY},\ \text{bit}_{m}(\text{AssignedTrees}(p)) = 1,\ \text{Reachability}(p) \ne \texttt{SYMMETRIC} \,\}$$

and the peer maintains a **floor of 3 candidates in $\mathcal{P}_m$ for every subscribed tree** $m \in \mathcal{T}_{\text{sub}}$. This is what makes the 250 ms churn-recovery path (§3.3) a *filtered pick* rather than a random draw: a random passive-set member is a relay of the wanted tree with probability $(1-\ell)/M$ — about one in twelve at $M = 6$ with half the swarm leaf-class — so an unfiltered promotion would spend $\approx 12$ handshakes and burn $\approx 12$ passive entries to find one usable parent.

Pools are fed by the same gossip that fills the Passive Set. `SHUFFLE` and `GOSSIP_EXCHANGE` carry a `WantedTrees` bitmap so a peer short of relays for tree $m$ asks its neighbours for exactly those; `FORWARD_JOIN` carries the joiner's full record. A pool that stays below the floor for a full gossip round triggers a `GET_PEERS(WantedTrees = \text{bit } m)` re-query, at most every 30 s per tree (Ch2 §2.3.2). At $c_p = 32$ random relays the *expected* pool size is $32(1-\ell)/M \approx 2.7$ at $M = 6$, $\ell = 0.5$ — below the floor — which is why the wanted-tree steering exists rather than relying on random shuffle alone.

A record whose `AssignedTrees` no longer matches what the peer observes (a `NEIGHBOR(TreeID = m)` rejected with "not assigned") is corrected locally and the pool re-evaluated; the bitmap is a hint that saves a probe, not a promise.

## Scaled Active Set for High-Fan-Out Relays

The $c_a = 8$ cap sizes the *control plane* (gossip, bitfield exchange, TFT state) — it is separate from the tree data plane, where a relay pushes slice data unconditionally to all of its $K_v$ tree children. For ordinary nodes ($K_v \le 16$) the two planes overlap almost completely and $c_a = 8$ suffices.

For high-fan-out relays the fixed cap breaks bitfield propagation: a super node with $K_v = 10{,}000$ children keeps gossip connections with only 8 of them (0.08%), so 99.92% of its children cannot see its buffer bitfield and must hunt for missing blocks via multi-hop gossip among siblings who are usually missing the same blocks — adding 2–4 gossip hops (~80–160 ms) to every PULL during correlated loss bursts.

Relay nodes therefore scale their effective Active Set with their tree fan-out:

$$c_a^{\text{eff}} = \min\left(64,\ \max\left(c_a,\ \left\lfloor K_v / 10 \right\rfloor\right)\right)$$

The $\max$ against $c_a$ is load-bearing and must not be dropped: $\lfloor K_v/10 \rfloor$ alone falls **below** the baseline for every $K_v < 80$ — a relay with 20 children would scale itself *down* to 2 gossip peers, which is worse than doing nothing. The formula is monotone by construction: it is exactly $c_a = 8$ up to $K_v = 80$, grows linearly to the ceiling at $K_v = 640$, and holds at 64 thereafter.

The extra slots are filled preferentially with the relay's own tree children, maximizing the fraction of children with direct bitfield visibility. The cap of 64 bounds control-plane bandwidth: at $\tau_{\text{gossip}} = 1000\text{ ms}$ and Bloom-compressed bitfields (Ch4 §4.3.2), 64 gossip peers cost a negligible fraction of a super node's upload.

### What the Scaled Active Set Does Not Fix

Scaling $c_a$ is a mitigation, not a solution, and the spec should not overstate it. At $K_v = 10{,}000$ it raises direct bitfield visibility from $0.08\%$ of children to $0.64\%$ — an $8\times$ improvement over a number that was negligible to begin with. The remaining $99.36\%$ still cannot see their parent's bitfield.

That is acceptable only because the parent's bitfield is not what those children actually need:

*   **On the push path they need nothing.** Symbols arrive proactively; a child does not consult a bitfield to receive what is already being sent to it. The bitfield matters only for **repair**.
*   **For repair, siblings are the better source anyway.** A child's siblings under the same relay in the same tree hold precisely the same blocks, are one hop away, and — unlike the parent — are not a shared bottleneck that every one of $K_v$ children would converge on. A repair pulled from the parent competes with the parent's push duty; a repair pulled from a sibling does not.
*   **FEC absorbs the common case before any of this engages.** Adaptive parity (Ch4 §4.2.2) is sized per link to the observed loss rate, so most loss is repaired locally without a PULL at all.

The quantity that actually bounds PULL latency at extreme fan-out is therefore **sibling mesh density**, not parent visibility. Siblings enter one another's Active Sets through ordinary `SHUFFLE` gossip, which is $O(c_a)$ per peer and independent of $K_v$ — it does not degrade as the relay grows. A child of a high-fan-out relay should preferentially retain siblings when filling its own Active Set, for the same reason the relay preferentially retains children.

*Future option (deliberately not required by this spec):* a one-to-many relay-broadcast bitfield channel, where the relay unicasts its bitfield to **all** $K_v$ children every 100 ms outside HyParView. This is architecturally cleaner at extreme fan-out but adds a frame type and a second distribution mechanism, and it re-creates the $O(K_v)$ per-parent cost that the scaled Active Set exists to avoid.
