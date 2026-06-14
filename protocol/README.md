# Pure P2P Protocol for Million-Scale Live Video Streaming
## A Technical Specification and Protocol Book

Welcome to the official protocol design specification. This folder contains the modular design of a purely decentralized, tokenless live-streaming protocol designed to support up to 1,000,000 concurrent viewers with low latency.

---

## Table of Contents

*   **[Foreword: The Philosophy of Decentralized Real-Time Broadcast](#foreword-the-philosophy-of-decentralized-real-time-broadcast)**
*   **[Chapter 1: Protocol Foundations and Mathematical Architecture](chapter1/README.md)**
    *   [1.1 Scale and Latency Bounds of Million-Peer Swarms](chapter1/1.1_scale_latency.md)
    *   [1.2 Mathematical Formulation of Dynamic Multi-Forest Overlays](chapter1/1.2_multi_forest_overlays.md)
    *   [1.3 State-Machine Representation of Peer Life Cycle](chapter1/1.3_peer_lifecycle.md)
*   **[Chapter 2: Peer Discovery and Stream Registration](chapter2/README.md)**
    *   [2.1 Bootstrapping and Stream Discovery over S/Kademlia DHT](chapter2/2.1_skademlia_dht.md)
    *   [2.2 S/Kademlia Cryptographic ID Generation & Proof-of-Work](chapter2/2.2_cryptographic_node_ids.md)
    *   [2.3 Store/Retrieve Protocols for Stream Signatures](chapter2/2.3_store_retrieve_protocols.md)
*   **[Chapter 3: Overlay Membership and Epidemic Gossip (HyParView)](chapter3/README.md)**
    *   [3.1 Active and Passive Neighbor Sets: Mechanics of Connection Maintenance](chapter3/3.1_active_passive_sets.md)
    *   [3.2 Gossip-based Topology Synchronization: Protocol Frame Formats](chapter3/3.2_gossip_protocols.md)
    *   [3.3 Dynamic Churn Recovery: Sub-Second Re-Routing Strategies](chapter3/3.3_churn_recovery.md)
*   **[Chapter 4: Media Distribution and Deadline-Aware Swarming](chapter4/README.md)**
    *   [4.1 Segment Serialization and Blake3 Merkle-Tree Encoding](chapter4/4.1_segment_serialization.md)
    *   [4.2 Forward Error Correction (FEC) Layer: RaptorQ Packetization](chapter4/4.2_fec_raptorq.md)
    *   [4.3 Hybrid Push-Pull Scheduling Logic and Buffer State-Bitfield Formats](chapter4/4.3_push_pull_scheduling.md)
*   **[Chapter 5: Economic Incentives and Fair-Share Swarm Contribution](chapter5/README.md)**
    *   [5.1 Adaptation-Aware Tit-For-Tat (TFT) Game Theory](chapter5/5.1_tit_for_tat.md)
    *   [5.2 Cryptographic Proof-of-Upload (PoU) Receipt Architecture](chapter5/5.2_proof_of_upload.md)
    *   [5.3 Local Reputation Gossip Auditing (Sybil and Collusion Resistance)](chapter5/5.3_reputation_auditing.md)
*   **[Chapter 6: NAT Traversal and Emergent Relaying Systems](chapter6/README.md)**
    *   [6.1 UDP Hole Punching under Symmetric and CGNAT Topologies](chapter6/6.1_udp_hole_punching.md)
    *   [6.2 QUIC Connection Migration and STUN/ICE Negotiation](chapter6/6.2_quic_migration_stun_ice.md)
    *   [6.3 Autonomous Relayer Selection and Crowd-Sourced Superpeer Incentivization](chapter6/6.3_emergent_relays.md)
*   **[Chapter 7: Security Hardening, Cryptography, and Threat Mitigation](chapter7/README.md)**
    *   [7.1 Signed Manifests and Source Identity Pinning (Ed25519)](chapter7/7.1_signed_manifests.md)
    *   [7.2 DDoS Shielding: Relay Clusters and Token Bucket Rate-Limiting](chapter7/7.2_ddos_shielding.md)
    *   [7.3 Privacy Routing: Multi-Hop Onion-Style Layering (Optional)](chapter7/7.3_privacy_routing.md)

---

## Foreword: The Philosophy of Decentralized Real-Time Broadcast

The centralized architecture of Content Delivery Networks (CDNs) operates on a simple, expensive premise: to serve $N$ viewers, the system must deploy resources proportional to $O(N)$. At million-user scales, this translates to massive financial expenditures and significant single points of failure. The peer-to-peer paradigm shifts this cost, turning every consumer of a stream into an active distributor.

This book describes the design of a purely decentralized, tokenless, and trackerless live-streaming protocol. We proceed from the core engineering axiom that **network contribution and playback performance must be inextricably linked**. No central server coordinates this swarm; instead, we rely on local, emergent behaviors driven by rigorous cryptographic verification and strict game-theoretic reciprocity.
