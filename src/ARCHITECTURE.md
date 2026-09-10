# BitStream Reference Implementation — Architecture

This document explains how `src/` is organised, why it is organised that way,
and the rules that keep it that way. The protocol itself is specified in
[`../protocol/`](../protocol/); this code implements it. Where the code has to
decide something the specification leaves open, the decision is recorded as an
issue in [`../issues/`](../issues/) and referenced from a doc comment at the
point of implementation.

Milestone tracking lives in [`MILESTONES.md`](MILESTONES.md). The test strategy
lives in [`docs/TESTING.md`](docs/TESTING.md).

---

## 1. Principles

1. **The spec is the source of truth.** Every constant is named as in Appendix B
   and lives in one place (`bs_core::params`). Every frame is encoded exactly as
   drawn in Appendix D. Tests cite the section they enforce.
2. **The core is sans-I/O.** `bs-core` contains the whole peer state machine and
   touches no socket, clock or thread. It consumes *inputs* and emits *outputs*.
   One driver runs it against a discrete-event simulator; another runs it against
   real UDP and QUIC. This is what makes 100 nodes on a laptop, deterministic
   replays and, later, a wasm build possible from a single implementation.
3. **Payload-agnostic.** From ingest to player, a chunk is an ordered list of
   layer byte-strings. Nothing between the source and the sink knows what a codec
   is. Video is one source/sink pair; audio, files, game state or sensor data are
   others.
4. **Verify before forward.** No byte leaves a node unless it has been Merkle
   verified against a source-signed manifest. The simulator asserts this as an
   invariant at every step.
5. **Deterministic by construction.** All time and randomness are injected. Any
   failure reproduces from a seed.
6. **Failures explain themselves.** Every test, KPI gate and invariant emits the
   spec section, the seed, the reproduction command and the node timeline that
   led to the failure. The test framework exists so that an agent can fix a
   defect from the report alone.

---

## 2. Crate map

```
crates/
  bs-wire        Frame header, registry, byte layouts, records.        no logic
  bs-crypto      Identities, PoW, Blake3 Merkle, signatures.           no I/O
  bs-media       Chunk → blocks → manifest; ladder; tree mapping;      no I/O
                 RaptorQ; sources and sinks (synthetic, file).
  bs-core        The peer: lifecycle, forest, swarm buffer, repair,    no I/O
                 (later) DHT, gossip, incentives, relays.
  bs-sim         Discrete-event simulator, scenarios, KPIs, invariants.
  bs-ingest      Real media producers: ffmpeg fMP4, file pacing.       tokio
  bs-node        Real runtime: UDP + quinn driver, `bsnode` binary.    tokio
  bs-player-api  Local HTTP/WebSocket to hand media to players.        tokio
py/              Report analysis, fleet orchestration, second wire decoder.
clients/web      MSE player.        clients/desktop  Tauri around bsnode.
fleet/           Docker + compose + netem: many real nodes, one command.
scenarios/       Simulator scenario files (TOML).
scripts/         check.sh — the single test entrypoint.
docs/            TESTING.md, wire_coverage.md, design notes.
```

Dependency direction is strictly downward: `wire ← crypto ← media ← core ←
{sim, node}`. `bs-wire` depends on nothing internal. No crate below `core`
depends on tokio.

### 2.1 bs-wire

Byte-exact encode/decode for every structure in Appendix D. Types are small and
plain: `NodeId`, `StreamId`, `Hash`, `TreeId`, `TreeSet`, `SegmentSeq`,
`BlockIndex`, `PeerRecord`, `StreamRecord`, `TreeMappingEntry`,
`ValidationBlock`, and one struct per frame. `Frame` is the enum over them with
whole-frame encode/decode. Frames not yet implemented decode into `Frame::Raw`
so a peer can log and ignore them rather than fail.

Signed structures expose `signable_bytes()`; the crypto crate signs exactly that.
This is the only mechanism by which signature coverage is defined, so the two
crates cannot drift apart.

Golden vectors in `tests/vectors/frames.json` are the interoperability contract.
The Rust codec and the Python decoder both check against them.

### 2.2 bs-crypto

`Identity` = Ed25519 key + static-puzzle nonce; its NodeID is the puzzle output.
`pow` implements the static and dynamic puzzles, the swarm-size tiers and the
"discounts are alternatives" rule with the verifier's grace band. `MerkleTree`
is the Blake3 tree over 16 KB blocks with zero-leaf padding to a power of two.
`rendezvous` computes tree assignment. `Difficulty::TEST` makes identities mint
in microseconds for simulation; `Difficulty::SPEC` is the real thing.

### 2.3 bs-media

The producer-side pipeline and the consumer-side reassembly, both pure:

```
Source ──LayeredChunk──▶ ChunkBuilder ──▶ { blocks[], MerkleTree, Manifest(signed) }
                                               │
                                               ▼ per block, per link
                                          FecEncoder (K=16 systematic + E parity)
                                               │
                     ... network ...           ▼
                                          SymbolCollector ──▶ verified block ──▶ ChunkAssembler ──▶ Sink
```

* `LayeredChunk { segment, chunk_index, layers: Vec<Bytes> }` is the unit every
  source produces every 250 ms. Layers are opaque.
* `ChunkBuilder` pads each layer to 16 KB blocks, numbers them layer-major
  (`j` over all of L0, then L1, …), builds the Merkle tree and the `MANIFEST`
  body with `LayerBlockCount[]`, and signs it.
* `ladder` holds the layer bitrates; `mapping::allocate(M, ladder)` implements
  the minimax allocation rule of Ch1 §1.2.4 §4.2.1 and produces the slicing
  matrix; `mapping::tree_for_block(manifest, matrix, j)` answers "which tree
  carries block j".
* `fec`: one RaptorQ source block per Merkle block. Encoding is systematic; the
  decoder takes the concatenation fast path when all 16 source symbols are
  present and only invokes RaptorQ otherwise. Repair ESIs for pull responders
  start at the per-node derived base of Ch4 §4.1.2.
* `source`: `SyntheticSource` (deterministic pseudo-random bytes at a target
  ladder) and `FileSource` (an `.mp4` or any file read as opaque bytes and
  paced at a target bitrate, split across layers by the ladder proportions).
* `sink`: `ChunkAssembler` rebuilds each layer's byte stream in order and
  exposes a running Blake3 hash. A viewer's hash is compared with the
  publisher's; equality is the end-to-end data-correctness check that every
  scenario asserts.

The file source is deliberately dumb in M1: it treats the `.mp4` as bytes, so a
viewer that reassembles it produces a byte-identical file. Real container-aware
ingest (fMP4 fragments from ffmpeg) arrives in M2 in `bs-ingest`.

### 2.4 bs-core

One `Node` per peer. The whole protocol lives here and nothing else.

```rust
pub enum Input  { Frame { from: PeerAddr, transport, frame }, SessionOpened, SessionClosed,
                  Timer(TimerId), Cmd(Command), Now }
pub enum Output { Send { to, transport, frame }, OpenSession(PeerAddr), CloseSession(PeerAddr),
                  SetTimer { id, at }, Deliver(LayeredChunk), Event(Event) }

impl Node {
    pub fn handle(&mut self, input: Input, now: Instant);
    pub fn poll_output(&mut self) -> Option<Output>;
    pub fn next_timer(&self) -> Option<Instant>;
}
```

Modules, matching the specification's chapters:

| Module | Spec | Responsibility |
|---|---|---|
| `params` | App B | Every constant, by spec name; `Params::default()` = spec, overridable |
| `lifecycle` | Ch1 §1.3 | BOOTSTRAP → DISCOVERY → JOINING → CONNECTING → ACTIVE (+ per-tree CHURN_REPAIR) → TERMINATED, subscribed vs assigned tree sets |
| `forest` | Ch1 §1.2 | Per-tree parent, children, depth, `K_v(m)`, warm-up gating, admission (depth, free slot, leaf share, rank), drain path |
| `join` | Ch1 §1.2.2, Ch3 §3.3 | Candidate probing, multivariate score, NEIGHBOR/ACCEPTED/rejection handling, retry/backoff, shed rule |
| `swarm` | Ch4 | Manifest acceptance by sequence window, symbol collection per block, verify-then-forward, playout timeline, retention window, (M3) PULL zone |
| `liveness` | Ch3 §3.3 | Heartbeat on idle, RTT-scaled eviction, per-tree repair |
| `publisher` | Ch4 §4.1, Ch3 §3.3.2 | Signs manifests, paces symbols over ≤ ½ chunk period, serves the base-layer reserve |
| `discovery` | Ch2 | M1: `DiscoveryOracle` trait answered by the simulator. M3: S/Kademlia |
| `membership` | Ch3 §3.1–3.2 | M3: HyParView active/passive sets, shuffles, gossip |
| `incentives` | Ch5 | M3: Tit-for-Tat, receipts, RANK_PROOF, audits |
| `relay` | Ch6 | M4: emergent relays, NAT signalling |
| `events` | — | Structured events for logs, KPIs and `--explain` |

`Instant` is a `u64` of microseconds owned by the driver. Core never reads a
clock; it receives `now` with every input and asks for timers with
`SetTimer`. Randomness is a `ChaCha` RNG seeded by the driver.

### 2.5 bs-sim

A discrete-event simulator that drives N `Node`s with a virtual clock.

* **Network model.** Each node has a region coordinate; link latency is base
  RTT by region distance plus jitter, per-link loss probability, and a per-node
  egress rate that queues outgoing frames (so a saturated uplink shows up as
  latency and loss, as it does in reality).
* **Discovery oracle.** Implements the `DiscoveryOracle` trait from `bs-core`:
  answers GET_PEERS with a uniformly random sample of registered relays for the
  wanted trees, exactly as a guardian would, without the DHT. Swapped for the
  real DHT in M3 without touching node logic.
* **Scenario.** A TOML file: node count and class mix, upload capacities, join
  schedule, churn events, loss profile, media ladder, duration, KPI thresholds,
  seed. `scenarios/` ships the Chapter 8 stress cases.
* **KPIs.** PSR, SJL, CDO, mean/max hop depth per tree, repair time
  distribution, bytes by frame type, output-hash equality per node.
* **Invariants.** Checked after every event: verify-before-forward; depth ≤
  `D_max`; one parent per tree; edge-disjoint trees; no symbol before its
  manifest; heartbeat gap bound; monotone live edge; `K_avail` never exceeds
  `K_v`. A violation halts the run with the offending node's recent event
  window.
* **Reports.** `results/<scenario>/{verdict.json, kpis.json, events.jsonl,
  topology.json}`. `bssim explain --node N --segment S` renders one node's
  timeline for one segment.

### 2.6 bs-node

The real driver. A UDP socket for pre-session frames; a quinn endpoint for
sessions, with one bidirectional stream per tree plus a control stream, and
QUIC datagrams for symbols. Certificates are self-signed and bound to the
Ed25519 identity, verified by a custom rustls verifier that checks the peer's
NodeID rather than a CA. The driver loop selects over socket reads, quinn
events and the node's next timer, and translates `Output`s into socket writes.
`bsnode publish --file x.mp4` and `bsnode watch --out y.mp4 --expect-hash H`
are the M2 smoke test.

### 2.7 bs-ingest and bs-player-api

`bs-ingest` produces `LayeredChunk`s from real media: the M1 file pacer, then
ffmpeg fMP4 fragments with a VP9 temporal-layer profile (additive layers) and a
simulcast profile (resolution switching). `bs-player-api` exposes the
reassembled stream over HTTP/WebSocket for a Media Source Extensions player and
serves Prometheus metrics.

---

## 3. Data path, end to end

1. Source emits a `LayeredChunk` every 250 ms.
2. Publisher's `ChunkBuilder` cuts blocks, builds the Merkle tree, signs a
   `MANIFEST`, and hands blocks to the forest by tree per the slicing matrix.
3. For each tree and child: `BLOCK_PROOF` (stream) then `RAPTORQ_SYMBOL`
   datagrams, paced over ≤ 125 ms per chunk.
4. A relay collects symbols per block; at 16 source symbols it concatenates,
   otherwise it decodes; verifies the block against the manifest root through
   the proof; re-encodes with parity sized per child link; forwards. Depth is
   updated from `SenderHopDepth + 1` on every block.
5. The playout scheduler releases each chunk 3.0 s behind the live edge; the
   `ChunkAssembler` appends layers to the sink; the running hash is the
   correctness witness.
6. Per segment per tree the child issues a receipt (M3) and the parent updates
   its roster (M3).

---

## 4. Testing framework

See [`docs/TESTING.md`](docs/TESTING.md) for the full plan. In one paragraph:
`scripts/check.sh` runs fmt, clippy, unit and property tests, golden vectors,
the Python cross-decoder, core state-machine tests, then the simulator's gate
scenarios, and writes `results/check.json`. Every test names its spec section.
Every failure carries a seed and a repro command. The simulator's invariant
checker and `explain` mode turn a bad KPI into a specific node, tree, segment
and event sequence.

---

## 5. Running many nodes

Two ways, same code:

* **Simulator** (`bssim run scenarios/baseline_100.toml`): hundreds to
  thousands of nodes in one process, virtual time, deterministic. This is
  where protocol logic is validated.
* **Fleet** (`fleet/up.sh --viewers 20 --profile lossy`): one publisher and N
  real `bsnode` containers on a docker network with netem-shaped links, each
  writing its reassembled output and hash to a shared volume; `fleet/report.py`
  produces the same JSON report as the simulator. This is where transport,
  timing and resource use are validated.

---

## 6. Spec decisions made by the implementation

| Decision | Where | Issue |
|---|---|---|
| Refused tree joins are answered with `DISCONNECT` reasons `0x0A–0x0C`; `DISCONNECT` carries a `TreeID` (0 = whole connection) | `bs_wire::DisconnectReason`, `frames::Disconnect` | SOLUTION-059 |
| Validation-block signature pre-image is `FrameType ‖ Timestamp ‖ body`, hashed with Blake3 | `ValidationBlock::signed_message` | (doc comment; spec says "hash of payload and timestamp") |
| Merkle trees pad to a power of two with zero leaves | `bs_crypto::MerkleTree` | (doc comment; spec fixes this only for RANK_PROOF) |
| A relay assigned to tree m displaces a pure subscriber of m through the drain path (`DRAIN_NOTICE` Displaced); it also asks saturated parents for its own tree | `Node::on_neighbor`, `Node::finish_probing` | ISSUE-061 |
| Sequential handover: below `10·B_m·Ω` uplink a displacing admission is `ACCEPTED.PENDING` and the new child is served when the drained child leaves; the drain window is then one segment, not τ_drain | `Node::on_neighbor`, `release_pending` | Ch1 §1.2.2 Handover budget; ISSUE-061 addendum |
| Playout anchors at the newest manifest seen before the first chunk plays; never re-anchors backward | `SwarmBuffer::insert_manifest` | Ch4 §4.3.1 |
| Opaque sources size every non-final layer to whole 16 KB blocks so layer boundaries are recoverable from `ChunkByteLength` alone | `bs_media::source::aligned_layer_sizes` | ISSUE-060 addendum |
| A parent that delivers three unverifiable blocks is evicted and excluded from future candidate lists | `Node::try_verify` | Ch4 §4.1.2 "the delivering parent is flagged" |
| A newly admitted child receives the manifests of the two most recent segments | `Node::send_recent_manifests` | Ch4 §4.3.1 (backfill via MANIFEST_REQUEST in M3) |
| Layer padding is stripped with the manifest's `LayerByteLength` (SOLUTION-060); the interim length prefix is gone | `bs_media::chunk::strip_layer` | App D §D.4.8 |
| `STREAM_DESCRIPTOR` is signed by the publisher, pushed to every admitted child once per version, forwarded by relays, requested by a joiner with `MANIFEST_REQUEST(0xFD)` on its first attach, and handed to the application as `Output::Descriptor` | `Node::set_descriptor`, `on_descriptor`, `on_manifest_request` | App D §D.4.20 |
| Real media is carried as an app-level TLV container inside each layer payload (INIT once per segment in chunk 0, MEDIA per fragment); renditions are layers (`SIMULCAST`). The player still reads INIT from the TLV; moving it to the descriptor's `InitData` is the next step | `bs_ingest::container`, `bs-player-api`, `clients/web` | SOLUTION-060 |
| A join round of pure timeouts is a discovery failure, not a shed signal; only refusals (saturated, depth, unparented) advance the shed counter | `Node::join_round_failed` | App D §D.4.3b |
| Plain pre-session frames and QUIC share one UDP socket, demultiplexed on the QUIC fixed bit; RFC 9287 greasing is disabled | `bs_node::transport::DemuxSocket` | App D §D.2 |
| QUIC sessions use mutual TLS with self-signed Ed25519 certs whose SAN carries `bitstream://<nodeid>/<nonce>`; verification is the identity binding + static PoW, no CA | `bs_node::tls` | Ch2 §2.2, Ch6 §6.2 |
| Channels map to QUIC as: control = one bidi stream tagged 0, tree m = one bidi stream tagged m, symbols = datagrams | `bs_node::driver` | App D §D.2 |
| Discovery in M2 is a single bootstrap guardian inside `bsnode publish` answering `GET_PEERS` / `REGISTER_PEER` with observed addresses | `bs_node::guardian` | Ch2 §2.3 (S/Kademlia in M3) |
| STREAM_END ends the relay role at once but playback drains the buffer before TERMINATED | `Node::on_stream_end` | Ch1 §1.3.1 |
