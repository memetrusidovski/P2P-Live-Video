# Appendix C: STRIDE Threat Model

This appendix maps the key vulnerabilities and design defenses of the protocol using the industry-standard **STRIDE** methodology (Spoofing, Tampering, Repudiation, Information Disclosure, Denial of Service, Elevation of Privilege).

---

## C.1 The STRIDE Defense Matrix

| Threat Category | Core Vector | Impact | Specific Protocol Mitigations |
| :--- | :--- | :--- | :--- |
| **S**poofing | Attacker claims a false identity or hijacks another peer's NodeID. | Eclipse attacks, routing redirection, reputation spoofing. | **Address-bound identities with per-prefix caps (Chapter 2):** NodeIDs are bound to public keys and to the observed IP via Static ($C_1 = 16$) and Dynamic ($C_2$) Proof-of-Work puzzles — which rate-limit identity creation (~10 ms per identity per CPU core) but do not make it expensive — and every place identities are counted caps them per `/24` or `/48` prefix. Sybil power scales with prefixes controlled, not hash rate. |
| **T**ampering | Malicious node alters packet payloads or injects corrupted video blocks. | Video artifacts, player crashes, downstream pollution. | **Blake3 Merkle progressive validation (Chapter 4):** Video blocks carry sister hashes to verify integrity against signed roots in microseconds, discarding poisoned blocks on the fly. |
| **R**epudiation | Peer downloads chunks but denies receiving them to avoid contributing. | Free riding, unfair slot allocation, swarm collapse. | **Proof-of-Upload (PoU) receipts (Chapter 5):** Receivers must return non-repudiable Ed25519 signatures validating receipt. A missing receipt is choked at the next 500 ms Tit-for-Tat cycle and evicted after three unreceipted segments. |
| **I**nformation Disclosure | Eavesdropper monitors peer connections and harvests IP addresses. | Viewer tracking, localized ISP censorship. | **Onion Privacy Tunnels (Chapter 7):** Clients can optionally wrap payloads in three-layered ChaCha20-Poly1305 envelopes routed through randomized entry/middle/exit relays. |
| **D**enial of Service | Botnets flood superpeers or query DHT nodes with garbage traffic. | Socket saturation, bootstrap failures, network partitioning. | **eBPF XDP NIC Rate Limiting (Chapter 7):** Incoming packets are filtered in kernel space using token-bucket filters over a handshake-validated allowlist, with a default-deny budget for unknown sources — flooding packets are dropped in the NIC driver before any memory buffer is allocated. |
| **E**levation of Privilege | Rogue node attempts to spoof the broadcaster's key and take over root streams. | Swarm hijacking, stream poisoning. | **Source Identity Pinning (Chapter 7):** StreamID is the hash of the publisher's public key ($PK_{\text{Source}}$). Clients permanently pin this key, discarding any non-matching manifest. |

---

## C.2 Threat Landscape Scenarios

### C.2.1 Eclipse Attack Defense
*   **Attack Vector:** An attacker spawns 5,000 virtual nodes (Sybils) close to Target Peer $X$ in the XOR space, attempting to fill $X$'s routing tables and isolate it from the legitimate network.
*   **Protocol Defense:** Minting 5,000 IDs is *cheap* — about 50 s on one CPU core, under a second on a GPU (Ch2 §2.2.2) — so PoW is not the defence. The defence is the **prefix cap** (Ch2 §2.2.2): $X$ admits exactly one k-bucket entry per bucket from one $/24$ (IPv4) or $/48$ (IPv6), unconditionally, so filling even one bucket needs $\ge 20$ distinct prefixes, and the disjoint-path lookup bound $P_{\text{hijack}} \le f^{\alpha}$ (Ch2 §2.1.2) holds with $f$ the *prefix* fraction the attacker controls. Its Active Set, parent set and child slots carry the diversity-relative cap $\max(1, \lceil K / \min(P_{\text{obs}}, 20) \rceil)$, which is one per prefix at every ordinary slot count once twenty prefixes have been observed.

### C.2.2 Free-Rider Exploitation Defense
*   **Attack Vector:** A modified client requests blocks from high-tier superpeers, immediately closing the connection or returning junk bytes when asked to upload.
*   **Protocol Defense:** For PULL service, the uploader evaluates reciprocity every $500\text{ ms}$ via Tit-for-Tat and chokes non-reciprocators on the next cycle. For tree delivery, a child that stops issuing receipts is dropped after three unreceipted segments, and the eviction is recorded against its address prefix as well as its NodeID, so rotating identities from the same block does not reset the clock (Ch5 §5.2.1).

### C.2.3 Collusive Rating Pump Defense
*   **Attack Vector:** Two malicious nodes, $M_1$ and $M_2$, generate thousands of fake PoU receipts for each other to artificially pump their contribution scores.
*   **Protocol Defense:** Per-tree symmetry auditing detects bi-directional flow within one tree (impossible for honest parent→child edges); IP subnet clustering discounts bundles whose *resolvable* signers cluster; verified evidence of a symmetric pair can be gossiped as a signed accusation, and three ranked accusers from three prefixes evict the pair locally for a bounded period (Ch5 §5.3). One-directional pumping through leaf-class Sybil downloaders is **not** detected — their addresses cannot be resolved — and is bounded instead where rank is spent: one child slot per prefix per honest parent per tree, reliability scoring of a forged multi-tree node by its own children, and the evidence requirement on accusations (Ch5 §5.3.2).
