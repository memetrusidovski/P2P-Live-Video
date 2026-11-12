# Protocol Issues

Open design defects in the specification. Each file is self-contained: context, impact analysis, and a proposed fix.

The canonical specification is [`../protocol/`](../protocol/). When an issue is closed, its file is deleted and the reasoning behind the fix — the design decision, the alternatives rejected, and the residual risk — is recorded in [`../solutions/`](../solutions/).

---

## Open Issues

| Issue | Priority | Component | Summary |
|---|---|---|---|
| [ISSUE-061](ISSUE-061-relay-of-tree-cannot-displace-pure-subscriber.md) | High | Ch1 §1.2.2, §1.3 | A relay assigned to tree m has no admission priority in m; a saturated parent full of pure subscribers deadlocks the tree's growth. Found by the M1 simulator at cold start; addendum on the sequential-handover deadlock |
| [ISSUE-062](ISSUE-062-per-layer-block-padding-not-in-overhead-factor.md) | Medium | Ch4 §4.1.1, Ch1 §1.3, App B | Per-layer 16 KB block padding costs 10–15 % at 250 ms chunks and is not in Ω, so slot counts are optimistic by that fraction. Found when four framing bytes per layer took the 100-relay baseline from 0.02 % to 31 % starvation |
| [ISSUE-063](ISSUE-063-unbounded-warming-exemption-livelocks-joiners.md) | Medium | Ch1 §1.2.2, Ch1 §1.1.5 §5.3, App D §D.4.7 | The WARMING exemption from the shed rule has no bound; at capacity some candidate is always warming, so joiners probe forever and never shed. Found by the lossy-links simulator gate; implementation bounds it to 4 consecutive rounds |
| [ISSUE-065](ISSUE-065-no-nodeid-to-public-key-resolution-for-receipt-verification.md) | Medium | Ch5 §5.2.1–§5.2.3, Ch2 §2.2.1, §2.3.3 | Receipts name the downloader by NodeID, which is a one-way hash of its key, and no record carries the mapping, so third parties (and RANK_PROOF verifiers) cannot check receipt signatures. Implementation carries signer keys in RANK_PROOF pairs and PINGs new sessions |
| [ISSUE-066](ISSUE-066-tft-cooldown-denies-repair-to-tree-children.md) | Medium | Ch5 §5.1.2–§5.1.3, Ch4 §4.3 | A child that cannot reciprocate is choked, and after a failed optimistic window is in a 30 s cooldown, so it cannot repair blocks its own parent lost on the link. Found by a core test; implementation serves a child's repair of the tree it is pushed |
| [ISSUE-067](ISSUE-067-rank-proof-after-neighbor-defers-admission.md) | Low | App D §D.4.17, Ch1 §1.2.2 | The RANK_PROOF follows the NEIGHBOR on one ordered stream, so the admission decision it should inform is already taken. Implementation sends the proof first |
| [ISSUE-068](ISSUE-068-symmetric-pair-evidence-flags-honest-role-swaps.md) | High | Ch5 §5.3.1, §5.3.3 | SYMMETRIC_TREE_PAIR evidence (same tree, 60 s, bytes within 10 %) is produced by honest parent-child swaps under migration and preemption; one simulated run evicted 104 honest nodes. Implementation requires the same segment and one common block |
| [ISSUE-069](ISSUE-069-rank-proof-window-and-nonce-for-third-party-verifiers.md) | Medium | Ch5 §5.2.2, §5.3.3 | The RANK_PROOF window and nonce are anchored on an unstated live edge, so gossip receivers behind the prover cannot verify the nonce and verifiers ahead of it reject the window. Implementation anchors both on the commitment, two segments behind the prover |
| [ISSUE-070](ISSUE-070-manifest-window-strands-a-starved-peer.md) | High | Ch7 §7.1.2 | A peer starved for a while keeps a stale live edge and then drops every current manifest as out of window; it never recovers. Implementation widens the window by the age of the edge |
| [ISSUE-071](ISSUE-071-fold-and-unfold-timing-under-fill-in-and-count-ramp.md) | Medium | Ch1 §1.1.5 §5.6, §1.2.4 §4.5 | Starvation measured across a switch, an unfold judged against a ramping relay count, and a grace window shorter than the fill-in each produced a wrong matrix change. Implementation measures from the fill-in, dwells from the start, and applies the grace to every tree |
| [ISSUE-072](ISSUE-072-slot-cap-leaves-no-queueing-headroom.md) | Medium | Ch1 §1.2.1 §1.3, App B | The slot count spends the whole tree budget, so a full relay runs its uplink at 100 % and the children queued last starve. Implementation keeps 5 % of the uplink free of slots |
| [ISSUE-073](ISSUE-073-slicing-matrix-entry-cannot-encode-a-layer-bundle.md) | High | App D §D.4.8, Ch2 §2.3.3, Ch1 §1.2.4 §4.2.1 | The 7-byte matrix entry has one Layer field and no bundle encoding, so the M = 2 rung every stream starts on is undefined on the wire |
| [ISSUE-074](ISSUE-074-deputy-timer-expires-before-a-sibling-can-answer.md) | Medium | Ch1 §1.2.3, App B τ_deputy, Ch3 §3.3.1 | The fixed 45 ms Deputy timer is shorter than a sibling's 1 to 2 RTT reply on every path over about 22 ms, so the election never succeeds at the reference RTT |
| [ISSUE-075](ISSUE-075-heartbeating-parent-that-never-delivers-is-never-evicted.md) | High | Ch3 §3.3.1–§3.3.2, Ch1 §1.2.2 Rank Admission | Eviction counts any packet and an unparented relay accepts children, so orphaned relays adopt each other and a subtree starves without bound |
| [ISSUE-076](ISSUE-076-unfold-trigger-re-creates-the-base-layer-freeze.md) | High | Ch1 §1.1.5 §5.6, App B | The growth-gated unfold fires when relays and viewers grow together, raises the base stripe bitrate and drains 16 % of base-layer children for 30 s per cycle |
| [ISSUE-077](ISSUE-077-propagation-term-is-per-link-not-per-child-share.md) | Medium | Ch1 §1.1.3, Ch4 §4.1.2, SOLUTION-013/033 | The 13 ms transmission term is per link; a relay with K_v children spends about 150 ms per hop, so glass-to-glass is 4.7 to 5.0 s, not 4.0 |
| [ISSUE-079](ISSUE-079-rank-admission-reads-a-rank-frozen-at-admission.md) | Medium | Ch1 §1.2.2 case 2, Ch5 §5.2.2, App D §D.4.17 | A child's rank is read from the proof it presented at admission and never refreshed, so a lower contributor preempts a higher one |
| [ISSUE-080](ISSUE-080-registration-sample-exponent-ratchets-without-bound.md) | High | Ch2 §2.3.2, §2.3.3, App B | Registrants never learn that s rose, so ActiveCount never halves, N_est doubles per evaluation and s ratchets to its cap within seconds |
| [ISSUE-081](ISSUE-081-dht-sample-is-the-only-entry-point-and-saturates-above-n0.md) | High | Ch2 §2.3.2, Ch1 §1.2.2 §2.2, Ch1 §1.1.5 §5.6 | The fixed registration sample absorbs every DHT-directed join; above N_0 round 1 yields only saturated candidates and the starved re-query feeds the fold trigger |
| [ISSUE-082](ISSUE-082-join-sequence-cannot-meet-the-startup-target.md) | High | Ch4 §4.3.1, Ch5 §5.1.3, Ch8 §8.2 | The specified join sequence renders its first frame at 3.3 to 4.3 s against the 1.5 s SJL target |
| [ISSUE-083](ISSUE-083-preemption-and-displacement-are-unreachable-from-the-join-algorithm.md) | High | Ch1 §1.2.2 §2.2 step 2, Rank Admission cases 2–3 | The probe filter drops every K_avail = 0 candidate, so no NEIGHBOR ever reaches a full parent and preemption and leaf displacement never fire |
| [ISSUE-084](ISSUE-084-punch-requester-probes-before-the-mapping-opens.md) | Medium | App D §D.4.14, Ch1 §1.2.2 §2.2, Ch6 §6.1.1 | The requester's PROBE leaves before the target's PING opens the mapping, and a SYMMETRIC requester never receives that PING |
| [ISSUE-085](ISSUE-085-source-ingress-proposal-carries-no-publisher-signature.md) | High | Ch6 §6.3.3, App D 0x30, Ch2 §2.3.1 | RELAY_PROPOSAL(SOURCE_INGRESS) has no signature field and no rule binds the session to PK_Publisher, so one identity can capture every ingress relay; the implementation trusts a self-declared flag |
| [ISSUE-087](ISSUE-087-xdp-allowlist-keyed-by-address-shares-one-bucket-across-a-cgnat.md) | High | Ch7 §7.2.1–§7.2.2, App B B.1.4 | The XDP allowlist is keyed by address, so children behind one CGNAT share one 2,000 pps bucket and lose the entry when any of them disconnects |

Both were filed from the reference implementation in [`../src/`](../src/) (2026-09-09/10). ISSUE-001 through ISSUE-060 are closed; the reasoning is in [`../solutions/`](../solutions/).

ISSUE-073 through ISSUE-088 came from the tick-by-tick walk audit of 2026-11-02 (twelve scenarios stepped on a virtual clock; traces in [`../audits/traces/`](../audits/traces/), report in [`../audits/2026-11-02-tick-walkthrough.md`](../audits/2026-11-02-tick-walkthrough.md)). Clusters: 080, 081 and 082 (registration sample, discovery entry point, startup latency); 083 with 061 (admission in a full tree); 077 and 078 (per-hop latency and FEC residue); 074 and 075 (repair timers); 085 carries an implementation divergence beside its spec defect. ISSUE-086 and ISSUE-088 were implementation-only and were closed the same week (SOLUTION-086, SOLUTION-088). ISSUE-078 was closed on 2026-11-11 (SOLUTION-078). ISSUE-089 was filed and closed on 2026-11-12 (SOLUTION-089).

New issues should be filed as `ISSUE-090-<slug>.md` following the existing format: a header (Status, Priority, Component, Affects, File), then Summary, Detailed Description with worked numbers, Impact, Proposed Fix, Effort. Cross-reference related issues and name the cluster if one exists. Read [`../AUDITOR-AGENT.md`](../AUDITOR-AGENT.md) before filing anything, and [`../FIXER-AGENT.md`](../FIXER-AGENT.md) before resolving anything.

---

## Closed

ISSUE-001 through ISSUE-060 have been verified against the specification and closed. Each has a corresponding record in [`../solutions/`](../solutions/) — see [`../solutions/README.md`](../solutions/README.md) for the index and the design principles they converged on.

ISSUE-040 through ISSUE-056 came from the third full audit (2026-09-09) of `protocol/` as rewritten by SOLUTION-019–039; most were interactions *between* fixes from different clusters of the previous round. They were resolved in five clusters plus six standalone tickets:

| Cluster | Issues | Shared change |
|---|---|---|
| Capacity and floor | 040, 048, 049 | A relay always joins its assigned trees; the publisher folds the top layer when a lower layer starves; the §5.2 layer table and the leaf-fraction bound are derived from the per-tree condition; the leaf share is enforced by displacement, never held idle |
| Resize and warm-up | 043, 044 | `PROBE_RESPONSE.TreeState` per tree; the source pre-emits new trees during the window within a stated slot budget; a shed grace period after any matrix change |
| PULL reserve | 045, 050 | One inequality per upload budget; bridges are tree edges charged to the tree budget (reversing part of SOLUTION-032); ingress relays are budgeted roots; sequential handover via `ACCEPTED.PENDING` |
| Child ↔ parent control path | 046, 047 | `DRAIN_NOTICE` (0x1E) for every graceful release; `NEIGHBOR` carries class and bitmap; `PROOF_OF_UPLOAD` carries the child advertisement the roster is built from |
| Rank proof | 041, 055 | Canonical-key sorted receipt list with adjacent-pair samples; rank from sampled capped bytes; accusations carry the accuser's proof |
| Standalone | 042, 051, 052, 053, 054, 056 | Diversity-relative prefix cap; `CONE` guardian keepalive; re-attach as round-trips and an RTT-clamped PULL zone; depth-aware hysteresis; four Ch2 count/record fixes; direct shuffle request |

ISSUE-057 and ISSUE-058 were surfaced while closing 041 and 048 and closed in turn. The first is the most important residual in the incentive layer and is now stated in the specification rather than implied away: a colluding uploader with leaf-class Sybil downloaders can forge rank at the cost of identities, not bandwidth, because no verifier can resolve a leaf-class signer's address; the bounds on what forged rank can buy live where rank is spent (SOLUTION-057). The second keeps minimax allocation and tunes the fold rule for the base layer, removing a timer-driven unfold probe that would have re-frozen a stable swarm's base layer every ten minutes (SOLUTION-058).

ISSUE-059 and ISSUE-060 were surfaced on 2026-09-09 by the wire-encoding trace while preparing the reference implementation in [`../src/`](../src/), and closed the same day. The first gave a refused `NEIGHBOR` a frame — `DISCONNECT` now carries a `TreeID` and three refusal reasons, so a refusal is scoped to a tree and never a silence (SOLUTION-059). The second added the `STREAM_DESCRIPTOR`: the codec, container, layer-combination mode and initialisation data a decoder needs, pointed to by the Stream Record and fetched with the segment-$X$ manifests, with `LayerByteLength` in every manifest so per-layer padding is locatable (SOLUTION-060).

Two of the audit's own proposals were found wrong during closure and are recorded in the solutions: hash-ordering a receipt list does not detect duplication (sorted duplicates preserve every order statistic — SOLUTION-041), and the `RANK_PROOF` nonce is not receiver-relative, which is exactly what lets a proof travel with an accusation (SOLUTION-055). One earlier decision was reversed rather than patched: charging emergent-relay bridges to the PULL reserve (SOLUTION-032) made bridging impossible at $M \le 3$ and is replaced by a capped charge to the tree budget (SOLUTION-050).

ISSUE-019 through ISSUE-039 came from a full read of `protocol/` against the requirement that the design work from $N = 2$ through $N = 10^6$, including gigabit-class super nodes. They were resolved in five clusters:

| Cluster | Issues | Shared change |
|---|---|---|
| Forest arithmetic | 019, 025, 027, 038 | Source-built layer→tree mapping with explicit per-tree bitrate; per-tree $K_v$ with FEC and framing overhead; rendezvous assignment; ladder keyed on relay count from $M = 2$ |
| Discovery and records | 020, 026, 028, 032, 037 | One Peer Record (NodeID, class, tree bitmap, reachability); tree-aware `GET_PEERS`; sampled registration; guardian counts on `STORE_RECORD_ACK`; `RELAY` requires reachability |
| Manifest timing | 030, 031, 033 | 250 ms signing chunks; 3.0 s buffer; sequence-windowed manifest acceptance; leaves are push children |
| Liveness and repair | 021, 022, 036 | Depth on every `BLOCK_PROOF`; heartbeats and RTT-scaled eviction; per-tree `CHURN_REPAIR` |
| Incentives | 023, 024, 029, 034, 035, 039 | Per-segment receipts and `RANK_PROOF`; rank-based tree admission; choking scoped to PULL; per-tree symmetry; PoW as rate limit; evidence-carrying eviction |

Verification was not a formality. Of the first eighteen fixes, **fourteen were incomplete and two were regressions**; of the second and third batches, the defects found while closing are recorded in each solution file. The ones worth knowing about:

| Found in | Defect |
|---|---|
| ISSUE-009 | `c_a_eff` formula dropped a `max()`, scaling relays *down* to as few as 1 gossip peer for $17 \le K_v < 80$ |
| ISSUE-010 | Degree weighting granted permanent immunity at any scale to exactly the collusion topology it hunts |
| ISSUE-015 | Shedding a tree left the peer stuck in `CONNECTING` forever — the ACTIVE transition required all $M$ parents |
| ISSUE-014 | Child roster was $O(k^2)$, requiring 280% of a super node's uplink at $K_v = 10{,}000$ |
| ISSUE-011 | Three independently-reasonable PoW discounts compounded to a 512× reduction |
| ISSUE-017 | Four bugs in the XDP C listing, including a rate limiter that fails **open** under multi-queue load |
| ISSUE-019 | The obvious proportional layer→tree rule made $L_1$ starve before $L_2$, forcing a two-layer shed for a one-layer shortage |
| ISSUE-022 | The 50 ms receipt deadline and the 200 ms eviction timeout were both below the spec's own 80 ms reference RTT |
| ISSUE-030 | A duplicate manifest returned the same verdict as a forgery, under a "permanently banned" rule, with up to 6 parents delivering each manifest |
| ISSUE-037 | Guardians' own XDP shield would have dropped over half of a million-viewer swarm's registrations |
| ISSUE-040 | At 4 Mbps relays and $M = 6$ a third of the swarm had no base layer while the layer table promised 720p; re-mapped to two layers the same relays were exactly sufficient |
| ISSUE-041 | A rank-0 node with one genuine receipt could prove any rank up to $107$ by declaring bytes, or $1{,}280$ by duplicating the receipt, and pass every check |
| ISSUE-042 | Under the unconditional 5% prefix cap a ten-viewer stream from one /24 became a chain that rejected the tenth viewer in every tree forever |
| ISSUE-043 | Every ladder step cost every peer $\approx 12$–$17$ s of the top layer: nothing could warm up in a new tree before the switch, and the shed rule fired on the fill-in |
| ISSUE-050 | Charging bridges to a 10% reserve meant a 20 Mbps relay could bridge no 3.0 Mbps tree — none at $M = 2$ — so a NAT-blocked home broadcaster could bind no ingress relay |
| ISSUE-057 | 100 leaf-class Sybils (one second of hashing) signing one receipt per segment per tree yield rank $\approx 220$ with every `RANK_PROOF` check passing; the subnet penalty meant to catch it has no input |
| ISSUE-058 | SOLUTION-040's own unfold probe, evaluated at the $M = 4$ rung with 5–7 Mbps relays, would have frozen the base layer for part of the swarm for 30 s every 10 minutes |
| ISSUE-059 | `DISCONNECT` had no tree scope, so even a drain's final `DISCONNECT(PREEMPTED)` would, read literally, have severed the peers' gossip relationship along with the tree edge |
| ISSUE-060 | Ch4 §4.2.3 pointed at a `SegmentByteLength` manifest field that did not exist, and the chunk total it did have cannot locate the padding of any layer but the last |

## Recurring Patterns

Several defects share a shape, and these are the checks most likely to catch the next one:

1. **Constants that must satisfy a joint relation, set independently in different chapters.** Ask: does any single expression own the total?
2. **A fix validated at one scale, composed with a fix validated at another.** The receipt was sized for ten children; the design also allows eight thousand.
3. **Prose amended, byte layout not.** For a wire format the diagram is the specification and the prose is commentary.
4. **A trigger described but never given a wire field.** Eight of them in ISSUE-029; three more (drain notice, child advertisement, shuffle reply) in the third audit; the join refusal in the fourth. Run the trace on the *negative* outcome of every handshake, not only the positive one.
5. **A rule derived from small-$N$ intuition, written as unconditional.** Sound at $N = 5$, an attack surface at $N = 10^6$ — and the reverse: a 5% cap sound at $K = 10{,}000$, a chain-builder at $K = 6$.
6. **Sentinel values and one-bit verdicts conflating transient with permanent causes.** "Seen before" is not "forged"; a node-level live edge is not a per-tree one.
7. **Fallback chains written as substitutions instead of cost-ordered unions.**
8. **A single-tree state machine wrapped around a multi-tree node.** One node-level repair state turned a $1/M$ loss into a cascade.
9. **A per-peer adaptation and a global assignment designed in different chapters with nothing connecting them.** Shedding could not move capacity the mapping had pinned.
10. **A proof checked for membership when the decision needed cardinality or a total.**

## Remaining Future Work

Not defects — design the specification defers:

- **Simulation validation (Chapter 8).** Every solution file ends with a "Validation owed" section. The constants chosen by argument rather than measurement are listed in [`../solutions/README.md`](../solutions/README.md#validation-debt).
- **Keep `schemas/p2p_live.proto` in step with Appendix D.** Regenerated 2026-09-09 against the registry as of SOLUTION-060; it is non-normative and must be regenerated after any registry change.
- **Frame conformance vectors.** One canonical encoded example per frame type, so implementations can be checked against bytes rather than prose.
- **Geographic locality.** Vivaldi synthetic coordinates and ASN-aware peer selection.
- **Multi-source ingest.** Active-active encoders with signed broadcaster handover.
- **IPv6 kernel filter.** The XDP shield is IPv4-only; v6 needs a parallel filter keyed on the 128-bit source address.
- **Memory-hard identity puzzle.** Whether an Argon2-class static puzzle is worth its cost on low-end devices (SOLUTION-034).
- **Onion relays (Ch7 §7.3)** have no frames, budget or incentive of their own; the section is an optional feature described at the level of intent.
- **Downloader diversity attestation.** A witnessed statement of a downloader's address that a rank verifier could trust — a guardian-witnessed session, or the uploader's own parent observing the child — would close the Sybil-downloader gap of SOLUTION-057; none of the candidates examined is both cheap and unforgeable.
