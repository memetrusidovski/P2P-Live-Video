# 2. 3x Multiplier Economics

To motivate peers with public IPs to donate their upload capacity, the protocol implements a **3x Reputation Multiplier**:

*   When a peer acts as a relay, forwarding stream blocks to a NAT-blocked node, the NAT-blocked node issues it ordinary `PROOF_OF_UPLOAD` receipts (Ch5 §5.2) with the `RELAYED` flag set — the same non-repudiable receipt every child issues, distinguished only by the flag.
*   Verifiers weight `RELAYED` receipts $3\times$ in the contribution score:
    $$\Theta_{\text{relay}} = 3.0 \cdot \Theta_{\text{standard}}$$
    The multiplier is bounded by the PULL reserve the relay may spend on bridging (§6.3.1), so it cannot be farmed beyond $\approx 3 \times 10\%$ of a node's upload.
*   This guaranteed high reputation score elevates the relay node to the topmost layers of the distribution trees (Layer 1 and Layer 2), granting them the absolute lowest possible playback latency in the swarm.
