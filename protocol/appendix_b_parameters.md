# Appendix B: System Parameters and Glossary

## B.1 Centralized System Parameters

To ensure uniform behavior across different implementations, the protocol defines a set of strict, hardcoded default constants.

### B.1.1 Timeouts & Intervals
| Parameter | Default Value | Unit | Description |
| :--- | :---: | :---: | :--- |
| $\tau_{\text{ping}}$ | $100$ | ms | A parent sends a heartbeat whenever it has sent nothing to a child for this long; a child probes after this much silence (Ch3 §3.3) |
| $\tau_{\text{evict}}$ | $\max(200,\ 2\tau_{\text{ping}} + SRTT + 4\,RTTVAR)$ | ms | Parent declared dead after this much total silence, from the connection's QUIC RTT estimator; $240$ ms at 20 ms RTT, $320$ at 80, $490$ at 250 (Ch3 §3.3) |
| Source pacing | $\le 1/2$ | chunk period | A chunk's symbols leave the source spread over at most half the 250 ms chunk period (Ch3 §3.3.2) |
| Receipt deadline | $1$ | segment | A child's receipt for segment $k$, tree $m$ is due before the parent sends the first block of segment $k+2$; choke at the next TFT cycle, evict after 3 consecutive unreceipted segments (Ch5 §5.2.1) |
| $\tau_{\text{tft}}$ | $500$ | ms | Tit-for-Tat unchoking evaluation cycle |
| $\tau_{\text{gossip}}$ | $1000$ | ms | Buffer state bitfield gossip interval (peer-to-peer exchange) |
| $\tau_{\text{sched}}$ | $100$ | ms | Buffer scheduler / reactive PULL evaluation cycle (Ch4 §4.3.3) — distinct from $\tau_{\text{gossip}}$ |
| $\tau_{\text{roster}}$ | $1000$ | ms | Child-roster distribution interval for sibling election (Ch1 §1.2.3); rosters stale after $5$ s |
| $R_{\text{roster}}$ | $8$ | entries | Child-roster size: top-$R$ children by $K_v$, bounding roster cost to $O(k)$ rather than $O(k^2)$ (Ch1 §1.2.3) |
| $\tau_{\text{deputy}}$ | $45$ | ms | Orphan's Deputy-response timer before independent passive-set fallback (Ch1 §1.2.3) |
| $\tau_{\text{forest}}$ | $30$ | s | Minimum dwell between forest-size ($M$) changes; flash-crowd growth may pre-empt it (Ch1 §1.2.1 §1.4) |
| Migration window | $5$ | segments | `MANIFEST_UPDATE.EffectiveSegmentSeq` $= S_{\text{now}} + 5$: segments between a matrix-change announcement (resize or fold) and the switch; the source pre-emits new trees throughout it (Ch1 §1.2.4 §4.5) |
| $\tau_{\text{grace}}$ | $8$ ($= D_{\max}$) | segments | After `EffectiveSegmentSeq`, failed join rounds against a tree whose matrix entry changed do not count toward the shed threshold (Ch1 §1.1.5 §5.3) |
| $s_{\text{fold}}$ / $\tau_{\text{fold}}$ | $0.05$ / $10$ s ($L_0$), $30$ s (other) | fraction / s | Publisher folds the top layer when the estimated starved fraction $\hat{s}_m$ of any tree of a **lower** layer exceeds $s_{\text{fold}}$ on average over $\tau_{\text{fold}}$; shorter for the base layer, whose starvation is a freeze (Ch1 §1.1.5 §5.6) |
| Unfold trigger | $N_{\text{relay}} \ge 1.5 \times$ at fold | — | A folded layer is restored only on relay-population growth, never on a timer — a wrong unfold re-creates the lower-layer starvation that caused the fold (Ch1 §1.1.5 §5.6) |
| $\tau_{\text{ttl}}$ | $180$ | s | Time-to-Live for dynamic S/Kademlia peer registrations; refreshed every $\tau_{\text{ttl}}/2$ |
| $\tau_{\text{nat}}$ | $25$ | s | A `CONE` registrant `PING`s each guardian it registered with at this interval so the guardian can forward `PUNCH_REQUEST`s through a live NAT mapping (Ch2 §2.3.2) |
| Starved-count window | $10$ | s | Guardian window for counting distinct `GET_PEERS` senders and, per tree, those with a starved bit (Ch2 §2.3.3); `StarvedCount` and `QueryCount` are never scaled by $2^{s}$ |
| Pool re-query interval | $30$ | s | Minimum interval between `GET_PEERS` re-queries for one thin per-tree pool (Ch2 §2.3.2) |
| Punch validity | $1$ | s | Window after a `PUNCH_REQUEST` in which the requester's `PROBE` is expected (App D §D.4.14) |
| Re-attach | $\approx 1$–$2$ | RTT | QUIC handshake plus `NEIGHBOR`/`ACCEPTED` to a passive-set standby; $1$ RTT with a 0-RTT resumption ticket (Ch3 §3.3.1) |
| Direct shuffle reply | $\tau_{\text{sched}}$ | ms | A `SHUFFLE` with `TTL = 0` over an open session is answered with a `GOSSIP_EXCHANGE` within this time (Ch3 §3.2.1) |
| `NEIGHBOR` reply | $\tau_{\text{sched}}$ | ms | Every `NEIGHBOR` is answered within this time with `ACCEPTED` or `DISCONNECT(TreeID, REJECTED_*)`; silence past $\tau_{\text{sched}} + RTT$ means unreachable (App D §D.4.3b) |
| Descriptor lead | $5$ | segments | A new `STREAM_DESCRIPTOR` is announced this far before its `EffectiveSegmentSeq`, like a matrix change (App D §D.4.20) |
| `InitData` | $\le 4096$ | bytes per layer | Container / codec initialisation data carried in the `STREAM_DESCRIPTOR` (App D §D.4.20) |
| Child advertisement age | $2$ | segments | A child whose last `PROOF_OF_UPLOAD` advertisement is older than this leaves the parent's `ROSTER` (App D §D.4.15) |

### B.1.2 Overlay Size Constraints
| Parameter | Default Value | Unit | Description |
| :--- | :---: | :---: | :--- |
| $c_a$ | $8$ | nodes | Target size of HyParView Active Set $\mathcal{A}$; relays scale to $c_a^{\text{eff}} = \min(64, \max(c_a, \lfloor K_v/10 \rfloor))$ — flat at 8 until $K_v = 80$, capped at 64 from $K_v = 640$ (Ch3 §3.1) |
| $c_p$ | $32$ | nodes | Target size of HyParView Passive Set $\mathcal{P}$ |
| Per-tree pool floor | $3$ | relays | Minimum known relays with $K_{\text{avail}} > 0$ per subscribed tree in the Passive Set before the peer re-queries (Ch3 §3.1.1) |
| $N_0$ | $10^4$ | peers | Registration sample target: `RegisterSampleLog2` $= \max(0, \lceil \log_2(N/N_0) \rceil)$ (Ch2 §2.3.2) |
| $k$ | $20$ | nodes | S/Kademlia k-bucket capacity |
| $M_{\text{max}}$ | $6$ | trees | Maximum number of trees; the active forest size $M$ is dynamic, scaling with the **relay** count $N_{\text{relay}}$ on the ladder of Ch1 §1.2.1 §1.4 ($M{=}2$ at $N_{\text{relay}}{<}12$ up to $M{=}6$ at $N_{\text{relay}}{\ge}30$); there is no $M{=}1$ rung |
| $M_{\text{min}}$ | $2$ | trees | Smallest forest; $M{=}1$ excludes sub-$7$ Mbps uploaders and gives every peer a single point of failure (Ch1 §1.2.1 §1.4) |
| $\Omega$ | $(1 + \bar{E}/K)(1 + f_{\text{frame}})$ | factor | Per-slice overhead factor entering every slot and sustainability computation: $1.15$ at the parity floor, $1.42$ at the ceiling (Ch1 §1.2.1) |
| $f_{\text{frame}}$ | $0.085$ | factor | Fixed framing overhead per pushed byte: ~70 B of headers per 1024 B symbol plus one `BLOCK_PROOF` per 16 KB block (Ch1 §1.2.1) |
| $r_{\text{pull}}$ | $0.10$ | share | Fraction of a node's upload reserved for PULL service and handover overlaps; owned by $\beta_{\text{pull}} \cdot \text{PullSlots} + \sum_{\text{handovers}} B_m\Omega \le r_{\text{pull}} u_v$ (Ch1 §1.2.1 *The Two Budgets*, Ch5 §5.1.2) |
| Bridge share | $1/2$ | of tree budget | Maximum $U_{\text{bridge}} = \sum_{\text{bridged}} B_m\Omega$ an emergent relay may carry from $(1 - r_{\text{pull}})u_v$; its own $K_v(m)$ is computed with $U_{\text{bridge}}$ removed (Ch1 §1.2.1, Ch6 §6.3.1) |
| $K_S(m)$ | $\lfloor (u_S - R_{\text{src}}) / (M B_m \Omega) \rfloor$ | slots | The source's ordinary slots per tree; pre-emit slots in a new tree during a migration window come from its remaining headroom (Ch1 §1.1.5 §5.4, §1.2.4 §4.5) |
| Ingress relay | $K_R(m) \ge 2$ every tree | slots | $K_R(m) = \lfloor ((1-r_{\text{pull}})u_R - R_{\text{src}}/n_{\text{ingress}}) / (M B_m \Omega) \rfloor$; with two ingress relays $u_R \ge 18$ Mbps at $M = 2$, $\ge 26$ Mbps at $M = 6$; $40$ Mbps recommended (Ch6 §6.3.3) |
| Coverage grant | $1$ | tree | Maximum trees a relay may hold beyond its ranked assignment to cover an otherwise empty tree; accepted only if $(1-r_{\text{pull}})u_v \ge 2\,\Omega B_m$ (Ch1 §1.2.1 §1.4) |
| Active registration | $135$ | s | A registration counts toward $N$, $N_{\text{relay}}$ and per-tree relay counts, and is returned by `GET_PEERS`, only if refreshed within $1.5 \cdot \tau_{\text{ttl}}/2$; it is retained but neither counted nor served until $\tau_{\text{ttl}}$ (Ch2 §2.3.2) |
| $D_{\text{max}}$ | $8$ | hops | Maximum permitted routing depth from source |
| $N_{\text{collusion}}$ | $20$ | nodes | Minimum swarm size before collusion/subnet reputation heuristics activate (Ch5 §5.3) |
| $\sigma_{\text{target}}$ | $1.70$ | ratio | Capacity ratio at which the $D \le 7$ depth proof holds: fan-out 8 at the nominal 1 Mbps slice including overhead, mean upload $\ge 10.2$ Mbps (Ch1 §1.1.5) |
| $\sigma_{\text{full}}$ | $1.28$ | ratio | $\Omega/(1-r_{\text{pull}})$ — capacity ratio below which full bitrate is not deliverable and layer shedding applies, mean upload $< 7.7$ Mbps (Ch1 §1.1.5) |
| Shed rounds | $2$ | rounds | Consecutive failed join rounds in any tree of a layer before shedding that layer and all above it (Ch1 §1.1.5) |
| Shed hysteresis | $10$ | s | Cool-down before a shed layer's trees are retried; never applies to a relay's own assigned trees, which stay in its join set (Ch1 §1.1.5 §5.3) |
| Source reserve $R_{\text{src}}$ | $3 \cdot b_0 \cdot \Omega$ | Mbps | Broadcaster upload held back as the base-layer emergency pool ($\approx 5.2$ Mbps at the reference ladder); delegated to ingress relays when the source is behind NAT (Ch1 §1.1.5 §5.4, Ch6 §6.3.3) |
| NodeClass | `0x00`/`0x01`/`0x02` | code | `RELAY` / `LEAF` / `LEAF_PRIVATE` (Ch1 §1.2.5) |
| TreeState | `SERVING`/`WARMING`/`UNPARENTED` | 2 bits | Per-tree state in `PROBE_RESPONSE.Flags` bits 5–6; only `WARMING` exempts a join round from the shed rule (Ch1 §1.2.2, App D §D.4.7) |
| First segment | $1$ | seq | `SegmentSeq` $0$ is never emitted, so a zero live edge or effective segment means *none* (App D §D.4.8) |
| $\Delta_{\text{buffer}}$ | $3.0$ | s | Playout deadline behind the received live edge (Ch4 §4.3.1) |
| $W_{\text{pull}}$ | $\text{clamp}(\tau_{\text{evict}}^{\max} + 6\,SRTT^{\max},\ 1.5,\ 2.0)$ | s | Width of the PULL zone at the old end of the buffer, over the peer's current parent paths; the PUSH zone is the remainder (Ch4 §4.3.1) |
| $\tau_{\text{retain}}$ | $8$ | s | Verified segments and manifests a peer retains to bootstrap late joiners (Ch4 §4.3.1); also the manifest acceptance window below the live edge (Ch7 §7.1.2) |
| Manifest lookahead | $2$ | segments | Manifests claiming a segment further ahead of the observed live edge are dropped (Ch7 §7.1.2) |
| $\tau_{\text{ban}}$ | $10$ | min | Local ban of a peer that delivered a frame with a forged source signature; doubles on repeat, capped at 24 h; never gossiped (Ch7 §7.1.1) |
| $w_c$ | $12$ | ms/unit | Parent-score capacity credit per unit of $\sqrt{K_{\text{avail}}}$ (Ch1 §1.2.2.1) |
| $w_h$ | $20$ | ms/unit | Parent-score hop penalty per unit of $e^{\lambda h}-1$, $\lambda=0.5$ (Ch1 §1.2.2.1) |
| $K_{\text{ref}}$ | $256$ | slots | Capacity credit saturates here; caps the term at $192$ ms (Ch1 §1.2.2.1) |
| HysteresisMargin | $30$ | ms | Score improvement a new parent must show before a peer migrates; reduced to half the one-hop depth gain $\Delta_h(h)/2$ when the candidate is strictly shallower, so one hop always wins in the shallow direction (Ch1 §1.2.2 §2.3) |
| Leaf share $R_{\text{leaf}}(m)$ | $\lceil 0.2 \cdot K_v(m) \rceil$ | slots | Share of a relay's $L_0$-tree slots leaf-class children may claim by **displacing** a pure-subscriber relay child; never held idle; $L_0$ slots are otherwise never taken from a sitting child (Ch1 §1.2.2 item 3, §1.2.5 §5.5) |
| $\beta_{\text{pull}}$ | $0.5$ | Mbps | Sustained draw permitted per unchoked PULL slot; $\text{PullSlots} = \lfloor (r_{\text{pull}} u_v - \sum_{\text{handovers}} B_m\Omega) / \beta_{\text{pull}} \rfloor$, recomputed every $\tau_{\text{tft}}$ (Ch5 §5.1.2) |
| Preemption margin | $1.25$ | ratio | A joiner's rank must exceed $1.25\times$ the lowest child's to preempt it in an enhancement tree; one handover in progress per tree; sequential (`ACCEPTED.PENDING`) when the reserve cannot hold $B_m\Omega$ (Ch1 §1.2.2) |
| PULL credit cap | $\beta_{\text{pull}} \times 1$ s | per counterparty per segment | Maximum `PULL`-receipt bytes credited to $\Theta$ from one peer per segment (Ch5 §5.2.2) |
| Receipt cap | $1.5 \times \text{BitrateKbps}_m \times 1$ s | per tree receipt | Maximum bytes one tree receipt may contribute to $\Theta^{\text{rate}}$ — what one segment of the named tree can honestly contain (Ch5 §5.2.2) |
| $\lambda$ | $0.005$ | s$^{-1}$ | Receipt age decay in $\Theta$ (Ch5 §5.2.2) |
| Rank sample | $8$ pairs ($16$ receipts) | — | Adjacent pairs sampled in a `RANK_PROOF` from a list sorted by canonical key; inflation by a fraction $f$ of fake or duplicated receipts survives with probability $(1-f)^8$ (Ch5 §5.2.2) |
| Rank window | $60$ | segments | Receipts count toward $\Theta^{\text{rate}}$ only if their `SegmentSeq` is within 60 of the verifier's live edge (Ch5 §5.2.2) |
| $r_{\text{accuser}}$ | $0.25$ | rank | Minimum $\Theta^{\text{rate}}/B$ for an accuser to be counted; established by the `RANK_PROOF` attached to the accusation (Ch5 §5.3.3) |
| Eviction threshold | $3$ accusers, $3$ prefixes | — | Distinct verified accusers from distinct `/24`/`/48` prefixes within $\tau_{\text{accuse}}$ (Ch5 §5.3.3) |
| $\tau_{\text{accuse}}$ | $10$ | min | Window over which accusations against one suspect are counted; also the first eviction duration (doubling, capped 24 h) (Ch5 §5.3.3) |
| Accusation rate | $4$ | per accuser per minute | Accusations accepted from one accuser; the rest are discarded unread (Ch5 §5.3.3) |
| Prefix cap | $\max(1, \lceil K / \min(P_{\text{obs}}, 20) \rceil)$ | of a slot set of size $K$ | Maximum Active Set, parent-set and per-tree child slots from one `/24` or `/48`, where $P_{\text{obs}}$ is the number of distinct prefixes the node has observed; k-buckets are capped at exactly $1$ of $20$ per prefix unconditionally (Ch2 §2.2.2, App C) |
| $\tau_{\text{drain}}$ | $5$ | segments | Window a parent keeps serving a child it has released with `DRAIN_NOTICE` — preemption, displacement, resize move, demotion, capacity fall, depth overflow (Ch1 §1.2.2 *The Drain Path*, App D §D.4.19) |

### B.1.3 Cryptographic & Encoding Puzzles
| Parameter | Default Value | Unit | Description |
| :--- | :---: | :---: | :--- |
| $C_1$ | $16$ | bits | S/Kademlia static Proof-of-Work prefix requirement |
| $C_2$ | $12$ | bits | S/Kademlia dynamic Proof-of-Work (IP-bound) requirement — adaptive by swarm size: $8/10/12/14$ at $N < 50 / 10^3 / 10^5 / \ge 10^5$ (Ch2 §2.2); halved on same-subnet reconnect. Discounts are alternatives, never cumulative, and are floored at $C_2^{\min} = 8$ |
| Chunk | $250$ | ms | Signing unit: 4 chunks per 1 s segment, one `MANIFEST` and one Merkle tree each; `BlockIndex = ChunkIndex << 12 \| j` (Ch4 §4.1.1) |
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
| `RATE_PPS_UNKNOWN` | $50$ | pps | Per-source budget for un-allowlisted first contact; sized for a CGNAT address shared by many viewers (Ch7 §7.2.1) |
| `BURST_UNKNOWN` | $100$ | packets | Bucket capacity for un-allowlisted sources |
| `GLOBAL_NEW_PPS` | $5000$ | pps | Aggregate ceiling on new-connection work from all unknown sources. Held in a percpu map: user space loads $\text{GLOBAL\_NEW\_PPS} / \text{num\_online\_cpus}$ per CPU, **not** the whole value (Ch7 §7.2.2) |
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
