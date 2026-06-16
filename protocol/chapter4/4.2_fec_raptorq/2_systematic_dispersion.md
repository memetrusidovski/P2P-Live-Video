# 2. Systematic Redundancy Allocation

By default, the protocol operates in a **Systematic FEC Mode** with a $10\%$ redundancy ceiling:

1.  **Source Transmission:** The first $K$ packets contain the original source symbols unchanged, allowing high-performance, zero-overhead decoding under clean network conditions.
2.  **Parity Generation:** Concurrently, the encoder generates $E = 0.10 \cdot K$ parity symbols (e.g., if $K = 50$, $E = 5$ parity symbols are generated).
3.  **Proactive Dispersion:** These parity symbols are distributed across the Multi-Forest active parent sets. If a downstream child suffers up to 10% packet drop due to UDP queue congestion, it reconstructs the original payload locally using the parity symbols, completely eliminating retransmission-induced latency spikes.
