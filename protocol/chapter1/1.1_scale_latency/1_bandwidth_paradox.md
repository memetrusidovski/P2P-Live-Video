# 1. The Bandwidth Paradox of Live Swarms

In a centralized Content Delivery Network (CDN), the operational cost of video streaming scales linearly as $O(N)$ with the number of concurrent viewers $N$. If the video stream's target encoding bitrate is $B$ (e.g., $B = 6\text{ Mbps}$ for a typical 1080p60 AVC/H.264 live feed), the infrastructure must sustain a massive egress bandwidth:
$$W_{\text{centralized}} = N \cdot B$$

For $N = 1,000,000$ concurrent viewers, the centralized egress bandwidth is:
$$W_{\text{centralized}} = 1,000,000 \cdot 6\text{ Mbps} = 6,000,000\text{ Mbps} = 6\text{ Tbps}$$

The economic cost of maintaining a 6 Tbps pipeline on public cloud CDNs makes independent or large-scale broadcasting financially prohibitive. In contrast, a peer-to-peer (P2P) model treats the viewers themselves as edge relays. Let $u_i$ represent the upstream upload capacity of peer $i$, and $d_i$ represent its downstream download capacity. For the swarm to remain fully self-sustaining without CDN fallbacks, the system must satisfy the **Swarm Sustainability Condition**:
$$\sum_{i=1}^{N} u_i \ge N \cdot B$$

In residential internet connections, upload and download capacities are typically asymmetric (e.g., a "gigabit fiber" plan often provides $1000\text{ Mbps}$ download but only $50\text{ Mbps}$ upload). Thus, while $d_i \gg B$ is almost universally true, $u_i$ can vary wildly from $0\text{ Mbps}$ (mobile connections under cellular limits) to $1000\text{ Mbps}$ (symmetrical FTTH).

Two consequences follow, and both are load-bearing for the rest of this chapter:

*   **Sustainability and fan-out are different bars.** Meeting $\sum u_i \ge N \cdot B$ (mean $\bar{u} \ge 6\text{ Mbps}$) makes full quality *deliverable*; the $D \le 7$ depth proof of §3 additionally assumes a mean fan-out of $k = 8$, i.e. $\bar{u} \ge 8\text{ Mbps}$. A swarm can satisfy the first and miss the second, in which case quality holds but trees deepen.
*   **The condition can fail.** Given real upload distributions it frequently will. The protocol does not assume it away: [§5 Capacity Adaptation](5_capacity_adaptation.md) defines the degraded-operation ladder — per-tree saturation detection, SVC layer shedding in manifest-priority order, $D_{\text{max}}$ overflow behaviour, and the source-side base-layer reserve that bounds the worst case.
