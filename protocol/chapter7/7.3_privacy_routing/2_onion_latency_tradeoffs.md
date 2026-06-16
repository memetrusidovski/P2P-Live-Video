# 2. Onion Latency Tradeoffs

While onion routing guarantees strong anonymity, it introduces significant network performance trade-offs:

1.  **Latency Inflation:** Each hop adds intermediate queuing and routing propagation delays. The total RTT scales linearly as:
    $$RTT_{\text{onion}} = RTT(C, M_1) + RTT(M_1, M_2) + RTT(M_2, M_3) + RTT(M_3, D)$$
    Typically, this increases end-to-end latency by $150\text{ ms}$ to $400\text{ ms}$, which remains well within the 5-second live playout deadline.
2.  **Throughput Overhead:** Nested 16-byte Poly1305 authentication tags increase the packet header size, slightly decreasing transmission efficiency.
3.  **Leaf Constraint:** Because onion-routed nodes cannot receive incoming UDP connections directly, they are strictly locked as **Leaf-Only Consumers** and are exempted from tree relay obligations.
