# Pure P2P Protocol for Million-Scale Live Video Streaming
## A Technical Specification and Protocol Book

Welcome to the official protocol design specification. This folder contains the modular design of a purely decentralized, tokenless live-streaming protocol designed to support up to 1,000,000 concurrent viewers with low latency.

---

## Table of Contents

*   **[Foreword: The Philosophy of Decentralized Real-Time Broadcast](#foreword-the-philosophy-of-decentralized-real-time-broadcast)**
*   **[Chapter 1: Protocol Foundations and Mathematical Architecture](chapter1_foundations.md)**
    *   1.1 Scale and Latency Bounds of Million-Peer Swarms
    *   1.2 Mathematical Formulation of Dynamic Multi-Forest Overlays
    *   1.3 State-Machine Representation of Peer Life Cycle
*   **[Chapter 2: Peer Discovery and Stream Registration](chapter2_discovery.md)**
    *   2.1 Bootstrapping and Stream Discovery over S/Kademlia DHT
    *   2.2 S/Kademlia Cryptographic ID Generation & Proof-of-Work
    *   2.3 Store/Retrieve Protocols for Stream Signatures
*   **[Chapter 3: Overlay Membership and Epidemic Gossip (HyParView)](chapter3_membership.md)**
    *   3.1 Active and Passive Neighbor Sets: Mechanics of Connection Maintenance
    *   3.2 Gossip-based Topology Synchronization: Protocol Frame Formats
    *   3.3 Dynamic Churn Recovery: Sub-Second Re-Routing Strategies
*   **[Chapter 4: Media Distribution and Deadline-Aware Swarming](chapter4_distribution.md)**
    *   4.1 Segment Serialization and Blake3 Merkle-Tree Encoding
    *   4.2 Forward Error Correction (FEC) Layer: RaptorQ Packetization
    *   4.3 Hybrid Push-Pull Scheduling Logic and Buffer State-Bitfield Formats
*   **[Chapter 5: Economic Incentives and Fair-Share Swarm Contribution](chapter5_incentives.md)**
    *   5.1 Adaptation-Aware Tit-For-Tat (TFT) Game Theory
    *   5.2 Cryptographic Proof-of-Upload (PoU) Receipt Architecture
    *   5.3 Local Reputation Gossip Auditing (Sybil and Collusion Resistance)
*   **[Chapter 6: NAT Traversal and Emergent Relaying Systems](chapter6_traversal.md)**
    *   6.1 UDP Hole Punching under Symmetric and CGNAT Topologies
    *   6.2 QUIC Connection Migration and STUN/ICE Negotiation
    *   6.3 Autonomous Relayer Selection and Crowd-Sourced Superpeer Incentivization
*   **[Chapter 7: Security Hardening, Cryptography, and Threat Mitigation](chapter7_security.md)**
    *   7.1 Signed Manifests and Source Identity Pinning (Ed25519)
    *   7.2 DDoS Shielding: Relay Clusters and Token Bucket Rate-Limiting
    *   7.3 Privacy Routing: Multi-Hop Onion-Style Layering (Optional)

---

## Foreword: The Philosophy of Decentralized Real-Time Broadcast

The centralized architecture of Content Delivery Networks (CDNs) operates on a simple, expensive premise: to serve $N$ viewers, the system must deploy resources proportional to $O(N)$. At million-user scales, this translates to massive financial expenditures and significant single points of failure. The peer-to-peer paradigm shifts this cost, turning every consumer of a stream into an active distributor.

This book describes the design of a purely decentralized, tokenless, and trackerless live-streaming protocol. We proceed from the core engineering axiom that **network contribution and playback performance must be inextricably linked**. No central server coordinates this swarm; instead, we rely on local, emergent behaviors driven by rigorous cryptographic verification and strict game-theoretic reciprocity.
