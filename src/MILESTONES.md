# Milestones

Status legend: `[ ]` not started · `[~]` in progress · `[x]` done · `[!]` blocked

Every milestone has an **exit gate**: a command that must be green. Nothing is
"done" by description alone.

## M1 — Forest in simulation (synthetic data, 100 nodes) `[~]`

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

## M2 — Real network and real data `[ ]`

| Item | Crate | Status |
|---|---|---|
| quinn/UDP driver around bs-core; identity-bound self-signed certs | bs-node | `[ ]` |
| `bsnode publish --file x.mp4` / `bsnode watch --out y.mp4`; hash check on exit | bs-node | `[ ]` |
| Localhost two-node integration test in CI | bs-node | `[ ]` |
| `fleet/`: docker image, compose with N viewers, netem profiles, one-command up/down/scale | fleet/ | `[ ]` |
| Fleet harness: collect per-node metrics and output hashes, produce the same JSON report as the simulator | py/ | `[ ]` |
| ffmpeg fMP4 ingest, VP9 temporal-layer profile, simulcast profile | bs-ingest | `[ ]` |
| Stream descriptor per ISSUE-060 (pending spec decision) | bs-wire/bs-core | `[!]` |
| Player API: HTTP + WebSocket fMP4 to MSE | bs-player-api | `[ ]` |
| Web client | clients/web | `[ ]` |

**Exit gate:** 1 publisher + 20 containerised viewers under netem loss play the
same `.mp4`, every viewer's output hash equals the source, and the web client plays
it live.

## M3 — Full protocol `[ ]`

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
