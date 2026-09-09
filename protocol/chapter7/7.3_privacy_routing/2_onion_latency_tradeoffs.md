# 2. Onion Latency Tradeoffs

While onion routing guarantees strong anonymity, it introduces significant network performance trade-offs:

1.  **Latency Inflation:** Each hop adds intermediate queuing and routing propagation delays. The total RTT scales linearly as:
    $$RTT_{\text{onion}} = RTT(C, M_1) + RTT(M_1, M_2) + RTT(M_2, M_3) + RTT(M_3, D)$$
    Typically, this increases end-to-end latency by $150\text{ ms}$ to $400\text{ ms}$, which remains well within the 5-second live playout deadline.
2.  **Throughput Overhead:** Nested 16-byte Poly1305 authentication tags increase the packet header size, slightly decreasing transmission efficiency.
3.  **Leaf Constraint:** Because onion-routed nodes cannot receive incoming UDP connections directly, they operate in the **`LEAF_PRIVATE` (0x02)** node class (Ch1 §1.2.5). This is not an exemption from the protocol's incentive rules — it is a defined class with a defined price:
    *   The node is a leaf in all $M$ trees and is never elected Deputy or recruited as an emergent relay.
    *   It receives the **base layer by right** through the universal service floor, but enhancement layers only from genuine surplus — it is the first child preempted when an enhancement tree is contended — and it pays the $150$–$400\text{ ms}$ onion penalty above on every hop.
    *   Proof-of-Work is still required (Ch2 §2.2); no class is PoW-exempt.
    *   A privacy peer that wishes to improve its standing **may** serve `PULL_REQUEST`s through its own circuit, earning ordinary Tit-for-Tat / PoU credit despite being unreachable for tree pushes.

    Privacy is therefore paid for in latency and quality ceiling, keeping the core axiom — contribution and playback performance are inextricably linked — intact rather than carving a sanctioned free-ride out of it.
