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
5. **Deterministic by construction.** All time and randomness are injected, and every
   collection whose iteration order can reach a decision (children, sessions, candidate
   pools, the oracle's registrations) is ordered (`BTreeMap`/`BTreeSet`), never a
   `HashMap`. Any failure reproduces from a seed; the same seed gives the same verdict.
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
| `dht` + `node/dht` | Ch2 | S/Kademlia: 256 k-buckets of reachable relay-class contacts (`RoutingTable`), α-disjoint-path iterative lookups (`Lookup`), FIND_NODE/FIND_VALUE/REGISTER_PEER/GET_PEERS/STORE_RECORD servers, guardian storage for the stream, sampled registration, the publisher's STORE_RECORD/ACK loop. `DiscoveryMode::Oracle` keeps the M1 simulator oracle for A/B runs |
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
* **Discovery.** By default (`discovery = "dht"`) every simulated node runs the
  real S/Kademlia DHT over plain-UDP frames through the same latency/loss model as
  media; the publisher is the only bootstrap seed. `discovery = "oracle"` keeps the
  M1 oracle (a random sample of registered relays answered in one flat RTT) for
  A/B comparison of tree behaviour without discovery effects.
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
| Real media is carried as an app-level TLV container inside each layer payload: one MEDIA record per fMP4 fragment; the INIT segment and a META record (fps, height) appear only in a rendition's first chunk, from which the publisher builds the STREAM_DESCRIPTOR. Players get initialisation data from the descriptor (`/api/descriptor`, kind-1 WebSocket frames on connect), so a late joiner starts without waiting for a segment boundary. Every rendition carries its own AAC track (`avc1.…,mp4a.40.2`) | `bs_ingest::container`, `bs-player-api`, `clients/web` | SOLUTION-060 |
| A join round of pure timeouts is a discovery failure, not a shed signal; only refusals (saturated, depth, unparented) advance the shed counter | `Node::join_round_failed` | App D §D.4.3b |
| A WARMING candidate exempts a join round from the shed counter for at most 4 consecutive rounds; after that warming counts as saturation. Unbounded, the exemption livelocks joiners in a tree at capacity because someone is always warming | `Node::join_round_failed`, `Params::warming_exempt_rounds` | Ch1 §1.2.2 table; ISSUE-063 |
| Depth propagation: when a block's `SenderHopDepth + 1` puts a relay at `h ≥ D_max` it drains its children in that tree (`DRAIN_NOTICE` DEPTH) and re-selects its own parent; the simulator checks admission depth as a hard invariant and propagated depth as a soft one, since a branch unwinds over a few segments | `Node::on_block_proof`, `drain_children` | Ch1 §1.2.2 Depth Propagation |
| Bootstrap has no PING round: the self-lookup's first FIND_NODE goes straight to the seeds as address-only placeholders whose ids the answer fills in. A contact learned from a FIND result is pinged before insertion; a contact that answered our RPC is inserted directly (both prove reachability) | `Node::dht_start`, `dht_learn`, `dht_insert_responsive` | Ch2 §2.1.3 steps 2–4, §2.3.2 rule 1 |
| Discovery is a FIND_VALUE lookup toward `K_s` that stops at the first α record holders (the guardians reached); they get the GET_PEERS and the answers are unioned. A full FIND_NODE lookup runs in the background to resolve the k closest contacts for REGISTER_PEER; that set is cached 30 s | `Node::dht_discover`, `Purpose::Discovery` / `Purpose::Guardians` | Ch2 §2.1.2, §2.1.3 step 5, §2.3.2 |
| Each disjoint path keeps α queries in flight, and a contact learned on one path is handed to the idlest path (still claimed once), so a single seed fans out to all paths from the second hop | `bs_core::dht::Lookup::poll`, `on_response` | Ch2 §2.1.2 |
| A node answers FIND_VALUE(K_s) with the Stream Record only while it is among the k closest contacts it knows of; otherwise it refers. Every reachable relay stores registrations and records sent to it | `Node::is_guardian_candidate`, `GuardianStore` | Ch2 §2.3.2 "Who Is a Guardian" |
| Registration is refreshed at τ_ttl/2 and immediately on a record change; an unchanged record inside the window is not re-sent (an early build re-sent on every slot change: 39k REGISTER_PEERs in a 60 s run) | `Node::register_self` | Ch2 §2.3.2 step 4 |
| The publisher folds the median of STORE_RECORD_ACK counts into `SwarmSize`/`RelayCount` (scaled by 2^s) and derives `RegisterSampleLog2 = max(0, ⌈log2(N/N_0)⌉)` | `Node::dht_store_record` | Ch2 §2.3.2 |
| A NEIGHBOR is admitted only on an open session; the simulator discards frames in flight on a closed connection as QUIC does. Without both, an Active Set eviction racing an in-flight NEIGHBOR admitted a child into a dead connection (24 starved chunks at one node in baseline_100) | `Node::on_neighbor`, `bs_sim::Sim` Deliver | Ch3 §3.1.3 |
| A viewer still in DISCOVERY without a Stream Record re-queries every second (a lost GET_PEERS answer would otherwise strand it, since join rounds need the matrix) | `Node::dht_timers` | Ch2 §2.3.2 Query Discipline |
| One PROOF_OF_UPLOAD per (segment, tree, provider), issued when the first block of the next segment arrives from anyone (and at the TFT tick for anything two segments behind the edge); PULL receipts use TreeID 0 with `RxFlags.PULL`. `LossRate` is estimated from symbols received against `K + default parity` per pushed block over 2 s | `Node::note_delivered`, `issue_receipt` | Ch5 §5.2.1, §5.2.3 |
| Receipts verify against keys learned from validation blocks (PROBE, JOIN, PING/PONG, DHT RPCs); every new session is PINGed once so both ends learn each other's key. Unknown-key receipts are counted (`receipts_rejected_by_reason`) and dropped | `Node::learn_key`, `on_receipt` | Ch5 §5.2.1; the spec's "resolve the key from the NodeID" has no mechanism (ISSUE-065 pending) |
| Receipt deadline: segment k's receipt is due before the first block of k+2 is pushed; the admission segment counts as receipted. A miss zeroes the peer's ingress for the next TFT cycle; three consecutive misses evict (`DISCONNECT` EVICTION, tree-scoped), and the identity and its /24 are refused at this parent for 60 s | `Node::check_receipt_deadline`, `admission_banned` | Ch5 §5.2.1 |
| Adaptive parity per child from its `LossRate`: `E = max(1, ⌈clamp(2ρK, 0.05K, 0.30K)⌉)`; the mean parity sent enters Ω and `K_v(m)`, and a fallen `K_v` releases the newest child through the drain path (CAPACITY) | `Node::apply_loss_report`, `shed_capacity`, `mean_child_parity` | Ch4 §4.2.2, Ch1 §1.2.1 |
| Tit-for-Tat every 500 ms over the Active Set and tree peers: `PullSlots = ⌊(r_pull·u − handovers·B_mΩ)/β_pull⌋`; regular slots to the top reciprocators by 2 s PULL ingress; the remainder to *waiting* zero-contributors (peers that asked us for PULL service), FIFO by first ask, in 2 s optimistic windows with a 30 s NodeID+/24 cooldown on failure. A waiting peer takes a free optimistic slot at once. CHOKE_STATE is sent on every change | `Node::tft_cycle`, `pull_allowed`, `try_optimistic_now` | Ch5 §5.1.2–§5.1.3; the "waiting" reading of J_new is ours |
| A child's PULL for a block of the tree we push to it is served regardless of choke state (its receipts pay for those bytes; the loss was on our link) | `Node::pull_allowed` | ISSUE-066 |
| The requester asks providers that unchoked it first, but still asks a choked provider: the request is refused and doubles as the bootstrap-queue "waiting" signal | `Node::schedule_pulls` | Ch5 §5.1.3 |
| Data stall: a parent that delivered before and pushes nothing for 2 s is treated as lost for that tree; a relay unparented for 2 s with children drains them (CAPACITY). Both keep an evicted or banned free rider from holding its honest subtree hostage | `Node::run_timers` | ISSUE-066 addendum |
| Depth propagation ignores PULL-path proofs (`SenderHopDepth = 255`); a relay's pre-loss depth is kept for the cycle guard, which skips candidates deeper than us while we hold children, and equal-depth ones while we still hold a parent | `Node::on_block_proof`, `finish_probing`, `TreeState::last_depth` | Ch1 §1.2.2; found by the free-rider gate (tree cycles) |
| The simulator gives every node its own /24: prefix-keyed cooldowns and caps otherwise treat the whole swarm as one household | `bs_sim::addr_for` | Ch2 §2.2.2, Ch5 §5.1.3 |
| A RaptorQ decode that fails with ≥ K symbols is *incomplete*, not poisoned: RFC 6330 decodes from exactly K symbols ≈ 99 % of the time. The symbols are kept and one more is awaited (the PULL zone asks for it); only a Merkle mismatch flags the parent. Before this, a 1 % decode failure counted toward the three-strike poison rule and honest parents were dropped under any loss | `SwarmBuffer::try_verify` | Ch4 §4.1.2, §4.2; found on the docker fleet (lossy profile) |
| The data-stall clock is per tree and restarts on every attach and detach; a new parent is never judged by the previous parent's last block | `TreeState::last_block_at`, `Node::on_accepted`, `detach_parent` | found on the docker fleet: instant "data stall" after each re-attach |
| `bsnode`'s run report carries the incentive counters (receipts issued/accepted/rejected by reason, deadline misses and evictions, choke changes, choked pulls, receipts held, rank) and logs receipt misses and child removals, so a fleet run can be diagnosed from `result.json` alone | `bs_node::driver::IncentiveCounters` | — |
| QUIC connections use BBR, not Cubic. Symbols are QUIC DATAGRAM frames and therefore congestion controlled (RFC 9221); a loss-based controller reads the random loss FEC exists for as congestion and, at 2 % loss and 40 ms RTT, held every connection near 2 Mbps, below one stripe: the docker fleet starved 99 % of chunks with Cubic and 1.9 % with BBR, nothing else changed. Initial MTU 1280. The run report lists per-connection RTT, cwnd, loss and DATAGRAM counts, and `datagram_send_errors` | `bs_node::tls::quic_configs`, `RunReport::connections` | App D §D.2 (transport classes); found on the docker fleet, lossy profile |
| RANK_PROOF: the prover sorts its window receipts by canonical key, pads a one-receipt list with the zero leaf, commits to the Merkle root one segment before the newest manifest it holds, and uses that manifest's root as the nonce. Eight adjacent pairs are sampled with a one-byte index. Each pair carries the signers' keys and static nonces (ISSUE-065). The verifier recomputes the rank from `ReceiptCount` and the sampled capped bytes; a failed cryptographic check bans the prover for the eviction cooldown, an unknown nonce manifest does not | `bs_core::node::rank` | Ch5 §5.2.2, App D §D.4.17 |
| The proof precedes the NEIGHBOR on the control stream and a verified rank stands for 10 s, so admission is decided in one step (ISSUE-067) | `Node::send_neighbor`, `Params::rank_proof_ttl` | App D §D.4.17 |
| Rank admission case 2: in an enhancement tree with no free slot, a joiner whose rank beats the weakest non-draining child by 1.25× preempts that child through the drain path (PREEMPTED), one handover at a time. Children carry the rank they proved at admission; a child without a proof ranks 0. Base-layer slots are never preempted | `Node::on_neighbor`, `Node::weakest_child`, `ChildLink::rank` | Ch1 §1.2.2 Rank Admission Rule |
| Two guards close the two-node cycle that concurrent re-selection opens: a parent never admits its own parent in that tree, and a joiner drops an ACCEPTED from a node that became its child meanwhile (DISCONNECT_CHOKE). Longer cycles are caught by the depth guard and the DEPTH drain | `Node::on_neighbor`, `Node::on_accepted` | found by the flash-crowd gate after rank preemption raised re-selection churn |
| A child that sends NEIGHBOR to its current parent again has left from its own point of view: the old link goes and the request is judged like any other, over the same session. Closing the session there cost the baseline gate 14 repairs per minute | `Node::on_neighbor`, `detach_child_link` | Ch1 §1.2.2 |
| Upward migration: every τ_migrate (5 s) an attached relay at depth ≥ 2 probes its candidates and moves only to a parent that places it strictly shallower; the old parent drains as in any handover | `Node::migration_round`, `TreeState::migrating` | Ch1 §1.2.3 |
| RANK_PROOF nonce and window for third-party verifiers: the nonce is the manifest two segments behind the prover's edge, the commitment is one segment before it, and the window is the 60 segments ending at the commitment at every verifier (ISSUE-069). The subnet penalty counts each sampled signer once | `Node::build_rank_proof`, `verify_rank_proof`, `subnet_penalty` | Ch5 §5.2.2, §5.3.2 |
| Reputation auditing: every verified proof (at admission or attached to gossip) feeds the sampled receipts into an audit log kept for τ_accuse. A sampled receipt whose reverse (same segment, same tree, the other way) is in the log is a symmetric pair when the two bitmaps claim a common block and the bytes agree within 10 % (ISSUE-068); PULL receipts are excluded. The detector is off below N_collusion peers (`N_guard` = max of record and peers seen) and the confidence is `1 − degree/opportunity` per Ch5 §5.3.1. A receipt that replaces one with the same key and a different bitmap is an equivocation by its signer | `bs_core::node::audit` | Ch5 §5.3.1, §5.3.3 |
| Accusations are signed, carry both receipts with the signers' keys, the accuser's key and static nonce, and the accuser's own RANK_PROOF unless the receiver is a tree neighbour; they go to the Active Set and each is made once per suspect per τ_accuse. A receiver rate-limits per accuser (4/min), checks the key against the NodeID, takes the accuser's standing from its child rank or the attached proof (≥ r_accuser), resolves its prefix from the session, membership, or a DHT lookup (`Purpose::Resolve`, the accusation parked up to 5 s), re-verifies the evidence (a forgery bans the accuser), counts distinct accusers by prefix and forwards each counted accusation once. Three accusers from three prefixes evict: the suspect loses its tree links, Active and Passive membership and session, and is refused for τ_ban, doubling per strike up to 24 h | `Node::accuse`, `on_audit_gossip`, `audit_evaluate`, `audit_evict` | Ch5 §5.3.3, App D §D.4.18 |
| Forest resize (Ch1 §1.2.4 §4.5): the source sizes M from the guardians' relay count on the spec ladder with 3 relays of shrink hysteresis and the 30 s dwell (a flash crowd at twice the next threshold pre-empts it), and announces a `MANIFEST_UPDATE` five segments ahead that carries the last manifest body under the new version. Peers verify it against the pinned key, forward it once, extend their tree list, join trees the new layout needs, and a relay whose rendezvous tree changes drains its old children (REASSIGNED, deadline = the switch) while it joins the new tree. New trees are pre-emitted: every relay pushes a block down the pending layout's tree as well, and receipts and liveness are credited to the tree a block arrived on. At `EffectiveSegmentSeq` every peer switches, closes trees that no longer exist and recomputes roles and slots; failed rounds in every tree are in grace until the switch plus D_max segments. A peer that missed the announcement fetches it with MANIFEST_REQUEST(0xFE) after a manifest with an unknown version, and the record's pending matrix covers a joiner inside the window | `bs_core::node::resize` | Ch1 §1.2.4 §4.5, §1.2.1 §1.4; ISSUE-071 |
| Layer fold (Ch1 §1.1.5 §5.6): the guardians count starved re-queries (a failed round with saturated candidates and no WARMING one sends GET_PEERS with the starved bit, at most every 5 s per tree), the source estimates the starved share per tree from the acknowledgements, and folds the top layer when a lower layer's share exceeds s_fold for τ_fold (10 s for the base layer). The history is cleared at each switch and measured only after the fill-in; a folded layer is not encoded; an unfold needs 50 % relay growth since the fold. The share is the sum of the guardians' counts divided by α, not the spec's median: our GET_PEERS go to the α record holders, so most guardians report zero and the median hides the starvation | `Node::forest_consider`, `forest_note_acks` | Ch1 §1.1.5 §5.6; ISSUE-071 |
| The manifest window ahead grows with the age of the live edge, so a peer starved for a while accepts current manifests again (ISSUE-070) | `SwarmBuffer::accept_manifest` | Ch7 §7.1.2 |
| Registration reaches every guardian: a node re-registers when a guardian lookup finds guardians that do not hold its record, refreshes the guardian set every 30 s (every 5 s while the set is short), and the stream record does not name the matrix version, so a joiner takes it from the first manifest. Before this the guardians' relay count lagged a join burst by 60 to 90 s | `Node::register_self`, `dht_timers` | Ch2 §2.3.2 |
| NAT model (Ch6 §6.1): every node carries a `Reachability`. A node configured without one decides it from two PONG reflections (one mapping equal to its own address = PUBLIC, one shared mapping = CONE, one per responder = SYMMETRIC) and re-registers when it changes. A SYMMETRIC node acts leaf-class. The simulator drops a packet into a CONE or SYMMETRIC host unless that host sent to the sender within the mapping TTL (30 s), and a SYMMETRIC sender's packet into a CONE host passes for the pair fraction `symmetric_cone_punch` gives (the spec's 95 % and 10 %) | `bs_core::node::nat`, `bs_sim::Network::can_reach` | Ch6 §6.1, Ch2 §2.1.3 |
| Hole punching (App D §D.4.14): the node remembers who named each record (the guardian's GET_PEERS answer or the neighbour's SHUFFLE or GOSSIP_EXCHANGE). Before it PROBEs a CONE candidate it sends the request form of PUNCH_REQUEST to that referrer, which forwards the requester's observed address to the target; the target PINGs the requester, and the PING re-triggers the PROBE. A CONE registrant PINGs its guardians every τ_nat (25 s) so they can reach it. Both forms carry the validation block on every channel | `Node::punch_if_needed`, `on_punch`, `reprobe_after_punch`, `nat_timers` | App D §D.4.14 |
| Emergent relays (Ch6 §6.3): a PUBLIC relay with 20 Mbps or more and the configured uptime advertises RELAY_CAPABLE. A NAT-blocked peer whose probed parents never answer for two rounds asks a RELAY_CAPABLE record it holds, else GET_PEERS with WANT_RELAY_CAPABLE, and sends RELAY_PROPOSAL with a zero target. The relay binds while bridges stay under half its tree budget and it holds (or first joins) the tree, answers RELAY_BIND, and admits the peer as a bridge child outside its assignment; bridge slots come on top of its own, whose count is computed with the bridge load removed, and an own child above the remaining count drains. The bridged peer's receipts carry RELAYED (weighted 3×). A bridge whose child never joins expires; a proposal without an answer tries the next relay; distinct relays are preferred per tree | `Node::request_bridge`, `on_relay_proposal`, `on_relay_bind`, `recompute_slots` | Ch6 §6.3.1–§6.3.3 |
| Slot headroom: `K_v` keeps 5 % of the uplink free, so a full relay does not run at 100 % and starve the children queued last (ISSUE-072) | `Params::slots`, `slot_headroom` | Ch1 §1.2.1 §1.3 |
| A relay that leaves a tree at the next switch admits nobody new there, so a child never holds it in both its old and its new tree | `Node::on_neighbor`, `TreeState::reassigning` | Ch1 §1.2.4 §4.5 step 3 |
| Data stall covers a parent that never delivered: a fresh or PENDING parent that pushes nothing for three stall windows is lost, where a parent that stopped is lost after one | `Node::run_timers` | Ch3 §3.3.2 |
| The cycle guard also runs on ACCEPTED: while a node holds children in a tree, a parent that reports a depth greater than the node's own last depth can be its descendant and is refused. A bridge relay was never probed, so the probe-time filter alone let a relay downstream of a stale subtree bridge its own ancestor into a three-node cycle | `Node::on_accepted` | Ch1 §1.2.2 cycle guard |
| A tree whose parent was attached as the repair is not probed again until that parent fails: re-probing on every timer pass made a node leave its new parent before the first block and re-join it in a loop (seen as five attachments in a second in the baseline gate) | `TreeState::needs_parent_search` | Ch1 §1.2.2 |
| Source behind a NAT (Ch6 §6.3.3): a publisher whose reachability is not PUBLIC asks the guardians for RELAY_CAPABLE records and sends each a RELAY_PROPOSAL with SOURCE_INGRESS, no more than its weakest tree holds slots (two by default). A relay that holds at least two slots in every tree and whose session peer is the publisher named by the Stream Record becomes hop 0: the publisher is its parent in every tree, it relays every tree, registers with SOURCE_INGRESS, and the publisher pushes all M slices to it as a child. Joiners take an ingress record as the source and never probe a SYMMETRIC source. Such a source cannot seed the DHT, so the simulator bootstraps everyone from the first viewer, which must be public and start first | `Node::ingress_timers`, `on_ingress_proposal`, `on_ingress_bind`, `bs_sim::Sim::bootstrap_addr` | Ch6 §6.3.3 |
| A discovery with nobody to ask ends on the timer, never synchronously: ending it in the call that started it re-entered the join and looped without bound on the DHT seed | `Node::dht_begin_guardian_lookup` | Ch2 §2.1.3 |
| Connection migration (Ch6 §6.2) is QUIC-native: quinn validates a new path on a client address change and keeps the connection id, so `bsnode` needs no frame for it. The simulator does not model address changes | `bs_node::tls::quic_configs` | Ch6 §6.2 |
| DHT answers that arrive after the RPC timeout still teach the joiner: a late FIND response from a contact asked within 5 s enters the routing table with its contacts, and a late GET_PEERS answer is used as the discovery result. A contact behind a saturated uplink (the publisher during a join storm) answered every request after the 400 ms timeout, and joiners that knew only that contact re-asked it for 15 to 20 s | `Node::dht_on_late_find_response`, `dht_on_get_peers_response` | Ch2 §2.3; found by the collusion gate (join latency) |
| PULL zone: bitfields for the last three segments every τ_gossip to every session peer; the scheduler runs every τ_sched over chunks within `W_pull` of their deadline, asks the best-RTT provider whose bitfield shows the block (whole block below 8 symbols, repair symbols above), broadcasts a rarity want at most every 5·τ_sched, and only for trees the node is attached to, repairing, or serving children in (a tree still in its initial join is not pulled: that turns repair into a second distribution path) | `bs_core::node::pull` | Ch4 §4.3 |
| The sender of a manifest is an implicit provider for that chunk (its own bitfield arrives up to τ_gossip later); a joiner requests the previous segment's manifests on its first attach and pulls them (late-join backfill) | `Node::note_implicit_provider`, `backfill_from` | Ch4 §4.3.1 steps 3 and 5 |
| Chunks before the first chunk pushed to a node are *backfill*: best effort, reported separately, excluded from steady-state PSR | `Node::first_push_chunk`, sim `backfill_*` facts | Ch4 §4.3.1 ("PUSH delivery from X+1 onward") |
| HyParView: JOIN goes over UDP to θ_join contacts while the joiner opens QUIC sessions to them; the contact admits on session open, answers ACCEPTED(JOIN) and starts the FORWARD_JOIN random walk (TTL 4, PR_gossip 2); NEIGHBOR(TreeID 0) HIGH is always admitted (evicting a random member with no tree role), LOW only with room; the Active Set never evicts a parent, child or draining parent | `bs_core::node::membership` | Ch3 §3.1–3.2, App D §D.4.2/§D.4.5 |
| GOSSIP_EXCHANGE HMAC is keyed Blake3 over the payload with a 32-byte key both ends derive from the QUIC handshake (`export_keying_material`, label `bitstream gossip v1`); the simulator derives it from the seed and the address pair | `Input::SessionOpened.session_key`, `membership::gossip_hmac` | Ch3 §3.2.2 |
| Per-tree Passive pools below the floor of 3 trigger a direct SHUFFLE(TTL 0) request to a neighbour and a `GET_PEERS(WantedTrees)` re-query at most every 30 s per tree | `Node::refill_pools` | Ch3 §3.1.1, §3.3.1 tier 3 |
| Bulk pull responses ride a dedicated reliable stream (`Channel::Pull`, tag 0xF0) so 16 KB blocks never head-of-line block NEIGHBOR/ACCEPTED on the control stream | `bs_core::Channel::Pull`, `bs_node::driver` | App D §D.2 lists BLOCK_TRANSMISSION under control traffic; separated for latency |
| `f_frame` is 0.085 as Ch1 §1.2 §1.3 states (Ω = 1.15 at the parity floor); App B's 0.03 is stale. Measured per-symbol framing is 6–7 %, the rest is the control plane's only headroom | `Params::default` | ISSUE-062 addendum |
| PULL service is budgeted at `r_pull` of the uplink, paced to a 100 ms burst; requests past their urgency deadline expire (`pulls_refused`), a directed request for a block we do not hold is silent (`pulls_unheld`). The requester asks each provider at most once per block, retries at 300/600/1200 ms, gives up after 4 attempts, and picks at random among providers within 20 ms of the best RTT | `Node::pull_budget_take`, `Node::schedule_pulls` | Ch1 §1.2.1 §1.3, Ch4 §4.3.3 |
| Plain pre-session frames and QUIC share one UDP socket, demultiplexed on the QUIC fixed bit; RFC 9287 greasing is disabled | `bs_node::transport::DemuxSocket` | App D §D.2 |
| QUIC sessions use mutual TLS with self-signed Ed25519 certs whose SAN carries `bitstream://<nodeid>/<nonce>`; verification is the identity binding + static PoW, no CA | `bs_node::tls` | Ch2 §2.2, Ch6 §6.2 |
| Channels map to QUIC as: control = one bidi stream tagged 0, tree m = one bidi stream tagged m, symbols = datagrams | `bs_node::driver` | App D §D.2 |
| Discovery in M2 is a single bootstrap guardian inside `bsnode publish` answering `GET_PEERS` / `REGISTER_PEER` with observed addresses | `bs_node::guardian` | Ch2 §2.3 (S/Kademlia in M3) |
| Parity per block is a node-wide setting (`bsnode --parity N`, fleet profiles: clean 1, lossy 4, cellular 5) until receipts carry per-link loss rates | `Params::default_parity` | Ch4 §4.2.2 (adaptive parity in M3) |
| STREAM_END ends the relay role at once but playback drains the buffer before TERMINATED | `Node::on_stream_end` | Ch1 §1.3.1 |
