# Protocol Issues

Open design defects in the specification. Each file is self-contained: context, impact analysis, and a proposed fix.

The canonical specification is [`../protocol/`](../protocol/). When an issue is closed, its file is deleted and the reasoning behind the fix — the design decision, the alternatives rejected, and the residual risk — is recorded in [`../solutions/`](../solutions/).

---

## Open Issues

None. Every issue filed to date (ISSUE-001 through ISSUE-060) has been verified against the specification and closed; the reasoning is in [`../solutions/`](../solutions/).

New issues should be filed as `ISSUE-061-<slug>.md` following the existing format: a header (Status, Priority, Component, Affects, File), then Summary, Detailed Description with worked numbers, Impact, Proposed Fix, Effort. Cross-reference related issues and name the cluster if one exists. Read [`../AUDITOR-AGENT.md`](../AUDITOR-AGENT.md) before filing anything, and [`../FIXER-AGENT.md`](../FIXER-AGENT.md) before resolving anything.

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
