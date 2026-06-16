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
