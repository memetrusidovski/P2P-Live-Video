# Appendix C: STRIDE Threat Model

This appendix maps the key vulnerabilities and design defenses of the protocol using the industry-standard **STRIDE** methodology (Spoofing, Tampering, Repudiation, Information Disclosure, Denial of Service, Elevation of Privilege).

---

## C.1 The STRIDE Defense Matrix

| Threat Category | Core Vector | Impact | Specific Protocol Mitigations |
| :--- | :--- | :--- | :--- |
| **S**poofing | Attacker claims a false identity or hijacks another peer's NodeID. | Eclipse attacks, routing redirection, reputation spoofing. | **S/Kademlia Cryptographic ID (Chapter 2):** NodeIDs are cryptographically bound to public keys and physical IPs via Static ($C_1 = 16$) and Dynamic ($C_2 = 12$) Proof-of-Work puzzles. |
| **T**ampering | Malicious node alters packet payloads or injects corrupted video blocks. | Video artifacts, player crashes, downstream pollution. | **Blake3 Merkle progressive validation (Chapter 4):** Video blocks carry sister hashes to verify integrity against signed roots in microseconds, discarding poisoned blocks on the fly. |
| **R**epudiation | Peer downloads chunks but denies receiving them to avoid contributing. | Free riding, unfair slot allocation, swarm collapse. | **Proof-of-Upload (PoU) receipts (Chapter 5):** Receivers must return non-repudiable Ed25519 signatures validating receipt. Failure to return PoUs triggers immediate choking. |
| **I**nformation Disclosure | Eavesdropper monitors peer connections and harvests IP addresses. | Viewer tracking, localized ISP censorship. | **Onion Privacy Tunnels (Chapter 7):** Clients can optionally wrap payloads in three-layered ChaCha20-Poly1305 envelopes routed through randomized entry/middle/exit relays. |
| **D**enial of Service | Botnets flood superpeers or query DHT nodes with garbage traffic. | Socket saturation, bootstrap failures, network partitioning. | **eBPF XDP NIC Rate Limiting (Chapter 7):** Incoming packets are filtered in kernel space using token-bucket filters, dropping flooding packets before allocating memory buffers. |
| **E**levation of Privilege | Rogue node attempts to spoof the broadcaster's key and take over root streams. | Swarm hijacking, stream poisoning. | **Source Identity Pinning (Chapter 7):** StreamID is the hash of the publisher's public key ($PK_{\text{Source}}$). Clients permanently pin this key, discarding any non-matching manifest. |

---

## C.2 Threat Landscape Scenarios

### C.2.1 Eclipse Attack Defense
*   **Attack Vector:** An attacker spawns 5,000 virtual nodes (Sybils) close to Target Peer $X$ in the XOR space, attempting to fill $X$'s routing tables and isolate it from the legitimate network.
*   **Protocol Defense:** S/Kademlia's static PoW makes generating 5,000 IDs computationally expensive. S/Kademlia's prefix limits ensure $X$ rejects connections once a subnet prefix ($/24$ IPv4 or $/48$ IPv6) exceeds $5\%$ of active slots.

### C.2.2 Free-Rider Exploitation Defense
*   **Attack Vector:** A modified client requests blocks from high-tier superpeers, immediately closing the connection or returning junk bytes when asked to upload.
*   **Protocol Defense:** The uploader evaluates reciprocity every $500\text{ ms}$ via Tit-for-Tat. Non-reciprocating peers are choked on the very next cycle, limiting exploitation to a single $500\text{ ms}$ interval.

### C.2.3 Collusive Rating Pump Defense
*   **Attack Vector:** Two malicious nodes, $M_1$ and $M_2$, generate thousands of fake PoU receipts for each other to artificially pump their contribution scores.
*   **Protocol Defense:** Graph-theoretic auditing detects highly symmetric bi-directional edges and IP subnet clustering, completely nullifying the reputation scores of the collusive clique.
