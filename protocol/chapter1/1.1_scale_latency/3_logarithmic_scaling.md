# 3. Logarithmic Overlay Scaling Proof

To prove that a million-peer swarm can scale within a 5-second playout budget, we must restrict the maximum overlay depth $D = \max(h_i)$. We model the distribution overlay as a structured graph with an average node fan-out (out-degree) of $k \ge 4$.

The maximum number of nodes $N(D)$ that can be accommodated in a structured tree of depth $D$ is given by the geometric progression:
$$N(D) = \sum_{d=0}^{D} k^d = \frac{k^{D+1} - 1}{k - 1}$$

Solving for $D$ as a function of $N$:
$$k^{D+1} = N(k - 1) + 1$$
$$D = \log_k \left( N(k-1) + 1 \right) - 1$$

Let $N = 1,000,000$ and $k = 8$ (meaning each relay node uploads to 8 children):
$$D = \log_8 \left( 1,000,000 \cdot (7) + 1 \right) - 1 = \log_8(7,000,001) - 1 \approx 7.58 - 1 \approx 6.58 \text{ hops}$$

Assuming conservative internet routing characteristics:
*   Average inter-peer RTT $RTT_{\text{avg}} = 80\text{ ms}$, meaning $\tau_{\text{prop}} = 40\text{ ms}$.
*   Chunk size is split into $16\text{ KB}$ blocks sent over a $10\text{ Mbps}$ active stream link, meaning $\tau_{\text{trans}} \approx 13\text{ ms}$.
*   Local node processing and queue delay $\tau_{\text{queue}} = 20\text{ ms}$.

The worst-case latency $L$ at depth $D = 7$ hops is:
$$L \approx D \cdot (\tau_{\text{prop}} + \tau_{\text{trans}} + \tau_{\text{queue}}) = 7 \cdot (40\text{ ms} + 13\text{ ms} + 20\text{ ms}) = 7 \cdot 73\text{ ms} = 511\text{ ms}$$

This mathematical proof demonstrates that **a structured P2P overlay can distribute live video to one million concurrent users with an active propagation latency of just $\sim 511\text{ ms}$**, leaving over $4.4\text{ seconds}$ of the playout budget for buffer safety, packet loss recovery, and jitter absorption.
