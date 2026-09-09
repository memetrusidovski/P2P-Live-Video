# Appendix B: System Parameters and Glossary

## B.1 Centralized System Parameters

To ensure uniform behavior across different implementations, the protocol defines a set of strict, hardcoded default constants.

### B.1.1 Timeouts & Intervals
| Parameter | Default Value | Unit | Description |
| :--- | :---: | :---: | :--- |
| $\tau_{\text{ping}}$ | $100$ | ms | Heartbeat ping interval over active QUIC streams |
| $\tau_{\text{evict}}$ | $200$ | ms | Active parent eviction timeout (2 missed pings) |
| $\tau_{\text{tft}}$ | $500$ | ms | Tit-for-Tat unchoking evaluation cycle |
| $\tau_{\text{gossip}}$ | $1000$ | ms | Buffer state bitfield gossip interval (peer-to-peer exchange) |
| $\tau_{\text{sched}}$ | $100$ | ms | Buffer scheduler / reactive PULL evaluation cycle (Ch4 §4.3.3) — distinct from $\tau_{\text{gossip}}$ |
| $\tau_{\text{roster}}$ | $1000$ | ms | Child-roster distribution interval for sibling election (Ch1 §1.2.3); rosters stale after $5$ s |
| $\tau_{\text{deputy}}$ | $45$ | ms | Orphan's Deputy-response timer before independent passive-set fallback (Ch1 §1.2.3) |
| $\tau_{\text{ttl}}$ | $180$ | s | Time-to-Live for dynamic S/Kademlia peer registrations |

### B.1.2 Overlay Size Constraints
| Parameter | Default Value | Unit | Description |
| :--- | :---: | :---: | :--- |
| $c_a$ | $8$ | nodes | Target size of HyParView Active Set $\mathcal{A}$; relays with $K_v > 2 c_a$ scale to $c_a^{\text{eff}} = \min(64, \lfloor K_v/10 \rfloor)$ (Ch3 §3.1) |
| $c_p$ | $32$ | nodes | Target size of HyParView Passive Set $\mathcal{P}$ |
| $k$ | $20$ | nodes | S/Kademlia k-bucket capacity |
| $M_{\text{max}}$ | $6$ | trees | Maximum number of independent sub-stream slices; the active forest size $M$ is dynamic, scaling with swarm size $N$ on the ladder defined in Ch1 §1.3.1 ($M{=}1$ at $N{<}6$ up to $M{=}6$ at $N{\ge}30$) |
| $D_{\text{max}}$ | $8$ | hops | Maximum permitted routing depth from source |
| $N_{\text{collusion}}$ | $20$ | nodes | Minimum swarm size before collusion/subnet reputation heuristics activate (Ch5 §5.3) |
| $\sigma_{\text{target}}$ | $1.33$ | ratio | Capacity ratio at which the $D \le 7$ depth proof holds (mean upload $\ge 8$ Mbps) (Ch1 §1.1.5) |
| Shed rounds | $2$ | rounds | Consecutive failed tree-join rounds before shedding an SVC layer (Ch1 §1.1.5) |
| Shed hysteresis | $10$ | s | Cool-down before a shed tree is retried (Ch1 §1.1.5) |
| Source reserve | $3 \cdot B_1$ | Mbps | Broadcaster upload held back as the Tree-1 base-layer emergency pool (Ch1 §1.1.5) |
| NodeClass | `0x00`/`0x01`/`0x02` | code | `RELAY` / `LEAF` / `LEAF_PRIVATE` (Ch1 §1.2.5) |
| Leaf live-edge offset | $5$–$10$ | s | How far behind the live edge leaf-class peers join (Ch1 §1.2.5) |
| Service floor cap | $20\%$ | percent | Maximum share of a node's upload slots committed to base-layer delivery for non-contributing peers (Ch5 §5.1) |

### B.1.3 Cryptographic & Encoding Puzzles
| Parameter | Default Value | Unit | Description |
| :--- | :---: | :---: | :--- |
| $C_1$ | $16$ | bits | S/Kademlia static Proof-of-Work prefix requirement |
| $C_2$ | $12$ | bits | S/Kademlia dynamic Proof-of-Work (IP-bound) requirement — adaptive by swarm size: $8/10/12/14$ at $N < 50 / 10^3 / 10^5 / \ge 10^5$ (Ch2 §2.2); halved on same-subnet reconnect |
| $\text{Size}_{\text{block}}$| $16$ | KB | Size of individual data blocks in Merkle trees; equals one RaptorQ source block (Ch4 §4.2) |
| $T_{\text{symbol}}$ | $1024$ | bytes | RaptorQ symbol size (fixed, anti-fragmentation) |
| $K_{\text{block}}$ | $16$ | symbols | Source symbols per 16 KB block; SBN = Merkle block index |
| $E_{\text{min}}$ | $1$ | symbol | Minimum parity symbols per block per link |
| $\text{Overhead}_{\text{fec}}$| $5$–$30\%$ | percent | Adaptive systematic RaptorQ redundancy, $E = \max(1, \lceil\text{clamp}(2\rho K, 0.05K, 0.30K)\rceil)$ per link (default ~10% at typical loss) |

### B.1.4 Kernel Filter (XDP/eBPF DDoS Shield)
| Parameter | Default Value | Unit | Description |
| :--- | :---: | :---: | :--- |
| `RATE_PPS_PEER` | $2000$ | pps | Token-bucket rate for allowlisted peers (~750 pps steady state + PULL/FEC headroom) |
| `BURST_PEER` | $1000$ | packets | Bucket capacity for allowlisted peers (absorbs QUIC bursts) |
| `RATE_PPS_UNKNOWN` | $10$ | pps | Per-source budget for un-allowlisted first contact |
| `GLOBAL_NEW_PPS` | $5000$ | pps | Global ceiling on new-connection work from all unknown sources |
| `MAX_PEERS` | $65536$ | entries | Allowlist map size (super node neighbor set + churn headroom) |
| `MAX_UNKNOWN` | $16384$ | entries | LRU map size for unknown-source budgets |

---

## B.2 Glossary of Terms

*   **Active Set ($\mathcal{A}$):** The local collection of peers with whom a node maintains active, bidirectional transport connections.
*   **Carrier-Grade NAT (CGNAT):** A form of large-scale address translation deployed by ISPs where multiple consumer households share a single public IPv4 address, restricting inbound P2P hole punching.
*   **Choke:** The state where a peer refuses to upload stream symbols to a specific neighbor while maintaining logical connection states.
*   **GOP (Group of Pictures):** A sequence of consecutive video frames starting with an Intra-frame (I-frame) that is fully self-contained and independent.
*   **HyParView:** A hybrid partial membership protocol that maintains active and passive views to manage network connectivity under extreme churn.
*   **Merkle Path Proof:** The set of sister hashes required to mathematically verify that a specific leaf node belongs to a cryptographically signed Merkle Tree root.
*   **Multi-Forest:** A routing topology consisting of multiple independent, parallel distribution trees designed to split stream bitrates.
*   **Onion Routing:** A privacy-preserving routing technique where messages are encapsulated in nested encrypted layers, obscuring IP identities.
*   **Passive Set ($\mathcal{P}$):** A standby pool of peer contacts cached in memory used to immediately replace failed active set connections.
*   **Proof-of-Upload (PoU):** A cryptographically signed receipt issued by a download recipient to an uploader verifying data delivery.
*   **RaptorQ:** A systematic fountain code used for Forward Error Correction to recover from packet drops without retransmission roundtrips.
*   **S/Kademlia:** A secure extension of the Kademlia Distributed Hash Table that incorporates Proof-of-Work puzzles to neutralize Sybil attacks.
*   **Tit-for-Tat (TFT):** A game-theoretic reciprocity strategy where nodes prioritize uploading to neighbors who actively upload back.
