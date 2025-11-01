# P2P Real-Time Streaming Mesh — Research & Design Notes

### ⚡ Purpose
This repository collects research and design ideas for a **decentralized, real-time video streaming network** built on a **peer-to-peer mesh**.  
The goal: achieve near real-time distribution of HLS (or similar) video chunks to **thousands of peers** — efficiently, securely, and without centralized bottlenecks.

The ideas here outline algorithms, topology strategies, and security models that allow the network to **self-optimize**, **resist attacks**, and **adapt to network conditions**.

---

### 🎯 Core Objectives
- **Low latency:** deliver HLS segments in near real time (2–10 s latency budget).  
- **Scalability:** support 1,000+ peers with efficient bandwidth use.  
- **Resilience:** recover quickly from churn, failures, and attacks.  
- **Fairness:** reward good uploaders, demote weak or malicious peers.  
- **Security:** protect against poisoning, Sybil, and DoS attacks with signed manifests and reputation.

---

### 🕸️ High-Level Architecture
1. **Hybrid mesh topology**  
   Peers form local clusters based on latency and capacity. Clusters interconnect through stable “super-peers” or lightweight relays.  
2. **Fountain/erasure coding**  
   Each video segment is split into small symbols, encoded (e.g., RaptorQ). Any subset of M symbols can reconstruct the segment.  
3. **Deadline-aware swarming**  
   Peers prioritize symbol requests by urgency and rarity, keeping the buffer full before playback deadlines.  
4. **Pull-dominant with optimistic push**  
   Peers mostly request missing data (“pull”) but may opportunistically push symbols to improve propagation speed.  
5. **Content authentication**  
   Segments are described by signed manifests and Merkle-verified symbols to prevent pollution or tampering.  
6. **Adaptive topology control**  
   Peers are scored continuously (upload rate, reliability, latency, contribution). High-score peers move closer to the source; weak or malicious nodes are gradually ignored.  
7. **Reputation-driven clustering**  
   Reputation and performance determine cluster membership and super-peer promotion.  
8. **Security hardening**  
   - Signed manifests and Merkle proofs  
   - Rate limiting & proof-of-work for new peers  
   - Encrypted transports (DTLS/TLS)  
   - Gossip-based local reputation rather than global trust lists  

---

### ⚙️ Key Algorithms
- **Peer scoring formula** combines upload capacity, reliability, latency, availability, and contribution ratio.  
- **Promotion/demotion loop** continuously reshapes the overlay:  
  - Good peers gain connections and move closer to the source.  
  - Bad peers lose neighbors and drop out.  
- **Cluster maintenance** keeps local mesh sizes stable (e.g., 50–200 peers).  
- **Deadline scheduler** prioritizes missing symbols by urgency, rarity, and network cost.  
- **Erasure coding layer** provides redundancy against loss and churn.  

---

### 🔐 Attack Resilience
| Threat | Mitigation |
|--------|-------------|
| Chunk poisoning | Signed manifests + Merkle verification |
| DoS / spam | Token buckets, rate limits, relay shielding |
| Sybil infiltration | Join tokens or proof-of-work |
| Free-riding | Contribution-based scoring & throttling |
| Replay attacks | Short-lived segment IDs |

---

### 🧩 Implementation Directions
- **Transport layer:** WebRTC or libp2p with STUN/TURN fallback  
- **Coding:** RaptorQ or Reed-Solomon  
- **Verification:** Ed25519 signatures, Merkle trees  
- **Scheduling:** adaptive pull with pipelined requests  
- **Simulation:** event-driven network to tune parameters (cluster size, FEC overhead, latency)  

---

### 🧠 Future Work
- Detailed message/state-machine spec (bitfields, gossip messages, request/response formats)  
- Peer reputation propagation models  
- Cluster balancing under churn  
- Integration with WebRTC data channels or QUIC  
- Security protocol proofs and performance simulations  

---

### 💬 Contributing
This repository is currently a **concept collection**.  
The goal is to refine the architecture, define core algorithms, and prototype the system.  
Issues and pull requests with design notes, literature references, or implementation sketches are welcome.

---

### 📜 License
All content is released for research and educational use under the **MIT License**.

---

> *“A resilient streaming mesh should behave like a living organism — adapting, healing, and evolving under pressure.”*

