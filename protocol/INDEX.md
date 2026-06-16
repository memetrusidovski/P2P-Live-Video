# Master Book Index: Pure P2P Protocol for Million-Scale Live Video

This index defines the complete, granular folder structure of the protocol specification. Every subchapter is an independent directory containing specific deep-dive technical documents.

## Chapter 1: Protocol Foundations and Mathematical Architecture
*   **`1.1_scale_latency/`**
    *   `1_bandwidth_paradox.md`: Egress math and swarm sustainability conditions.
    *   `2_propagation_latency.md`: Playout deadlines and end-to-end delay equations.
    *   `3_logarithmic_scaling.md`: Mathematical proof of $D \le c \log_k(N)$.
    *   `4_unstructured_swarm_failure.md`: Why mesh-pull flooding fails low-latency bounds.
*   **`1.2_multi_forest_overlays/`** *(Previously 1.2_multi_forest_deep_dive)*
    *   `1_graph_theory_and_slicing.md`: Edge-disjoint spanning trees and the Orthogonal Placement Rule.
    *   `2_parent_selection_algorithm.md`: Multivariate scoring function and tree join algorithm.
    *   `3_topology_healing.md`: Sub-250ms deterministic lateral sibling election.
    *   `4_stream_slicing_architecture.md`: Graceful degradation using SVC and MDC.
*   **`1.3_peer_lifecycle/`**
    *   `1_transition_model.md`: State-machine definition (`BOOTSTRAP` to `TERMINATED`).
    *   `2_algorithmic_core_loop.md`: The microsecond event-driven execution loop.
    *   `3_connection_handover.md`: Mobile tolerance and physical layer transit.

## Chapter 2: Peer Discovery and Stream Registration (S/Kademlia)
*   **`2.1_skademlia_routing/`**
    *   `1_k_bucket_prefix_matching.md`: 256-bit XOR metric space routing tables.
    *   `2_disjoint_parallel_lookups.md`: Eclipse attack resistance math ($P_{\text{hijack}} \le f^\alpha$).
    *   `3_bootstrap_sequence.md`: Hardcoded seed querying and self-lookup handshakes.
*   **`2.2_crypto_node_id/`**
    *   `1_static_dynamic_puzzles.md`: Blake3 Proof-of-Work bound to IP addresses.
    *   `2_sybil_defense_math.md`: Difficulty parameters ($C_1$, $C_2$) and attack cost analysis.
    *   `3_validation_frame.md`: Secure S/Kademlia header byte layout.
*   **`2.3_stream_registration/`**
    *   `1_publisher_genesis_key.md`: Decoupling StreamID from human-readable names.
    *   `2_store_get_rpcs.md`: Registration and query timelines over the DHT.
    *   `3_registration_frames.md`: `REGISTER_PEER` and `GET_PEERS` packet layouts.

## Chapter 3: Overlay Membership and Epidemic Gossip (HyParView)
*   **`3.1_neighbor_sets/`**
    *   `1_memory_structures.md`: Sizing and rules for Active ($\mathcal{A}$) vs Passive ($\mathcal{P}$) sets.
    *   `2_random_walk_propagation.md`: `FORWARD_JOIN` TTLs and small-world clustering.
    *   `3_degree_distribution.md`: Mathematical guarantees of graph connectivity.
*   **`3.2_topology_gossip/`**
    *   `1_shuffle_neighbor_frames.md`: Binary layouts for `NEIGHBOR`, `SHUFFLE`, `DISCONNECT`.
    *   `2_hmac_security.md`: Transient session keys and replay-sequence tracking.
*   **`3.3_churn_recovery/`**
    *   `1_recovery_timeline.md`: The 0-250ms active connection drop and repair timeline.
    *   `2_active_probing.md`: Keepalive heartbeats and immediate eviction rules.

## Chapter 4: Media Distribution and Deadline-Aware Swarming
*   **`4.1_segment_serialization/`**
    *   `1_gop_serialization.md`: 1.0s closed-GOP constraints and bitrates.
    *   `2_progressive_blake3_hashing.md`: Zero-trust Merkle tree block verification.
    *   `3_verification_frame.md`: `BLOCK_TRANSMISSION` packet layout with proof paths.
*   **`4.2_fec_raptorq/`**
    *   `1_generator_matrix_math.md`: RFC 6330 Fountain code probabilities and encoding symbols.
    *   `2_systematic_dispersion.md`: Distributing 10% parity across multi-forest parents.
    *   `3_symbol_packet_layout.md`: `RAPTORQ_SYMBOL` byte alignments (SBN and ESI).
*   **`4.3_hybrid_push_pull/`**
    *   `1_buffer_sliding_timeline.md`: The exact boundary between the PUSH zone and PULL zone.
    *   `2_bitfield_frames.md`: Compressed Bloom filters and bitfield availability arrays.
    *   `3_rarest_first_heuristics.md`: The chunk-urgency algorithm for reactive pulling.

## Chapter 5: Economic Incentives and Fair-Share Contribution
*   **`5.1_tit_for_tat/`**
    *   `1_prisoner_dilemma_math.md`: Nash equilibrium proofs and discount factor bounds.
    *   `2_sliding_window_unchoker.md`: The 500ms reciprocity calculation loop.
    *   `3_optimistic_exploration.md`: Bandwidth allocation for newly joined peers.
*   **`5.2_proof_of_upload/`**
    *   `1_receipt_cryptography.md`: Non-repudiable Ed25519 PoU payload structure.
    *   `2_exponential_decay_scoring.md`: The global reputation math formula ($\Theta_A$).
    *   `3_pou_frame.md`: `PROOF_OF_UPLOAD` binary layout.
*   **`5.3_reputation_auditing/`**
    *   `1_graph_collusion_auditing.md`: Catching "Mutual Backscratching" via symmetric edge analysis.
    *   `2_ip_subnet_penalties.md`: Penalizing /24 IPv4 and /48 IPv6 Sybil clusters.
    *   `3_consensus_eviction.md`: `REPUTATION_AUDIT_GOSSIP` frames and network ban rules.

## Chapter 6: NAT Traversal and Emergent Relays
*   **`6.1_udp_hole_punching/`**
    *   `1_nat_permutation_matrix.md`: Success probabilities across FC, RC, PRC, and CGNAT.
    *   `2_socket_bind_reuse.md`: `SO_REUSEPORT` requirements for STUN mapping consistency.
*   **`6.2_quic_ice/`**
    *   `1_candidate_gathering.md`: STUN/ICE bidirectional probing loops.
    *   `2_64bit_connid_migration.md`: Seamless physical interface handovers in QUIC.
*   **`6.3_emergent_relays/`**
    *   `1_recruitment_criteria.md`: Uptime and symmetrical bandwidth rules for Superpeers.
    *   `2_3x_multiplier_economics.md`: How the system pays relays with premium stream latency.
    *   `3_relay_frames.md`: `RELAY_PROPOSAL` and `RELAY_BIND` layouts.

## Chapter 7: Security Hardening & Threat Mitigation
*   **`7.1_source_pinning/`**
    *   `1_genesis_key_anchoring.md`: Binding stream IDs to the broadcaster's Ed25519 identity.
    *   `2_replay_attack_prevention.md`: Sequence caching and absolute timestamp drift limits.
*   **`7.2_ddos_shielding/`**
    *   `1_token_bucket_math.md`: Egress and ingress rate limiting parameters.
    *   `2_xdp_kernel_ebpf.md`: The NIC-level C-code filter to drop floods before user-space.
*   **`7.3_privacy_routing/`**
    *   `1_chacha20_envelope_math.md`: Nested onion encryption through 3-hop proxies.
    *   `2_onion_latency_tradeoffs.md`: Why privacy routing forces peers into Leaf-Only states.

## Appendices
*   `appendix_a_sequence_diagrams.md`
*   `appendix_b_parameters.md`
*   `appendix_c_threat_model.md`
*   `schemas/p2p_live.proto`
*   `chapter8_simulation/README.md`
