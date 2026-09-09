# 1.2 Mathematical Formulation of Dynamic Multi-Forest Overlays

To optimize latency and prevent network interface saturation, the swarm does not form a single, monolithic distribution tree. Instead, the video stream is split into $M$ independent sub-streams (slices). The overlay network is structured as a **Multi-Forest** consisting of $M$ independent, self-healing distribution trees.

Because the mechanics of establishing, balancing, and healing these independent trees represent the core algorithmic complexity of this protocol, the complete technical specification has been broken down into a dedicated five-part deep dive.

## Detailed Multi-Forest Specifications

Please review the following sub-documents for the exact mathematical models, parent-selection pseudocode, and sub-second healing timelines:

*   **[1. Graph-Theoretic Foundations & Slice Assignment](1_graph_theory_and_slicing.md):** Defines the directed graph $G=(V, E)$, the Orthogonal Placement Rule capacity constraints, and edge-disjoint spanning trees.
*   **[2. The Parent Selection & Scoring Algorithm](2_parent_selection_algorithm.md):** The multivariate capacity/latency/hop-count scoring formula and the algorithmic pseudocode for querying and joining a tree.
*   **[3. Topology Healing (Sub-250ms Sibling Election)](3_topology_healing.md):** The exact millisecond-by-millisecond timeline and deterministic lateral election logic used to repair a broken tree branch without querying the DHT.
*   **[4. Stream Slicing Architecture (MDC & SVC)](4_stream_slicing_architecture.md):** How the video bytes are actually divided across trees using Scalable Video Coding (SVC) or Multiple Description Coding (MDC) to ensure graceful degradation (dropping resolution) instead of playback freezing during network failures.
*   **[5. Node Classes and Relay Eligibility](5_node_classes.md):** The `RELAY` / `LEAF` / `LEAF_PRIVATE` taxonomy — who bears relay duty, why Proof-of-Work is universal, and how leaf-only status is priced in quality of service rather than exempted from the incentive system.