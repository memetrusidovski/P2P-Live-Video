# Milestones

Status legend: `[ ]` not started · `[~]` in progress · `[x]` done · `[!]` blocked

Every milestone has an **exit gate**: a command that must be green. Nothing is
"done" by description alone.

## M1 — Forest in simulation (synthetic data, 100 nodes) `[x]`

Goal: prove the multi-forest push path, verify-then-forward, parent selection and
per-tree repair work at 100 nodes in a deterministic simulator, with data
integrity confirmed end to end.

| Item | Crate | Status |
|---|---|---|
| Workspace, toolchain, CI skeleton, `scripts/check.sh` | — | `[x]` |
| Frame header, registry, all Appendix D layouts needed for M1, golden vectors, proptests | bs-wire | `[x]` |
| Identities, static/dynamic PoW with tiering, Blake3 Merkle, rendezvous assignment | bs-crypto | `[x]` |
| Chunking (250 ms), 16 KB blocks, manifests, layer ladder, tree-mapping allocation rule, RaptorQ with fast path | bs-media | `[x]` |
| Sources: synthetic layered bytes; file-backed (`.mp4` as opaque bytes at a target bitrate) | bs-media | `[x]` |
| Sinks: reassemble per layer, hash and compare against the source (data-correctness check) | bs-media | `[x]` |
| `params` module mirroring Appendix B | bs-core | `[x]` |
| Sans-I/O node: Input/Output model, timers, lifecycle states | bs-core | `[x]` |
| Forest: per-tree parent/children, depth propagation, `K_avail`, warm-up gating | bs-core | `[x]` |
| Join: probe, score (§1.2.2), NEIGHBOR/ACCEPTED, admission (depth, free slot, ISSUE-059 rejections) | bs-core | `[x]` |
| Swarm buffer: manifests by sequence window, symbol accumulation, Merkle verify, forward, playout deadline | bs-core | `[x]` |
| Liveness: heartbeat on idle, RTT-scaled eviction, per-tree CHURN_REPAIR, DRAIN_NOTICE handling | bs-core | `[x]` |
| Simulator: virtual clock, event queue, latency/loss model, discovery oracle, churn scripting | bs-sim | `[x]` |
| KPIs (PSR, SJL, CDO, D_avg), invariant checker, `--explain`, JSON reports, event log | bs-sim | `[x]` |
| Scenarios: `small_swarm`, `baseline_100`, `flash_crowd`, `churn_storm`, `lossy_links`, `poisoned_blocks`, `file_source` | scenarios/ | `[x]` |
| Core state-machine tests (`crates/bs-core/tests/node.rs`, 12 tests) | bs-core | `[x]` |
| ISSUE-061 displacement, sequential handover (PENDING), poisoned-parent eviction | bs-core | `[x]` |
| Python: report summariser, plots, independent wire decoder against golden vectors | py/ | `[x]` |
| Docs: ARCHITECTURE.md, TESTING.md, wire coverage table | docs/ | `[x]` |

**Exit gate:** `scripts/check.sh` green, which includes `baseline_100` at PSR ≤ 0.1 %
and mean depth ≤ 7, `churn_storm` recovering every tree within the Ch3 §3.3 budget,
zero invariant violations, and every viewer's reassembled output hashing equal to
the source file.

## M2 — Real network and real data `[~]`

| Item | Crate | Status |
|---|---|---|
| quinn/UDP driver around bs-core; identity-bound self-signed certs | bs-node | `[x]` |
| `bsnode publish --file x.mp4` / `bsnode watch --out y.mp4`; hash check on exit | bs-node | `[x]` |
| Localhost two-node integration test in CI | bs-node | `[x]` |
| `fleet/`: docker image, compose with N viewers, netem profiles, one-command up/down/scale | fleet/ | `[x]` |
| Fleet harness: collect per-node metrics and output hashes, produce the same JSON report as the simulator | py/ | `[x]` |
| ffmpeg fMP4 ingest, simulcast renditions as layers (temporal-SVC profile deferred) | bs-ingest | `[x]` |
| REGISTER_PEER / GET_PEERS frames + bootstrap guardian | bs-wire, bs-node | `[x]` |
| Stream descriptor (SOLUTION-060): wire frame, core plumbing, Stream Record fields | bs-wire/bs-core | `[x]` |
| Player reads INIT from the descriptor instead of the in-band TLV; publisher builds the descriptor from ffmpeg init segments (fps, dimensions via META) | bs-node/bs-player-api | `[x]` |
| Audio: AAC track in every rendition, MSE type includes `mp4a.40.2` | bs-ingest, clients/web | `[x]` |
| Fleet runs isolated by run id; parity from the netem profile; `--parity` CLI | fleet/, bs-node | `[x]` |
| Fleet run under `lossy` / `cellular` netem profiles with a KPI gate in `check.sh` (opt-in, needs docker) | fleet/ | `[ ]` |
| Browser check of the web player (manual: open http://host:8080 while watching) | clients/web | `[ ]` |
| Fleet observability: per-viewer player pages on mapped host ports, `fleet/dashboard.py` with per-chunk integrity check against the publisher, ffmpeg-mode fleet | fleet/, bs-player-api | `[x]` |
| Player API: HTTP + WebSocket fMP4 to MSE | bs-player-api | `[x]` |
| Web client | clients/web | `[x]` |

**Exit gate:** 1 publisher + 20 containerised viewers under netem loss play the
same `.mp4`, every viewer's output hash equals the source, and the web client plays
it live. Status: 6 clean-profile containers byte-exact (2026-09-10); lossy profile and
the browser check still to run.

## M3 — Full protocol `[~]`

| Item | Crate | Status |
|---|---|---|
| PULL zone: BUFFER_STATE_BITFIELD, deadline-aware rarest-first scheduler, PULL_REQUEST / BLOCK_TRANSMISSION / repair symbols, paced PULL budget served in urgency order, late-join backfill | bs-core | `[x]` |
| HyParView membership (Active/Passive sets, JOIN/FORWARD_JOIN/SHUFFLE/GOSSIP_EXCHANGE, session HMAC, JOINING state) | bs-core | `[x]` |
| S/Kademlia DHT: routing table, disjoint lookups, guardians, STORE_RECORD, sampled registration | bs-core | `[x]` |
| Incentives: PROOF_OF_UPLOAD receipts, Tit-for-Tat unchoker over PULL, RANK_PROOF, rank admission | bs-core | `[ ]` |
| Reputation auditing, evidence-carrying eviction | bs-core | `[ ]` |
| Forest resize and fold (MANIFEST_UPDATE), leaf share, adaptive parity from receipts | bs-core | `[ ]` |
| Ch8 adversarial scenarios as gates; fleet churn run matches the simulator | scenarios/, fleet/ | `[ ]` |


S/Kademlia DHT and registration · HyParView membership and gossip · PULL zone,
bitfields, rarest-first · Tit-for-Tat, Proof-of-Upload, RANK_PROOF, admission by
rank · reputation auditing · forest resize and fold · leaf share and drain path in
full. Exit gate: adversarial scenarios from Ch8 §8.3 pass in simulation; 100-node
docker fleet with churn matches simulator KPIs within tolerance.

## M4 — Edge cases and clients `[ ]`

NAT traversal (PONG reflection, PUNCH_REQUEST, ICE-lite) · emergent relays ·
connection migration · Tauri desktop · browser peer over WebTransport (LEAF_PRIVATE)
· XDP shield (Linux only). Exit gate: NAT matrix scenario from Ch8 §8.3 C passes on
the fleet with simulated NAT types.

## Log

- 2026-09-16 — S/Kademlia DHT replaces the oracle (simulator) and the bootstrap guardian
  (`bsnode`): FIND_NODE/FIND_VALUE/STORE_RECORD/STORE_RECORD_ACK typed frames (32 golden
  vectors), `bs-core::dht` (k-buckets, α-disjoint-path lookups, unit-tested) and
  `node/dht.rs` (RPC servers, guardian storage, registration, publisher store loop). All
  seven gates pass on the real DHT; discovery adds ~350 ms p50 to join latency (60 ms with
  the oracle) and the small swarm's SJL improved (2.24 s vs 2.5 s). Found on the way: an
  Active Set eviction racing an in-flight NEIGHBOR admitted children into dead connections
  (admission now needs an open session; the simulator drops in-flight frames on close), the
  propagated-depth check is now soft (admission depth stays hard), and a lost GET_PEERS
  answer stranded viewers in DISCOVERY (1 s re-query).

- 2026-09-15 (later) — HyParView membership landed and the seven simulator gates are green
  again. Regressions found on the way: depth propagation had no reaction at `h ≥ D_max`
  (poisoned_blocks reached depth 11 and halted on the invariant; relays now drain their
  children with reason DEPTH and re-select); nodes that could not join a tree at capacity
  pulled the whole stripe from the publisher (19k egress drops in lossy_links; pulls are now
  only for attached, repairing or serving trees); and the spec's unbounded WARMING exemption
  kept those nodes probing for 20+ s without ever shedding (ISSUE-063; bounded to 4 rounds).
  `f_frame` follows Ch1 (0.085), the PULL requester retries with backoff and provider
  rotation (requests per pulled block 10 → 5), and the verdict now reports `pulls_refused`,
  `pulls_unheld` and `top_queue_drops` for overload attribution.

- 2026-09-15 — M3 started with the PULL zone. First full run cut startup join latency from
  3.4 s to about 2.0 s (backfill) but exposed three defects in turn: unsubscribed layers were
  pulled forever (17k–230k pulls per run); admission-time manifests older than the first pushed
  chunk were counted as steady-state starvation; and repair proofs on the control stream were
  dropped. Fixed; backfill is now reported separately (`backfill_played/ok`).

- 2026-09-15 — Descriptor-driven initialisation in the player (init segments ride the
  STREAM_DESCRIPTOR; INIT+META only in a rendition's first chunk), AAC audio in every
  rendition, `--parity` with per-profile defaults, fleet runs isolated by run id. A lossy
  fleet run at 2 % loss with parity 1 starved 21 % of chunks, as the sim predicted: adaptive
  parity (M3) is the real fix; profiles carry parity 4/5 meanwhile.

- 2026-09-10 (evening) — Fleet observability: every container serves its player page on a
  mapped host port, `fleet/dashboard.py` compares per-chunk per-layer Blake3 checkpoints of
  each viewer with the publisher's; publisher builds the STREAM_DESCRIPTOR from ffmpeg init
  segments and viewers receive it. Found and fixed simulator nondeterminism (HashMap order
  leaking into forwarding/victim choice); identical seeds now give identical verdicts.

- 2026-09-10 (later) — Spec caught up (SOLUTION-059/060 in the user's commit): wire, media and
  core moved to the TreeID-scoped DISCONNECT, per-layer byte lengths, Stream Record descriptor
  fields and the STREAM_DESCRIPTOR frame (36 codes, 22 typed; Python decoder in step).
  Docker fleet: 6 viewers byte-exact. ffmpeg simulcast ingest → swarm → WebSocket → MSE player
  working (two renditions, 0 starvation over 60 s). ISSUE-062 filed (block padding overhead).

- 2026-09-10 — M2: quinn runtime with identity-bound mutual TLS and a shared UDP socket;
  `bsnode publish/watch`; one publisher and three viewers on localhost streamed the first
  3 MB of the user's example.mp4 byte-exact over QUIC; integration test in CI. Found and
  fixed: RFC 9287 QUIC-bit greasing broke the plain-frame demux; STREAM_END discarded the
  playout buffer. Filed ISSUE-062 (block padding overhead). Docker fleet and ffmpeg/player
  path in progress.

- 2026-09-09 (evening) — `scripts/check.sh` green end to end: fmt, clippy, 41 Rust tests,
  20 Python cross-decoder tests, seven simulator gates. Drain path reworked twice from
  simulator evidence (release on the new parent's first BLOCK_PROOF; one-segment drain
  window when the handover is sequential; ISSUE-061 addendum).
- 2026-09-09 (later) — All seven simulator scenarios pass: 100 relays stream three
  layers with PSR 0.02 %, mean depth 2.6, CDO 1.3 %; churn storm (30 % of relays
  killed) repairs within budget; 3 % loss absorbed by parity; poisoning relays are
  evicted by their children; a real `.mp4` streams byte-exact (every delivered
  layer-chunk hash-checked). First runs surfaced ISSUE-061 (relay of tree m locked
  out by pure subscribers) and six implementation defects, all fixed. Startup join
  latency is Δ_buffer-bound (≈3.3 s) until M3 adds backfill; thresholds say so.
- 2026-09-09 — M1 started. Workspace created; bs-wire complete with 19 typed
  frames, golden vectors and proptests. ISSUE-059 and ISSUE-060 filed from the
  wire-encoding trace.
