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

This mathematical proof demonstrates that **a structured P2P overlay can distribute live video to one million concurrent users with an active propagation latency of just $\sim 511\text{ ms}$**.

## The Glass-to-Glass Budget

Propagation is one term of four, and an earlier draft presented it as if it were the whole latency ("leaving 4.4 s of the budget"). The complete budget from camera to screen:

| Term | Where it comes from | Value |
| :--- | :--- | :---: |
| Encoder | Low-latency 1 s closed-GOP encoding | $0.1$–$0.3$ s |
| Signing barrier | The source signs a chunk's Merkle root after its last block is encoded (Ch4 §4.1.1) | $0.25$ s |
| Source pacing | A chunk's symbols are spread over $\le$ half the chunk period (Ch3 §3.3.2) | $\le 0.125$ s |
| Propagation | $D = 7$ hops as above | $0.51$ s |
| Playout buffer | $\Delta_{\text{buffer}}$ behind the received live edge (Ch4 §4.3.1) | $3.0$ s |
| **Glass-to-glass** | | **$\approx 4.0$–$4.2$ s** |

This meets the $3$–$5$ s objective of §2 with margin for jitter and one repair cycle. Two choices make it fit: the signing unit is a $250$ ms chunk rather than the $1$ s segment (a segment-sized barrier would add $0.75$ s), and the playout buffer is $3.0$ s rather than the $4.0$ s of the earlier draft, which — with the terms it omitted — would have landed at $\approx 5.5$ s.
