# 3. Consensus Eviction

Peers do not rely on a central server to track blacklists. Instead, they gossip localized reputation audits to form an emergent consensus.

```text
REPUTATION_AUDIT_GOSSIP Frame (Type 0x22):
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|   Version     |  Type (0x22)  |          Payload Length       |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                      S/Kademlia Sender NodeID                 |
|                            (32 bytes)                         |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|   Suspect Count (N)           | Reserved                      |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                               |
|        List of N Suspect Peer Blocks (NodeID + PenaltyScore)  |
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

*   **Eviction Threshold:** When a peer $X$'s Penalty Score in the local gossip consensus exceeds a threshold ($\Theta_{\text{malicious}} \ge 0.8$), all well-behaved nodes immediately sever active connections, drop $X$ from their passive sets, and refuse to route its messages. This completely starves the attacker's nodes of video data.
