# BitStream

**A tokenless, trackerless peer-to-peer protocol for live video at one million concurrent viewers.**

BitStream distributes a live stream from a single broadcaster to up to 1,000,000 viewers with a 3–5 second glass-to-glass delay, using only the viewers' own upload capacity. There is no CDN, no tracker, no coordination server and no token. Every mechanism in the design is derived from two constraints: the swarm must work from two participants (a streamer and one viewer) up to a million, and it must exploit gigabit super nodes sitting next to phones on cellular links without trusting either.

This repository contains the **specification**, the **reference implementation**, and the **audit record** that connects them.

| | |
|---|---|
| Specification | [`protocol/`](protocol/) — canonical; start at [`protocol/INDEX.md`](protocol/INDEX.md) |
| Reference implementation | [`src/`](src/) — Rust, sans-I/O core, deterministic simulator |
| Design record | [`solutions/`](solutions/) — why each decision was taken and what was rejected |
| Open defects | [`issues/`](issues/) |
| License | [Apache License 2.0](LICENSE) |

---

## Contents

1. [Design goals](#1-design-goals)
2. [How it works](#2-how-it-works)
3. [Key mechanisms](#3-key-mechanisms)
4. [Threat model](#4-threat-model)
5. [Repository structure](#5-repository-structure)
6. [How the specification is maintained](#6-how-the-specification-is-maintained)
7. [Reference implementation](#7-reference-implementation)
8. [Status](#8-status)
9. [Reading order](#9-reading-order)
10. [Contributing](#10-contributing)
11. [License](#11-license)

---

## 1. Design goals

| Goal | Target | Where it is derived |
|---|---|---|
| Glass-to-glass latency | 3–5 s; about 4.0 s at the reference settings (250 ms signing chunks, 3.0 s playout buffer, at most 7 hops) | Ch1 §1.1.3 |
| Startup | First frame within 1.5 s of pressing play | Ch8 §8.2 |
| Scale | 1,000,000 concurrent viewers at a mean tree depth of at most 7 | Ch1 §1.1.3 |
| Cold start | Every mechanism defined and working at N = 2 | Ch1 §1.2.1, §1.3 |
| Resilience | Sub-second repair of a lost parent, scoped to one tree so the others keep flowing; resolution loss rather than buffering under capacity shortage | Ch3 §3.3, Ch1 §1.1.5 |
| Fairness | Contribution buys placement and quality; a device that cannot upload still receives the base layer | Ch1 §1.2.2, §1.2.5, Ch5 |
| Trust | No byte is forwarded before it is verified against a broadcaster signature; no peer declares its own address; identities are bound to addresses | Ch4 §4.1, Ch2 §2.2, Ch7 |
| Cost to the broadcaster | Bounded by a small constant, never proportional to the audience | Ch1 §1.1.5 §5.4 |

The specification treats the sustainability condition — that the viewers' aggregate upload can carry the stream — as an assumption that *frequently fails* on real residential connections, and defines the degraded modes as first-class behaviour rather than as an afterthought.

## 2. How it works

A stream is a sequence of 1-second closed-GOP segments, each cut into four 250 ms chunks. The broadcaster encodes the video as a ladder of scalable layers (a 480p base layer and enhancement layers to 720p and 1080p on the reference ladder), signs one manifest per chunk over a Blake3 Merkle tree of 16 KB blocks, and pushes the blocks into a **multi-forest overlay**.

```
                             Broadcaster (source)
                     signs one MANIFEST per 250 ms chunk
                        ┌──────────┼──────────┐
                        │          │          │
                     Tree 1     Tree 2  ...  Tree M        one tree per stripe of one layer
                        │          │          │            (M scales 2 → 6 with the relay count)
                    relays     relays      relays          each relay is interior in ONE tree,
                     /  \       /  \        /  \           a leaf in the others
                   ...  ...   ...  ...    ...  ...
                        │          │          │
                     viewer  ←  viewer  ←  viewer          every viewer subscribes to every tree
                              of the layers it renders
```

**Layer 1 — Discovery (Chapter 2).** Peers find one another through an S/Kademlia distributed hash table. A stream is identified by the hash of its publisher's Ed25519 key, so it is self-authenticating. Node identities are bound to a public key and to the peer's observed IP address by two proof-of-work puzzles; the protocol is explicit that this makes identities *rate-limited*, not expensive, and that its Sybil bound is address diversity, enforced by per-prefix caps wherever identities are counted. Registration is sampled so that the twenty DHT nodes guarding a stream carry bounded load at any audience size.

**Layer 2 — Membership and repair (Chapter 3).** Each peer keeps a small active set of open QUIC connections and a larger passive set of standby records, indexed by which tree each standby relays. When a parent goes silent, the child detects it within an RTT-scaled deadline, promotes a standby for that one tree, and continues forwarding every other tree meanwhile. Siblings orphaned together elect a deputy deterministically from a roster their parent had already sent them, so a lost subtree is re-attached without a coordination round.

**Layer 3 — The forest (Chapter 1).** The stream is split across M edge-disjoint spanning trees, one per stripe of one layer. M scales with the relay population from 2 to 6. Each relay-class peer is an interior node in exactly one tree, chosen by a rendezvous hash of its identity that stays stable when M changes, and a leaf in the others, so no home connection ever uploads the full bitrate. Super nodes earn interior status in more trees by demonstrated throughput. Parents are chosen by a score in milliseconds over free capacity, round-trip time and depth; a joiner's rank, proven from signed receipts, decides who gets a slot when the tree is full.

**Layer 4 — Media (Chapter 4).** Blocks are pushed down the trees at the live edge and repaired from mesh neighbours in the trailing part of a 3.0 s buffer. Each 16 KB block is one RaptorQ source block with 5–30% parity sized per link, so ordinary packet loss is repaired without a round trip. A relay reconstructs each block, verifies it against the signed manifest root, and only then re-encodes it for its children: corrupted data cannot travel more than one hop.

**Layer 5 — Incentives (Chapter 5).** Every child signs one receipt per segment per tree for the blocks it verified. A peer's rank is the throughput those receipts prove over the last minute, presented as a commit-then-sample proof that a verifier can check in a few kilobytes. Rank buys enhancement-layer slots and shallower placement; Tit-for-Tat governs only the reciprocal repair traffic. Base-layer slots are never taken from a sitting child by rank, and a share of them is reserved for devices that cannot upload.

**Layer 6 — Reachability (Chapter 6).** Cone-NAT peers are reached by a hole punch relayed through whoever introduced them. Symmetric-NAT and CGNAT peers are leaf-class and are served by emergent relays recruited from the swarm and paid in reputation. A broadcaster behind NAT binds ingress relays that act as the root.

**Layer 7 — Hardening (Chapter 7).** Manifests are admitted by sequence window, never by wall clock. A kernel-level XDP filter enforces the *result* of identity validation with token buckets and a default-deny budget for unknown sources. Onion routing is an optional leaf-only mode priced in latency.

## 3. Key mechanisms

Each of these is specified with worked numbers across the full range of N; the table gives the shape and the owning section.

| Mechanism | What it does | Section |
|---|---|---|
| Slot count with overhead | A relay's child slots per tree are its upload, less a 10% repair reserve, divided by the tree's declared bitrate times the parity-and-framing overhead. One expression owns every consumer of each budget. | Ch1 §1.2.1 |
| Layer-to-tree allocation | The source builds the mapping from layers to trees by a minimax rule and publishes it signed; peers read it and never derive it. | Ch1 §1.2.4 |
| Capacity adaptation | Peers shed enhancement layers after two failed join rounds; the publisher folds the top layer out of the mapping when a lower layer starves, moving relays to where they are needed. The base layer never fails while any mapping can carry it. | Ch1 §1.1.5 |
| Rendezvous assignment | Each relay ranks trees by a keyed hash of its identity; growing the forest by one tree moves about 1/(M+1) of relays and no others. | Ch1 §1.2.1 |
| Forest resize | A new tree is pre-emitted during a five-segment window so its relays warm up before the switch; a grace period keeps the fill-in from being mistaken for a shortage. | Ch1 §1.2.4 §4.5 |
| Drain path | Every graceful release — preemption, displacement, resize, demotion, capacity fall, depth overflow — sends a notice first and keeps serving until the child has re-attached. | Ch1 §1.2.2 |
| Depth propagation | A node's depth rides on every block its parent sends, so the hop penalty and the depth limit run on current data, not join-time data. | Ch1 §1.2.2 |
| RTT-scaled liveness | Eviction after `max(200 ms, 2·τ_ping + SRTT + 4·RTTVAR)`; re-attachment costs about two round trips; the pull zone widens with the path. | Ch3 §3.3, Ch4 §4.3.1 |
| Rank proof | A Merkle commitment over the prover's receipts, sorted by a canonical key, sampled in adjacent pairs by a nonce the prover could not have known; rank is computed from the sampled receipts, never declared. | Ch5 §5.2.2 |
| Evidence-carrying eviction | An accusation is admissible only with the signed statements that prove it, and it carries the accuser's own rank proof; three ranked accusers from three prefixes evict locally for a bounded time. | Ch5 §5.3.3 |
| Diversity-relative prefix cap | At most `max(1, ⌈K / min(P_obs, 20)⌉)` slots from one /24 or /48, where P_obs is the number of prefixes the node has itself observed — one per prefix at scale, no cap in a single-subnet swarm. | Ch2 §2.2.2 |
| Stream descriptor | A signed record of codec, container, layer-combination mode and initialisation data per layer, so a decoder can consume the first verified block. The forwarding layers never read it. | Ch4 §4.1.1 |

## 4. Threat model

The specification's security posture is set out in [Appendix C](protocol/appendix_c_threat_model.md). In summary:

| Threat | Defence | Stated limit |
|---|---|---|
| Block poisoning | Blake3 Merkle proofs against a source-signed manifest before any forwarding | None; corrupted data travels at most one hop |
| Stream hijack | Stream identity is the hash of the publisher's key; every manifest is pinned to it | None |
| Sybil identities | Proof-of-work rate-limits identity creation; per-prefix caps bound what one address block can hold | Identities cost about 10 ms; the bound is prefixes controlled, not hashes computed |
| Eclipse | One k-bucket entry per prefix, disjoint parallel lookups | Holds with f the *prefix* fraction the attacker controls |
| Free-riding | Receipt deadlines on the tree, Tit-for-Tat on repair traffic, rank-gated enhancement slots | A non-signing child obtains three segments of one stripe before eviction, bounded per prefix |
| Rank forgery | Commit-then-sample receipt proof; per-receipt caps; distinctness check | A colluding uploader with leaf-class Sybil downloaders *can* forge rank; the damage is bounded where rank is spent, and the specification says so plainly |
| Collusion | Per-tree symmetry audit; subnet penalty on resolvable signers; evidence-carrying accusations | The subnet penalty cannot be applied to leaf-class signers |
| Denial of service | XDP token buckets over a handshake-validated allowlist; default-deny budget for unknown sources; sampled registration keeps guardian load bounded | IPv4 only; IPv6 filter is future work |
| Replay | Manifests admitted by sequence window relative to the observed live edge | No wall clock is consulted |

Every limit in the right-hand column is stated in the specification at the point where the defence is defined.

## 5. Repository structure

```
protocol/           The specification. Canonical; wins over everything else.
  INDEX.md            Chapter map.
  chapter1/ … 7/      Foundations, discovery, membership, media, incentives, NAT, hardening.
  chapter8_simulation Validation plan and key performance indicators.
  appendix_b_…        Every tunable constant, with the section that uses it.
  appendix_d_…        Normative frame registry; the byte diagram is the specification.
  schemas/            Non-normative protobuf field reference, regenerated from Appendix D.

src/                The reference implementation (Rust). See src/README.md.

issues/             Open defects, one file each, with worked numbers and a proposed fix.
solutions/          One file per closed issue: the decision, the alternatives rejected,
                    the defects found during verification, the residual risk, and what
                    simulation still owes. README.md carries the accumulated design
                    principles.

AUDITOR-AGENT.md    Operating instructions for finding defects in the specification.
FIXER-AGENT.md      Operating instructions for resolving them.

thoughts/           Early exploratory notes, retained for their reasoning. Superseded.
latex/              Typeset edition and a taxonomy paper.
```

## 6. How the specification is maintained

The specification is developed as an engineering artefact under an explicit audit loop rather than as prose that is edited in place.

1. **Audit.** A full read of `protocol/` against the requirement that every rule hold from N = 2 to N = 10⁶ produces tickets in `issues/`. A ticket names the sections, works the numbers, states the impact, and proposes a fix. Auditors do not fix. [`AUDITOR-AGENT.md`](AUDITOR-AGENT.md) is the procedure.
2. **Cluster.** Before anything is changed, the open tickets are mapped by the sections, formulas and frames their fixes would touch, and overlapping tickets are resolved together as one coherent change. Fixing them one at a time produced partial fixes that did not compose.
3. **Fix and verify.** A fix is not accepted on prose review. The arithmetic is evaluated across the whole input range; byte layouts are summed; every value a rule consumes is traced to the frame that delivers it; every degraded mode is traced through every state machine it touches. Of the first eighteen fixes, fourteen were incomplete and two made the specification worse; the current procedure exists because of that. [`FIXER-AGENT.md`](FIXER-AGENT.md) is the procedure.
4. **Record.** Each closed issue becomes a file in `solutions/` that says what would break if the decision were reverted. Design principles that generalise beyond the issue that produced them are collected in [`solutions/README.md`](solutions/README.md); there are forty-one.
5. **Feed back.** The reference implementation files an issue whenever it must decide something the specification leaves open. Several recent tickets came from the simulator rather than from reading.

Two practical rules follow from this and are worth knowing before reading anything: **where two documents disagree, `protocol/` wins**, and **for a wire format, the byte diagram is the specification and the prose is commentary.**

## 7. Reference implementation

[`src/`](src/) is a Rust implementation of the protocol together with a deterministic simulator, tooling and clients. Its organising principle is that the peer state machine, `bs-core`, is **sans-I/O**: it consumes inputs (frames, session events, timers, local commands) and emits outputs, and touches no socket, clock or thread. One driver runs it against a discrete-event simulator with a hundred nodes on a laptop; another runs it against real UDP and QUIC. Every constant is named as in Appendix B and lives in one module; every frame is encoded exactly as drawn in Appendix D and checked against golden vectors by an independent Python decoder.

| Crate | Role |
|---|---|
| `bs-wire` | Frame header, registry, byte layouts, records. No logic. |
| `bs-crypto` | Identities, proof-of-work, Blake3 Merkle trees, signatures. |
| `bs-media` | Chunking, blocks, manifests, layer ladder, tree mapping, RaptorQ, sources and sinks. |
| `bs-core` | The peer: lifecycle, forest, swarm buffer, repair, admission, drain path. |
| `bs-sim` | Discrete-event simulator, scenarios, KPIs, runtime invariants, `--explain`. |
| `bs-ingest`, `bs-node`, `bs-player-api` | Real media ingest, the `bsnode` runtime over quinn, and the local player interface. |

The test framework is designed so that a failure can be fixed from its report alone: every test, KPI gate and invariant emits the specification section it enforces, the seed, the reproduction command and the node timeline that led to the failure.

```
cd src
scripts/check.sh                                   # fmt, clippy, tests, cross-decoder, simulator gates
cargo run -p bs-sim --profile sim -- run scenarios/baseline_100.toml
cargo run -p bs-sim --profile sim -- explain <report> --node <id>
```

See [`src/ARCHITECTURE.md`](src/ARCHITECTURE.md), [`src/MILESTONES.md`](src/MILESTONES.md) and [`src/docs/TESTING.md`](src/docs/TESTING.md).

## 8. Status

**Specification.** Sixty issues have been filed against the specification across four audit passes and closed with a recorded solution. Two are open at the time of writing, both filed by the simulator: a relay assigned to a tree has no admission priority in it, and per-layer block padding is not counted in the overhead factor. The constants chosen by argument rather than measurement are listed under *Validation debt* in [`solutions/README.md`](solutions/README.md) and are the subject of Chapter 8.

**Implementation.** Milestone 1 — the multi-forest push path, verify-then-forward, parent selection and per-tree repair at 100 simulated nodes with byte-exact data integrity — is complete and gated by `scripts/check.sh`. Milestone 2 — real networking over quinn, a containerised fleet under network emulation, ffmpeg ingest and a browser client — is in progress. The DHT, gossip, pull repair and incentive layers (Milestone 3) and NAT traversal, relays and the desktop client (Milestone 4) follow.

## 9. Reading order

For a first pass through the specification:

1. [`protocol/README.md`](protocol/README.md) — the foreword and table of contents.
2. Chapter 1 §1.1 — the bandwidth arithmetic and what happens when it fails.
3. Chapter 1 §1.2.1 and §1.2.4 — the forest, the slot count, and how layers map to trees.
4. Chapter 1 §1.2.2 — parent selection, rank admission, the drain path.
5. Chapter 4 §4.1 and §4.3 — what a block is, and the push–pull buffer.
6. Chapter 3 §3.3 — repair under churn.
7. Chapter 5 §5.2 — receipts and rank.
8. Appendix B and Appendix D — every constant and every frame.

Then [`solutions/README.md`](solutions/README.md) for the design principles, and the individual solution files for the reasoning behind any decision that seems surprising.

## 10. Contributing

Contributions are welcome in three forms.

- **Defects in the specification.** File an issue in [`issues/`](issues/) following [`AUDITOR-AGENT.md`](AUDITOR-AGENT.md): name the sections, work the numbers across the range of N, state the impact, propose a fix. A finding that survives the fixer's verification is the measure of a good ticket.
- **Fixes.** Follow [`FIXER-AGENT.md`](FIXER-AGENT.md). Verify against the specification, never against a ticket's status header; compute the arithmetic; trace the wire encoding; write the solution file before deleting the issue.
- **Implementation.** Work in [`src/`](src/) against the milestones in [`src/MILESTONES.md`](src/MILESTONES.md). Where the code must decide something the specification leaves open, file an issue rather than deciding silently.

## 11. License

Licensed under the [Apache License, Version 2.0](LICENSE).
