# 2. Systematic Redundancy Allocation

The protocol operates in a **Systematic FEC Mode** with an adaptive redundancy rate bounded between $5\%$ and $30\%$:

1.  **Source Transmission:** The first $K$ packets contain the original source symbols unchanged, allowing high-performance, zero-overhead decoding under clean network conditions.
2.  **Adaptive Parity Generation:** Each peer measures the rolling packet loss rate $\rho$ to each active parent over a 2-second sliding window. The parity symbol count $E$ is computed as:
    $$E = \text{clamp}\left(2\rho \cdot K,\ 0.05K,\ 0.30K\right)$$
    This scales parity proportionally to observed loss (doubling $\rho$ to leave headroom), with a $5\%$ floor for clean links and a $30\%$ ceiling to bound bandwidth overhead on degraded mobile links.
    *(Example: clean link at $\rho = 0.02$ → $E = 0.04K$, clamped to $0.05K$. Cellular link at $\rho = 0.15$ → $E = 0.30K$.)*
3.  **Proactive Dispersion:** Parity symbols are distributed across the Multi-Forest active parent sets. If a downstream child suffers packet loss within the measured $\rho$ budget, it reconstructs the original payload locally using the parity symbols, completely eliminating retransmission-induced latency spikes.
4.  **Rate Update Interval:** $E$ is recalculated once per segment (every $1.0\text{ s}$) and communicated to the encoder via a local control signal. No wire format change is required; the ESI field already distinguishes source ($\text{ESI} < K$) from parity ($\text{ESI} \ge K$) symbols.
