# 2.1 Bootstrapping and Stream Discovery over S/Kademlia DHT

This subchapter details the initial entry phase of a node into the network, focusing on routing tables, Sybil-resistant lookup mechanics, and the initial connection steps.

## Deep Dive Topics:
*   [1. k-Bucket Prefix Matching](1_k_bucket_prefix_matching.md): The mathematics of XOR distance and 256-bit routing tables.
*   [2. Disjoint Parallel Lookups](2_disjoint_parallel_lookups.md): Eclipse attack resistance math ($P_{\text{hijack}} \le f^\alpha$).
*   [3. Bootstrap Sequence](3_bootstrap_sequence.md): Hardcoded seed querying and self-lookup handshakes.
