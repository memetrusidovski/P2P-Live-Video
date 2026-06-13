# Chapter 3: Overlay Membership and Epidemic Gossip (HyParView Protocol)

### 3.1 Active and Passive Neighbor Sets: Mechanics of Connection Maintenance

Once bootstrap peers are obtained, nodes maintain local overlay membership using a customized **HyParView** gossip framework. This decouples peers from global trackers and confines membership state locally:
*   **Active Set ($\mathcal{A}$):** A small, high-priority set of connected peers (size $c_a \approx 8$) with whom the node maintains active QUIC channels. These are used for direct video chunk forwarding and latency-sensitive metadata.
*   **Passive Set ($\mathcal{P}$):** A larger pool of backup peer addresses (size $c_p \approx 32$) stored in local memory. No active sockets are open to these nodes; they are cached as candidate standbys.

```text
                  [ Gossip Message Loop ]
                 /                       \
        Active Set (A) <=============> Passive Set (P)
        (Open Sockets)  (Failed parent  (Memory Cache)
                        demoted here)
```

### 3.2 Gossip-based Topology Synchronization: Protocol Frame Formats

Peers periodically exchange membership states using compact UDP-based gossip frames to maintain small-world properties:

```text
GOSSIP_PEER_EXCHANGE Frame:
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|   Version     |   Type (0x02) |          Payload Length       |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                        Sequence Number                        |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                      S/Kademlia Sender NodeID                 |
|                           (32 bytes)                          |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|   Neighbor Count (N)          | Reserved                      |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                               |
|             List of N Peer Address Blocks (IP + Port)         |
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                         HMAC Signature                        |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

### 3.3 Dynamic Churn Recovery: Sub-Second Re-Routing Strategies

*   **Heartbeat Probing:** Nodes exchange low-overhead ping/pong keepalives every $100\text{ ms}$ over their active QUIC streams.
*   **Immediate Eviction:** If a parent fails to respond to 2 consecutive pings ($200\text{ ms}$ total), it is flagged as dead.
*   **Instant Promotion:** The orphan peer immediately selects the highest-reputation peer from its Passive Set $\mathcal{P}$, opens a QUIC stream, and requests connection promotion. This sub-second transition prevents the client's playback buffer from running dry.
