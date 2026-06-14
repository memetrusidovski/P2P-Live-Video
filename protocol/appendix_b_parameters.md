# Appendix B: System Parameters and Glossary

## B.1 Centralized System Parameters

To ensure uniform behavior across different implementations, the protocol defines a set of strict, hardcoded default constants.

### B.1.1 Timeouts & Intervals
| Parameter | Default Value | Unit | Description |
| :--- | :---: | :---: | :--- |
| $\tau_{\text{ping}}$ | $100$ | ms | Heartbeat ping interval over active QUIC streams |
| $\tau_{\text{evict}}$ | $200$ | ms | Active parent eviction timeout (2 missed pings) |
| $\tau_{\text{tft}}$ | $500$ | ms | Tit-for-Tat unchoking evaluation cycle |
| $\tau_{\text{gossip}}$ | $1000$ | ms | Buffer state bitfield gossip interval |
| $\tau_{\text{ttl}}$ | $180$ | s | Time-to-Live for dynamic S/Kademlia peer registrations |

### B.1.2 Overlay Size Constraints
| Parameter | Default Value | Unit | Description |
| :--- | :---: | :---: | :--- |
| $c_a$ | $8$ | nodes | Target size of HyParView Active Set $\mathcal{A}$ |
| $c_p$ | $32$ | nodes | Target size of HyParView Passive Set $\mathcal{P}$ |
| $k$ | $20$ | nodes | S/Kademlia k-bucket capacity |
| $M$ | $6$ | trees | Number of independent sub-stream slices (forest size) |
| $D_{\text{max}}$ | $8$ | hops | Maximum permitted routing depth from source |

### B.1.3 Cryptographic & Encoding Puzzles
| Parameter | Default Value | Unit | Description |
| :--- | :---: | :---: | :--- |
| $C_1$ | $16$ | bits | S/Kademlia static Proof-of-Work prefix requirement |
| $C_2$ | $12$ | bits | S/Kademlia dynamic Proof-of-Work (IP-bound) requirement |
| $\text{Size}_{\text{block}}$| $16$ | KB | Size of individual data blocks in Merkle trees |
| $\text{Overhead}_{\text{fec}}$| $10\%$ | percent | Systematic RaptorQ redundancy overhead ceiling |

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
