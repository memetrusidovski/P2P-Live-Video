# 5.3 Local Reputation Gossip Auditing

This subchapter details the defenses used to identify and mathematically neutralize networks of Sybil nodes attempting to manipulate the reputation ledger.

## Deep Dive Topics:
*   [1. Graph Collusion Auditing](1_graph_collusion_auditing.md): Catching "Mutual Backscratching" via symmetric edge analysis.
*   [2. IP Subnet Penalties](2_ip_subnet_penalties.md): Penalizing /24 IPv4 and /48 IPv6 Sybil clusters.
*   [3. Consensus Eviction](3_consensus_eviction.md): `REPUTATION_AUDIT_GOSSIP` frames and network ban rules.
