# Protocol Issues

Open design defects in the specification. Each file is self-contained: context, impact analysis, and a proposed fix.

The canonical specification is [`../protocol/`](../protocol/). When an issue is closed, its file is deleted and the reasoning behind the fix — the design decision, the alternatives rejected, and the residual risk — is recorded in [`../solutions/`](../solutions/).

---

## Open Issues

| ID | Title | Priority | Component |
|---|---|---|---|
| — | *No open issues.* | | |

New issues should be filed as `ISSUE-040-<slug>.md` following the existing format: a header (Status, Priority, Component, Affects, File), then Summary, Detailed Description with worked numbers, Impact, Proposed Fix, Effort. Cross-reference related issues and name the cluster if one exists. Read [`../AUDITOR-AGENT.md`](../AUDITOR-AGENT.md) before filing anything, and [`../FIXER-AGENT.md`](../FIXER-AGENT.md) before resolving anything.

---

## Closed

ISSUE-001 through ISSUE-039 have been verified against the specification and closed. Each has a corresponding record in [`../solutions/`](../solutions/) — see [`../solutions/README.md`](../solutions/README.md) for the index and the design principles they converged on.

ISSUE-019 through ISSUE-039 came from a full read of `protocol/` against the requirement that the design work from $N = 2$ through $N = 10^6$, including gigabit-class super nodes. They were resolved in five clusters, each as one coherent design change:

| Cluster | Issues | Shared change |
|---|---|---|
| Forest arithmetic | 019, 025, 027, 038 | Source-built layer→tree mapping with explicit per-tree bitrate; per-tree $K_v$ with FEC and framing overhead; rendezvous assignment; ladder keyed on relay count from $M = 2$ |
| Discovery and records | 020, 026, 028, 032, 037 | One Peer Record (NodeID, class, tree bitmap, reachability); tree-aware `GET_PEERS`; sampled registration; guardian counts on `STORE_RECORD_ACK`; `RELAY` requires reachability |
| Manifest timing | 030, 031, 033 | 250 ms signing chunks; 3.0 s buffer; sequence-windowed manifest acceptance; leaves are push children |
| Liveness and repair | 021, 022, 036 | Depth on every `BLOCK_PROOF`; heartbeats and RTT-scaled eviction; per-tree `CHURN_REPAIR` |
| Incentives | 023, 024, 029, 034, 035, 039 | Per-segment receipts and `RANK_PROOF`; rank-based tree admission; choking scoped to PULL; per-tree symmetry; PoW as rate limit; evidence-carrying eviction |

Verification was not a formality. Of the first eighteen fixes, **fourteen were incomplete and two were regressions**; of the second batch, the defects found while closing are recorded in each solution file. The ones worth knowing about:

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

## Recurring Patterns

Several defects share a shape, and these are the checks most likely to catch the next one:

1. **Constants that must satisfy a joint relation, set independently in different chapters.** Ask: does any single expression own the total?
2. **A fix validated at one scale, composed with a fix validated at another.** The receipt was sized for ten children; the design also allows eight thousand.
3. **Prose amended, byte layout not.** For a wire format the diagram is the specification and the prose is commentary.
4. **A trigger described but never given a wire field.** Eight of them, in ISSUE-029 alone.
5. **A rule derived from small-$N$ intuition, written as unconditional.** Sound at $N = 5$, an attack surface at $N = 10^6$.
6. **Sentinel values and one-bit verdicts conflating transient with permanent causes.** "Seen before" is not "forged".
7. **Fallback chains written as substitutions instead of cost-ordered unions.**
8. **A single-tree state machine wrapped around a multi-tree node.** One node-level repair state turned a $1/M$ loss into a cascade.

## Remaining Future Work

Not defects — design the specification defers:

- **Simulation validation (Chapter 8).** Every solution file ends with a "Validation owed" section. The constants chosen by argument rather than measurement are listed in [`../solutions/README.md`](../solutions/README.md#validation-debt).
- **Regenerate `schemas/p2p_live.proto`** against the current Appendix D; it is non-normative and now lags the registry.
- **Frame conformance vectors.** One canonical encoded example per frame type, so implementations can be checked against bytes rather than prose.
- **Geographic locality.** Vivaldi synthetic coordinates and ASN-aware peer selection.
- **Multi-source ingest.** Active-active encoders with signed broadcaster handover.
- **IPv6 kernel filter.** The XDP shield is IPv4-only; v6 needs a parallel filter keyed on the 128-bit source address.
- **Memory-hard identity puzzle.** Whether an Argon2-class static puzzle is worth its cost on low-end devices (SOLUTION-034).
