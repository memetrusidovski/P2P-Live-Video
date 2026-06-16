# 2. 3x Multiplier Economics

To motivate peers with public IPs to donate their upload capacity, the protocol implements a **3x Reputation Multiplier**:

*   When a peer acts as a relay, forwarding encrypted stream blocks between NAT-blocked nodes, it generates **Relay PoU Receipts**.
*   These receipts carry a $3\times$ weight multiplier during the global reputation score calculation:
    $$\Theta_{\text{relay}} = 3.0 \cdot \Theta_{\text{standard}}$$
*   This guaranteed high reputation score elevates the relay node to the topmost layers of the distribution trees (Layer 1 and Layer 2), granting them the absolute lowest possible playback latency in the swarm.
