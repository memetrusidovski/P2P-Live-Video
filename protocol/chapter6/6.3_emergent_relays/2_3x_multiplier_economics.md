# 2. 3x Multiplier Economics

To motivate peers with public IPs to donate their upload capacity, the protocol implements a **3x Reputation Multiplier**:

*   When a peer acts as a relay, forwarding stream blocks to a NAT-blocked node, the NAT-blocked node issues it ordinary `PROOF_OF_UPLOAD` receipts (Ch5 §5.2) with the `RELAYED` flag set — the same non-repudiable receipt every child issues, distinguished only by the flag.
*   Verifiers weight `RELAYED` receipts $3\times$ in the contribution score:
    $$\Theta_{\text{relay}} = 3.0 \cdot \Theta_{\text{standard}}$$
    The multiplier is bounded by the half of the tree budget a relay may spend on bridging (§6.3.1): a relay that bridges to the cap earns $0.5 \times 3 + 0.5 \times 1 = 2\times$ the rank of pure tree relaying on the same upload, never more — and only while NAT-blocked peers actually need bridging, since a bridge exists only on a `RELAY_PROPOSAL`. Bridged receipts count $3\times$ toward $\Theta^{\text{rate}}$ and are capped per receipt like any tree receipt (Ch5 §5.2.2).
*   This guaranteed high reputation score elevates the relay node to the topmost layers of the distribution trees (Layer 1 and Layer 2), granting them the absolute lowest possible playback latency in the swarm.
