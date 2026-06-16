# 1.1 Scale and Latency Bounds of Million-Peer Swarms

This subchapter explores the mathematical feasibility of serving one million concurrent viewers. It breaks down the bandwidth costs of centralized CDNs versus the propagation latency limits of P2P swarms.

## Deep Dive Topics:
*   [1. Bandwidth Paradox](1_bandwidth_paradox.md): The $W_{\text{centralized}} = N \cdot B$ equation and the Swarm Sustainability Condition.
*   [2. Propagation Latency Model](2_propagation_latency.md): Playout deadlines and end-to-end hop delay equations.
*   [3. Logarithmic Scaling Proof](3_logarithmic_scaling.md): Mathematical proof that depth scales as $D \le c \log_k(N)$.
*   [4. Failure of Unstructured Swarms](4_unstructured_swarm_failure.md): Why traditional mesh-pull flooding fails low-latency bounds.
