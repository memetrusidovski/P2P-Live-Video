# 4. Why Unstructured Swarms Fail

Traditional unstructured mesh-pull protocols (such as BitTorrent-based swarms) are highly resilient but fail the low-latency constraint. In an unstructured swarm, peers exchange bitfields and pull chunks at random. The propagation delay of a chunk in an unstructured gossip mesh scales as:
$$D_{\text{unstructured}} \approx O(N)$$
or at best, under highly optimized flooding:
$$D_{\text{optimized-unstructured}} \approx \alpha \ln(N)$$

where the coefficient $\alpha$ is large due to redundant transmission queries and duplicate packet overhead. Furthermore, since chunks are requested reactively, each hop suffers from a minimum of one full RTT roundtrip query delay, inflating the propagation delay beyond the playout deadline $\Delta_{\text{playout}}$, causing immediate playback collapse. Hence, structuring the overlay is a hard requirement for million-scale low-latency live video.
