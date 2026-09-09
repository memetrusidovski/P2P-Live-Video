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

## Scaled Active Set for High-Fan-Out Relays

The $c_a = 8$ cap sizes the *control plane* (gossip, bitfield exchange, TFT state) — it is separate from the tree data plane, where a relay pushes slice data unconditionally to all of its $K_v$ tree children. For ordinary nodes ($K_v \le 16$) the two planes overlap almost completely and $c_a = 8$ suffices.

For high-fan-out relays the fixed cap breaks bitfield propagation: a super node with $K_v = 10{,}000$ children keeps gossip connections with only 8 of them (0.08%), so 99.92% of its children cannot see its buffer bitfield and must hunt for missing blocks via multi-hop gossip among siblings who are usually missing the same blocks — adding 2–4 gossip hops (~80–160 ms) to every PULL during correlated loss bursts.

Relay nodes therefore scale their effective Active Set with their tree fan-out:

$$c_a^{\text{eff}} = \begin{cases} c_a = 8 & \text{if } K_v \le 2 \cdot c_a \\ \min\left(64,\ \left\lfloor K_v / 10 \right\rfloor\right) & \text{if } K_v > 2 \cdot c_a \end{cases}$$

The extra slots are filled preferentially with the relay's own tree children, maximizing the fraction of children with direct bitfield visibility. The cap of 64 bounds control-plane bandwidth: at $\tau_{\text{gossip}} = 1000\text{ ms}$ and Bloom-compressed bitfields (Ch4 §4.3.2), 64 gossip peers cost a negligible fraction of a super node's upload.

*Future option (deliberately not required by this spec):* a one-to-many relay-broadcast bitfield channel, where the relay unicasts its bitfield to **all** $K_v$ children every 100 ms outside HyParView. This is architecturally cleaner at extreme fan-out but adds a frame type and a second distribution mechanism; the scaled $c_a$ above is the mandated baseline, and FEC parity (Ch4 §4.2) already absorbs most correlated loss.
