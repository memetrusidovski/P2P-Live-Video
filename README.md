# BitStream Protocol

# Pure P2P Protocol for Million-Scale Live Video

### ⚡ Purpose
This repository contains the design specification for a **decentralized, real-time video streaming network** — tokenless, trackerless, and built to distribute a live stream to **up to 1,000,000 concurrent viewers** without centralized egress costs.

> **📘 The canonical specification lives in [`protocol/`](protocol/).** Start at [`protocol/INDEX.md`](protocol/INDEX.md) for the full chapter map, or [`protocol/README.md`](protocol/README.md) for the table of contents.
>
> The notes in [`thoughts/`](thoughts/) are earlier exploratory material, retained for the reasoning they capture. Where they disagree with `protocol/`, the specification wins.

Open design gaps are tracked in [`issues/`](issues/); the reasoning behind resolved ones — decisions taken, alternatives rejected, residual risk — is recorded in [`solutions/`](solutions/).

---

### 🎯 Core Objectives
- **Low latency:** 3–5 s glass-to-glass (≈ 4.0 s at the reference settings: 250 ms signing chunks, 3.0 s playout buffer, ≤ 7 hops), ≤ 1.5 s startup join.
- **Scalability:** 1,000,000 concurrent viewers at ≤ 7 hops mean depth.
- **Resilience:** sub-second churn recovery (≈ 250–300 ms on local paths, RTT-scaled elsewhere), repaired per tree so one lost parent never stops forwarding on the others; graceful resolution degradation instead of buffering.
- **Fairness:** reward contributors with lower latency and higher quality; leave free-riders at the base layer.
- **Security:** signed manifests, Merkle-verified blocks, Sybil-resistant identities, kernel-level DDoS shielding.

---

### 🕸️ High-Level Architecture
1. **Multi-forest overlay**
   The stream is split into $M$ sub-stream slices carried by $M$ edge-disjoint spanning trees. Each relay-class peer is an interior node in its assigned tree(s) and a leaf elsewhere, so no peer carries the full bitrate upstream. $M$ scales with the relay population (2 → 6), and each tree carries a declared share of the SVC layer ladder.
2. **S/Kademlia discovery**
   256-bit XOR routing with Proof-of-Work-bound node IDs and disjoint parallel lookups for eclipse resistance. Streams are self-authenticating: `StreamID = Blake3(publisher pubkey)`.
3. **HyParView membership**
   Active set (~8 open QUIC connections) plus a passive standby set (~32 addresses) for sub-second repair under churn.
4. **Hybrid push-pull**
   Live-edge segments are pushed proactively down the trees (no request RTT); the trailing 1.5 s of the buffer is repaired by reactive mesh pull.
5. **RaptorQ FEC**
   One source block per 16 KB Merkle block ($K = 16$ symbols of 1024 B), with adaptive 5–30% parity sized per link, so packet loss is repaired locally instead of by retransmission.
6. **Zero-trust verification**
   Every 16 KB block is Blake3 Merkle-verified against a broadcaster-signed manifest *before* it is forwarded. Corrupted data cannot propagate more than one hop.
7. **Tit-for-Tat + Proof-of-Upload**
   500 ms reciprocity cycles with cryptographically signed upload receipts; contribution buys shallower placement and higher SVC layers.
8. **Emergent relays, no TURN**
   Peers behind symmetric NAT are served by community relays recruited with a 3× reputation multiplier — paid in latency, not tokens.

---

### ⚙️ Key Algorithms
- **Parent selection:** multivariate scoring over available capacity, RTT, and hop depth, with warm-up gating and depth admission limits.
- **Topology healing:** deterministic sibling election over a per-tree child roster, falling back to promotion from a per-tree standby pool.
- **Deadline scheduler:** ranks missing blocks by playout urgency and rarity every 100 ms.
- **Capacity adaptation:** when swarm upload cannot sustain full bitrate, SVC enhancement layers are shed in priority order — the base layer never fails.

---

### 🔐 Attack Resilience
| Threat | Mitigation |
|--------|-------------|
| Chunk poisoning | Signed manifests + Merkle verification |
| DoS / spam | Token buckets, rate limits, relay shielding |
| Sybil infiltration | S/Kademlia Proof-of-Work node IDs + subnet diversity limits |
| Free-riding | Tit-for-Tat unchoking, PoU receipts, SVC layer entitlement |
| Replay attacks | Monotonic segment sequence numbers + timestamp drift bounds |

---

### 🧩 Implementation Directions
- **Transport layer:** QUIC over UDP with ICE/STUN hole punching and emergent community relays (no TURN)
- **Coding:** RaptorQ (RFC 6330) systematic fountain codes
- **Verification:** Ed25519 signatures over Blake3 Merkle trees
- **Scheduling:** hybrid push (live edge) / deadline-aware pull (trailing buffer)
- **Simulation:** event-driven network simulation to tune parameters before deployment — see [`protocol/chapter8_simulation/`](protocol/chapter8_simulation/)

---

### 🧠 Future Work
- Reference implementation and the Chapter 8 simulation harness
- Vivaldi synthetic coordinates / ASN-aware peer selection for geographic locality
- Active-active multi-source ingest with signed broadcaster handover
- Security protocol proofs and large-scale adversarial simulation

---

### 💬 Contributing
This repository is a **design specification**, not yet an implementation.
The goal is to refine the architecture, close the tracked issues, and prototype the system.
Issues and pull requests with design notes, literature references, or implementation sketches are welcome.

---

### 📜 License
All content is released for research and educational use under the **MIT License**.

---

> *“A resilient streaming mesh should behave like a living organism — adapting, healing, and evolving under pressure.”*

