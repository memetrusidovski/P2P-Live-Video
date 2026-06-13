# Chapter 6: NAT Traversal and Emergent Relaying Systems

### 6.1 UDP Hole Punching under Symmetric and CGNAT Topologies

Direct peer connections are frequently blocked by network boundaries. We classify NAT behaviors into four categories based on the STUN protocol (RFC 5389):
1.  **Full Cone NAT:** Easy traversal; any external peer can send packets to the mapped port.
2.  **Restricted Cone NAT:** Traversable if the internal node first sends a packet to the external peer's IP.
3.  **Port Restricted Cone NAT:** Traversable if the internal node first sends a packet to the external peer's IP and port.
4.  **Symmetric NAT / CGNAT:** Difficult traversal; the mapped external port changes for every destination IP. Direct connection is impossible between two symmetric NAT nodes.

### 6.2 QUIC Connection Migration and STUN/ICE Negotiation

*   **Bidirectional Hole Punching:** Nodes behind restricted NATs perform bidirectional hole punching by sending coordinated UDP packets to each other's predicted external ports, opening a path through their firewalls.
*   **QUIC Connection Migration:** Since UDP mappings can change unexpectedly due to NAT timeouts, the protocol uses UDP-based QUIC transport. QUIC identifies connections using a 64-bit `Connection ID` instead of the 4-tuple IP/Port. If a NAT re-maps a node's port, the QUIC session migrates seamlessly without dropping the streaming connection.

### 6.3 Autonomous Relayer Selection and Crowd-Sourced Superpeer Incentivization

For peers stuck behind dual-symmetric NATs where hole punching mathematically fails, the protocol recruits **Emergent Relays**:

```text
   [ Symmetric Peer A ] <---> [ Public Superpeer (Relay) ] <---> [ Symmetric Peer B ]
```

*   **Relay Recruitment:** Nodes with public IP addresses (No NAT) and high bandwidth are automatically recruited as relays.
*   **Incentive Multipliers:** Relaying is treated as a premium contribution. Relayer nodes receive a $3\times$ score multiplier on their PoU receipts. This high score guarantees them top priority access to the broadcaster's direct low-latency feed, providing a strong incentive for users to keep their network ports open.
