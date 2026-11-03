# Trace: impl-core-loop

Walker scenario `impl-core-loop`: the reference implementation (`src/crates/bs-core`) stepped on its
injected clock through bootstrap, discovery, join rounds, first chunk playout, a parent loss and
repair, and a layer shed; every tick compared with the spec rule it implements. The walk is
calibrated against two real simulator runs (`bs-sim run scenarios/small_swarm.toml`, seed 3, and
`scenarios/churn_storm.toml`, seed 7; outputs under the session scratchpad, not committed), so the
tick times below are the implementation's own where a run supplied them and hand-computed from the
implementation's formulae otherwise. The simulator KPI definitions (`bs-sim/src/kpi.rs`) are
checked against Ch8 §8.2 in §3.

Files read: `node.rs`, `node/{membership,pull,incentive,dht}.rs`, `join.rs`, `forest.rs`, `swarm.rs`,
`params.rs`, `bs-sim/src/{kpi,sim,network,scenario,report}.rs`, `src/ARCHITECTURE.md` §6 (the
implementation's own list of spec decisions), `protocol/chapter1/1.3_peer_lifecycle/*`,
`chapter3/3.3_churn_recovery/*`, `chapter1/1.2_multi_forest_overlays/2_parent_selection_algorithm.md`,
`chapter1/1.1_scale_latency/5_capacity_adaptation.md`, `chapter4/4.3_hybrid_push_pull/*`,
`chapter8_simulation/README.md`, Appendix B.

## 1. Scenario and initial state

| Item | Value | Derivation |
|---|---|---|
| Viewer V | RELAY class, u = 10 Mbps, wants layers 0..2, starts at sim t = 1000 ms | `small_swarm.toml` group `first` (node 1) |
| Swarm at V's join | N = 2 (source + V); later N = 5 (relays) then 9 (+4 leaves) | scenario |
| Forest | M = 3 (`forest_size = 3`): tree 1 = L0 1.5 Mbps, tree 2 = L1 1.5 Mbps, tree 3 = L2 3.0 Mbps | Ch1 §1.2.1 §1.4 ladder row M = 3 (spec: 12 <= N_relay < 18; the sim pins M) |
| Omega | (1 + 1/16)(1 + 0.085) = 1.1528 at `default_parity = 1` | `Params::slots`; spec quotes 1.15 |
| V's slots | K_v(1) = floor(0.85 x 10000 / (1 x 1500 x 1.1528)) = floor(4.92) = **4** (observed `k_v: 4`) | `Params::slots` with `slot_headroom = 0.05` (ISSUE-072); spec formula gives floor(9000/1725) = 5 |
| Source slots | tree 1: floor(0.85 x 20000 / (3 x 1500 x 1.1528)) = 3; tree 3: floor(17000 / 10376) = **1** | impl uses the relay formula for the source (cold-start-n2 F6); spec K_S(3) = floor((20 - 5.2)/(3 x 3.0 x 1.15)) = 1, K_S(1) = 2 |
| theta_join | max(1, min(4, N - 1)) = 1 at N = 2 | `Params::theta_join`, Ch1 §1.3.1 |
| RTT V <-> source | ~36 ms (pair-hashed in 10..40 ms); discovery RTT 60 ms | `NetworkSpec` defaults; sim `session_open_rtts = 1.5` |
| Timers armed at start | liveness every tau_ping/4 = 25 ms; tau_sched 100; tau_tft 500; tau_gossip 1000; shuffle 3000; probe_timeout 200; NEIGHBOR deadline 3 x 200 = 600; join_backoff 250 ms x 2^min(failed,2) | `Node::start`, `request_next`, `join_round_failed` |
| Playout | Delta_buffer 3.0 s from the newest manifest seen before the first play; W_pull = clamp(tau_evict_max + 6 SRTT_max, 1.5, 2.0) | `SwarmBuffer::insert_manifest`, `Node::w_pull` |
| Churn-storm cross-check | 80 relays + 20 leaves, 3 regions (RTT 10..40 intra, 60..160 inter), 30 % of relays killed at t = 30 s | `churn_storm.toml` |

## 2. Tick table

Times are simulator milliseconds (V starts at 1000). "obs" = read from `events.jsonl`; "calc" =
computed from the implementation's constants; "spec" column gives the rule the tick is compared
with and the spec's own number.

### 2a. Bootstrap, discovery, join, first chunk (N = 2, node 1 of `small_swarm`)

| t_ms | node | rule (file §) | computation | state after |
|---:|---|---|---|---|
| 1000 obs | V | `Node::start` (Ch1 §1.3.1 BOOTSTRAP -> DISCOVERY) | dynamic puzzle solved synchronously (C2 = 2 in sim; 8 at N < 50 per App B); timers armed; `Output::Discover` | Bootstrap -> **Discovery** |
| 1074 obs | V | `on_discovered` (Ch1 §1.3.1 DISCOVERY -> JOINING "stream records fetched") | 1 record (source) + Stream Record N = 2, relays 1; matrix 3 trees; roles: subscribed {1,2,3}, assigned {rendezvous(V) = 1}; K_v(1) = 4; theta = 1; JOIN over UDP to the source + `open_session` | Discovery -> **Joining**; `joining_since = 1074` |
| 1127 calc | source | `on_join` / `admit_joiner` (Ch3 §3.1.2) | session opens after 1.5 RTT = 53 ms; `active_add`; ACCEPTED(JOIN); FORWARD_JOIN to nobody (no other actives) | source Active Set = {V} |
| 1145 obs | V | `membership_timers` (Ch1 §1.3.1 JOINING -> CONNECTING when \|A\| >= theta) | \|A\| = 1 >= 1 (impl also caps the wait at 1000 ms; spec has no cap); `start_tree_joins` -> `begin_probing` x3: candidates = {source} per tree; PROBE x3 on UDP; probe deadline 1345; `rounds = 1` | Joining -> **Connecting**; 3 x `Probing` |
| 1163 calc | source | `on_probe` (Ch1 §1.2.2 §2.2 step 2) | PROBE_RESPONSE per tree: k_avail 3 / 3 / 1, hop 0, SERVING, live_edge seg 1 | — |
| 1181 calc | V | `on_probe_response`, `finish_probing` (§2.1 score) | RTT 36 ms; score(tree 1) = 12 sqrt(3) - 36 - 20(e^0 - 1) = **-15.2**; tree 3: 12 - 36 = -24. All answered -> round closes before the 200 ms deadline; `request_next`: session already open -> `send_rank_proof_for` (none: no receipts) + NEIGHBOR(m, LOW, RELAY, assigned {1}); deadline 1781 | 3 x `Requesting` |
| 1199 calc | source | `on_neighbor` (Rank Admission case 1) | depth 0 < 8, k_avail > 0 -> ACCEPTED(hop 0, pending false); `send_recent_manifests`: STREAM_DESCRIPTOR + manifests from live_edge - 1: (1,0..3), (2,0) = 5 manifests | source: children tree 1 = {V} ... |
| 1219.7 obs | V | `on_accepted` (Ch3 §3.3.1 re-attach; Ch4 §4.3.1 step 3) | parent = source, depth 1; srtt 36 ms, **rttvar = srtt/2 = 18 ms -> tau_evict = max(200, 200 + 36 + 72) = 308 ms** (spec table reads 200 + RTT + 4 RTTVAR with RTTVAR ~RTT/4..RTT/25: 240..272); MANIFEST_REQUEST(Descriptor); `backfill_from`: `live_edge` is NONE at this instant (manifests follow ACCEPTED on the same stream) -> **returns without sending** | tree 1 attached |
| 1219.7..1219.8 obs | V | `on_manifest` / `insert_manifest` (Ch7 §7.1.2 window; Ch4 §4.3.1 anchor) | 5 manifests verified; anchor = newest (2,0) at 1219.8 -> deadline(2,0) = 4219.8; live_edge = 2; `note_implicit_provider(source)` for each | buffer holds 5 chunk manifests, 0 blocks |
| 1220.4 / 1220.9 obs | V | `on_accepted` x2, `check_active` (Ch1 §1.3.1 CONNECTING -> ACTIVE on T_sub) | trees 2, 3 attached to the source (depth 1); all subscribed trees parented | Connecting -> **ACTIVE** at 1220.9 (221 ms after start) |
| 1250 calc | source | `publish_chunk` (Ch3 §3.3.2 pacing) | chunk (2,1): 12 blocks; block j leaves at 1250 + 125 x j / 12 (10.4 ms apart) -> <= 125 ms per chunk, spec "<= 1/2 chunk period" | — |
| 1269.1 obs | V | `insert_manifest` re-anchor rule | manifest (2,1) arrives (19 ms one-way); nothing played yet and (2,1) > (2,0) -> anchor = (2,1) at 1269.1; deadline(2,1) = **4269.1**, deadline(1,0) = 4269.1 - 5 x 250 = **3019.1** | deadlines re-laid on the 250 ms grid |
| 1275.2 obs | V | `on_block_proof` (Ch1 §1.2.2 depth propagation), `try_verify` (Ch4 §4.1) | BLOCK_PROOF(tree 1, seg 2, blk 4096, SenderHopDepth 0) + 17 symbols; `first_push_chunk[1] = (2,1)`; depth = 0 + 1 = 1; decoded from 16 of 17; `last_block_at` | tree 1 streaming; blocks 4097 @1286, 4098 @1296, tree 2 blocks @1307..1328 (10 ms apart, pacing preserved at depth 1) |
| 1600 obs | V | `schedule_pulls` (Ch4 §4.3.3 every tau_sched; §4.3.1 W_pull) | W_pull = clamp(308 + 6 x 36 = 524 ms, 1500, 2000) = **1.5 s**; (1,0) enters the zone at 3019 - 1500 = 1519 -> first pass at 1600; 6 directed PULL_REQUESTs (blocks 0..5, urgency 1419 ms) to the implicit provider (source); tree-3 blocks pulled on later passes | 6 pending pulls |
| 1647..2154 obs | source -> V | `on_pull_request`, `serve_pull_queue`, `pull_budget_take` | budget r_pull x 20 Mbps = 250 KB/s (15 blocks/s), 100 ms burst; blocks land at 1647, 1724, 1898, 1925, 2149, 2154 (shared with pulls for (1,1)..) | (1,0) complete at 2154 < 3019 |
| 2000 calc | V | `incentive_timers`, `gossip_bitfields` | TFT cycle #2 (500 ms grid ✓ tau_tft), bitfields for last 3 segments (tau_gossip ✓); first PROOF_OF_UPLOAD for segment 2 is issued when the first block of segment 3 arrives (~3270) | — |
| 3019.1 obs | V | `run_timers` playout (Ch8 §8.2 SJL) | deadline (1,0): 12/12 blocks -> layers_complete 3, ok -> **FirstChunk**; SJL = 3019 - 1000 = **2.019 s** (spec target 1.5 s); flagged `backfill = true` because (1,0) <= first_push_chunk (2,1) | `first_ok_chunk` |
| 3269 / 3519 / 3769 / 4019 obs | V | playout | (1,1): 1 layer complete, ok; (1,2), (1,3), (2,0): **0 layers complete, ok = false** (10 blocks missing each) -> a 0.75 s freeze right after the first frame; all four flagged backfill -> `steady_played` not incremented | 3 starved intervals, none counted in PSR |
| 4269.1 obs | V | playout | (2,1) complete from the push -> ok, first steady chunk; playout offset from emission = 4269.1 - 1250 = **3.019 s** | PSR denominator starts |

### 2b. Parent loss and repair in tree 1 (V now a relay with 2 children; hand walk at RTT 40 ms, cross-checked with churn-storm node 3)

Initial: V's tree-1 parent P at depth 1 (V at 2), ParentLink srtt 40 ms, rttvar 20 ms (set at
attach and never updated since: `on_pong` adjusts only after a V-initiated PING, which only
happens in a silence episode). tau_evict = max(200, 200 + 40 + 80) = **320 ms**.

| t_ms | node | rule (file §) | computation | state after |
|---:|---|---|---|---|
| 9990 | P -> V | last block of a chunk burst | `last_rx = 9990` | — |
| 10000 | P | scenario | dies silently | — |
| 10100 calc | V | `liveness` (Ch3 §3.3.2 line 4) | tick on the 25 ms grid: silence 110 > tau_ping 100 -> PING(P) on UDP once (`pinged`) | probing |
| 10100..10325 calc | V -> children | `liveness` heartbeat (Ch3 §3.3.1 rule 1) | `last_tx` to each tree-1 child >= 100 ms -> PING heartbeat per tick when due; children keep `last_rx` fresh and do not evict V (no cascade; observed `heartbeat` events every 250 ms at node 1 between chunk bursts) | children attached, waiting |
| 10325 calc | V | `liveness` line 7a-11 | silence 335 > 320 -> `lose_parent(1,"timeout")`: `lost_at = 10325`, `last_depth = 2`, depth = 255, `repairing = true`; `begin_probing(1)`: candidates = passive ∪ active relaying tree 1 minus V's tree-2/3 parents (3 records; in a churn storm one is dead); PROBE x3, deadline 10525. V's PROBE_RESPONSE for tree 1 now says UNPARENTED, k_avail 0 (`TreeState::k_avail`) | tree 1 in repair; trees 2, 3 unaffected |
| 10365 calc | V | `on_probe_response` | 2 live candidates answer (RTT 40); cycle guard: `has_children` and own last depth 2 -> skip hop > 2 (equal hop allowed since no parent); third candidate silent -> round waits for the deadline | — |
| 10525 calc | V | `finish_probing` -> `request_next` | best E (hop 1, k_avail > 0); no session -> `open_session` (impl keeps no 0-RTT ticket; sim 1.5 RTT = 60 ms) | `Requesting`, deadline 11125 |
| 10585 / 10605 calc | V -> E | `on_session_opened` -> `send_neighbor` | NEIGHBOR(1, **HIGH** because `repairing`) arrives at E at 10605 | — |
| 10625 calc | V | `on_accepted` | parent E, depth 2, `attached_as_repair`; `repairing` stays true (`lost_at` set); fresh srtt/rttvar from the probe RTT | attached, awaiting data |
| ~10700 calc | E -> V | `on_block_proof` first block (Ch1 §1.2.2 Drain Path step 3 "when the new parent begins delivering") | next chunk boundary <= 250 ms away, blocks paced over 125 ms -> ~60..130 ms wait; `RepairDone duration = 10700 - 10325 = 375 ms`; from the last packet **710 ms** | tree 1 streaming; children get SenderHopDepth 2 |
| 30887 / 31326 / 31404 obs (churn, node 3) | — | same path | kill at 30000; `parent_lost(timeout)` +887 ms (its tau_evict = 200 + 3 x RTT_probe + burst gap); `parent_attached(repair)` +439 ms; `repair_done` +78 ms: **1.404 s from death** | — |
| churn aggregate obs | 48 timeout evictions, 51 repairs | — | detection after the kill: min 203, **median 476**, p95 761, max 887 ms. Eviction -> ACCEPTED: min 221, **median 495**, p95 1271 ms. ACCEPTED -> first block: median 78, p95 823 ms. Repair from death, median ≈ 0.48 + 0.57 = **1.05 s**; spec table at 20/80/250 ms RTT: 280/480/990 ms | — |
| 11500..12700 calc | V | `schedule_pulls` | tree-1 blocks of the 3 chunks emitted during the outage (9 blocks, 1.18 Mbit) are missing; their manifests arrived through the tree-2 parent (`forward_manifest` to every child); deadlines ≈ 13.0..13.75 s; zone opens at deadline - 1.5 s; one 10 Mbps provider serves 7.5 blocks/s -> 9 blocks in 1.2 s -> complete by ~12.7 s < 13.0 s | no starvation from this loss (churn: 75 counted starved intervals of 19,017 across 51 repairs) |

### 2c. Shed (node 4 of `small_swarm`, N = 5; observed)

| t_ms | node | rule (file §) | computation | state after |
|---:|---|---|---|---|
| 6759 obs | n4 | `join_round_failed` tree 2 (Ch1 §1.1.5 §5.3 item 1) | 1 saturated candidate (source tree-2 slots held), no WARMING -> `failed_rounds(2) = 1`; backoff 250 x 2^1 = 500 ms; `starved_requery(2)`; GET_PEERS re-issued | tree 2 `Backoff` until 7259 |
| 6883.6 obs | n4 | `on_accepted` tree 1 | parent = source, depth 1 | — |
| 6884 obs | n4 | `join_round_failed` tree 3 | source K(3) = 1, held by node 1 -> REJECTED_SATURATED -> `failed_rounds(3) = 1`; backoff 500 | tree 3 `Backoff` until 7384 |
| 7384 calc | n4 | `run_timers` Backoff -> `begin_probing(3)` | fresh sample from the re-query (still only the source relays tree 3) -> PROBE | — |
| 7408.9 obs | n4 | `join_round_failed` -> `shed_to(1)` (Ch1 §1.1.5 §5.3 item 3) | saturated 1, `saw_warming = false` -> `failed_rounds(3) = 2` >= `shed_after_failed_rounds` and layer(3) = 2 > 0 -> **shed layer 2**: tree 3 unsubscribed, join Idle, counters reset; `shed_retry_at = 17409`. **The two failed rounds are 525 ms apart; the spec requires a retry interval >= 1.0 s (one segment), so the earliest spec-conformant shed is ≈ 6884 + 1000 + RTT ≈ 7920** | top_layer 1 |
| 7422.5 obs | n4 | `on_accepted` tree 2, `check_active` | all subscribed trees {1, 2} parented | **ACTIVE** |
| 17409 calc | n4 | `unshed_attempt` (§5.3 item 5 hysteresis 10 s) | `top_layer = 2` and tree 3 re-subscribed **before any parent is held** (spec: "restored only when all of its trees have parents again"), `begin_probing(3)` at once (spec: left to the 5 s migration loop); if still saturated: rounds at +40 and +540 ms -> re-shed at ≈ 17950, `shed_retry_at = 27950` | 10.5 s shed cycle |
| churn aggregate obs | 31 sheds | — | gap between the two rounds that produced a shed: min 342, median 700, max 2076 ms (spec floor 1000); 16 sheds in `small_swarm` at N <= 10 | — |

## 3. Simulator KPI definitions versus Ch8 §8.2

| KPI | Ch8 §8.2 | `kpi.rs` | Verdict |
|---|---|---|---|
| PSR | "StarvedPlayoutIntervals / TotalPlayoutIntervals ... Target <= 0.1 % across the entire swarm" | `steady_starved / steady_played` over non-publisher nodes; an interval counts only if (a) a first ok chunk was already played, (b) the chunk is not flagged `backfill`, where `backfill` = accepted behind the live edge **or** `!is_steady_chunk`, which is false until every tree with `wants_parent()` (subscribed **or assigned**) has pushed a later chunk. Starved = base layer incomplete (`layers_complete >= 1` is ok) | Diverges. Churn run: 560 not-ok chunks played, 333 before a first frame, **152 after the first frame but flagged backfill (excluded), 75 counted**. Reported 0.39 %; by the spec's denominator (intervals after the first frame) 227 / 19,939 = **1.14 %**. Base-layer-only "ok" is a reasonable SVC reading (resolution loss is not a freeze) and is recorded. The exclusion is recorded in ARCHITECTURE §6 ("Chunks before the first chunk pushed to a node are backfill: best effort ... excluded from steady-state PSR") but its scope is wider than that sentence: it also excludes the freeze after the first frame (tick 3269..4019 above) and every interval of a relay whose assigned-but-unsubscribed tree has not yet pushed |
| SJL | T_first_frame - T_join_intent, <= 1.5 s | `first_chunk_us - started_us`; FirstChunk = first played chunk with the base layer complete, backfill included | Definition matches. Measured: small 1.40..3.41 s (mean 2.47); churn mean 3.20, median 3.26, p95 4.16 s. Gate `sjl_mean_max_s = 3.6` with the comment "M1: no late-join backfill yet" (stale: the parent-pushed manifests and implicit-provider pulls are the backfill). Spec-level inability to reach 1.5 s is flash-crowd F2 / cold-start-n2 F4 |
| CDO | control / (video + control), video = "source blocks and RaptorQ parities", control = "DHT RPCs, gossip shuffles, pings, bitfields"; <= 2 % | video = RAPTORQ_SYMBOL + BLOCK_TRANSMISSION frame bytes; everything else (BLOCK_PROOF, MANIFEST, receipts, rosters...) is control; frame payload sizes without the 60 B/packet overhead | Conservative classification (BLOCK_PROOF and MANIFEST are neither named video nor control in the spec; counted as control). Measured 2.25 % (small), 2.52 % (churn) > 2 %; gate `cdo_max = 0.05`, a 2.5x relaxation not listed in ARCHITECTURE §6. Spec-level overrun is media-pipeline-latency F5 |
| D_avg | (1/N) sum_i HopCount(i), "hops traversed by a chunk slice before arriving at a leaf" | mean over (node, tree) pairs with `subscribed && parent.is_some() && depth != 255`, last snapshot only | Per-slice mean matches the spec's wording; unparented trees (effectively infinite depth) are excluded, which biases low exactly when trees are starving. Low; noted, not filed |

## 4. Requirement checks

| Req | Verdict | Evidence |
|---|---|---|
| R1 glass-to-glass 3-5 s, ~4.0 s | pass | Steady chunks in the churn run play 3.007..3.157 s after source emission (median 3.064; depth <= 4, RTT <= 160 ms). Adding the encoder (0.1..0.3) and signing barrier (0.25) the simulator does not model: 3.4..3.7 s; at depth 7 (+0.3..0.5 s of hops) ≈ 4.0..4.2 s. The implementation's anchor (newest manifest before the first play + 3.0 s, 250 ms grid) reproduces Ch4 §4.3.1 |
| R2 first frame within 1.5 s | fail | Node 1: start 1000 -> FirstChunk 3019 = 2.019 s. small_swarm: min 1.398, mean 2.468, p95 3.408 s; churn: mean 3.197, p95 4.156 s. The first renderable chunk is the oldest manifest the parent pushed (live_edge - 1), whose deadline is anchor + 3.0 - k x 0.25 s, k <= 7, so SJL >= join (0.22..1.5 s) + 1.25 s even when every pull lands. Known at spec level (flash-crowd F2, cold-start-n2 F4) |
| R5 sub-second repair, one tree, resolution not playback | fail (margin) | Repair is per tree (`TreeState::repairing`; trees 2/3 verified and forwarded at every tick of §2b; children held by heartbeats). Time from last packet: hand walk at 40 ms RTT 710 ms; churn median ≈ detection 476 + eviction->first block 573 = **1.05 s**, p95 ≈ 0.76 + 1.34 = 2.1 s, versus the spec's 280/480/990 ms. Contributors: RTTVAR frozen at RTT/2 (F1), a full probe round with a 200 ms timeout when the pool holds a dead record and no 0-RTT ticket (F3). Capacity shortage sheds a layer, never stops playback (§2c), but the shed fires on rounds 0.34..0.7 s apart (F4) |
| R9 PSR <= 0.1 % | pass with caveat | small_swarm steady PSR 0 / 925. Churn storm (a §8.3 stress case, not the R9 regime) 0.39 % by the implementation's definition, 1.14 % by the spec's (F2). The implementation's number is not comparable to the spec's until the exclusion is narrowed |

## 5. Candidate findings

Each is stated as "with inputs X the implementation produces Y, and Y diverges from the spec because Z",
with the file and line, whether the deviation is recorded, and the debate against it.

### F1 (Medium, impl, new) The parent RTT estimator is seeded once from the PROBE RTT with RTTVAR = RTT/2 and never updated while media flows, so tau_evict = 200 + 3 RTT and detection is 1.5-2x the spec's

Spec: "The deadline is instead taken from the connection's own QUIC RTT estimator:
tau_evict = max(200 ms, 2 tau_ping + SRTT + 4 RTTVAR)" (Ch3 §3.3.1; App B tau_evict row).

Impl: `node.rs` `on_accepted` sets `srtt = probe RTT`, `rttvar = srtt.div_by(2)`; `on_pong` updates
them only when `Session::ping_sent_at` was set by `send_ping`, which a child does only after
`tau_ping` of silence or once at session open (before the ParentLink exists). No other input
touches them; `bs-node` reads quinn's `path.rtt` only into the run report (`driver.rs:1629`) and
feeds nothing to the core; `io.rs` has no RTT input.

Numbers: at RTT 40 ms tau_evict = 320 (spec table interpolates ~250..280); at 160 ms = 680 (spec
with a converged RTTVAR of a few ms ≈ 370). Churn run, kill at 30 s: timeout detection median 476,
p95 761, max 887 ms (the sim's RTTs are 10..160 ms, jitter 2 ms). That detection plus the
eviction->first-block median of 573 ms puts the median repair at 1.05 s from the last packet
against the spec's "sub-second" claim and its 280/480/990 ms table.

Debate: a larger tau_evict is the safe direction (fewer false evictions), and `W_pull` is derived
from the same inflated value so the PULL zone still covers the repair. It still diverges from the
rule as written, it makes the R5 number miss by itself at inter-region RTTs, and it is not in
ARCHITECTURE §6. Fix shape: add an RTT-sample input the drivers feed (quinn path stats; the sim's
pair latency), or sample from heartbeat PING/PONG pairs the parent already sends every tau_ping of
idle; then RFC 6298 smoothing as `on_pong` already does.

### F2 (Medium, impl, recorded but wider than recorded) PSR excludes starved intervals after the first frame and every interval of a relay whose assigned tree has not pushed yet

Spec: "PSR = StarvedPlayoutIntervals / TotalPlayoutIntervals ... Target: PSR <= 0.1 % across the
entire swarm" (Ch8 §8.2). Ch4 §4.3.1 step 5: "PULL any blocks of segment X (and any gaps) from
active-set neighbors to fill the initial buffer" — the backfilled chunks are meant to be served,
not best effort.

Impl: `kpi.rs` `event()` counts `steady_played/steady_starved` only when `!backfill` and a first ok
chunk exists; `node.rs` `run_timers` sets `backfill = p.backfill || !is_steady_chunk(...)`, and
`is_steady_chunk` returns false until every tree with `wants_parent()` (subscribed **or assigned**)
has an entry in `first_push_chunk` later than the chunk.

Numbers: node 1 played (1,0) ok at 3.019 s, then (1,2), (1,3), (2,0) with 0 layers complete — a
0.75 s freeze after the first frame — none counted. Churn run: 152 starved intervals after the
first frame excluded, 75 counted; 0.39 % reported, 1.14 % by the spec's denominator. The
assigned-tree clause has no occurrence in these runs (`steady_played == 0` for no node) but is
reachable whenever a relay's rendezvous tree is saturated (ISSUE-061's regime), where every
interval of that relay is excluded for as long as the tree stays unparented.

Debate: ARCHITECTURE §6 records "chunks before the first chunk pushed to a node are backfill:
best effort, reported separately, excluded from steady-state PSR" with a Ch4 §4.3.1 citation. The
intervals before the first frame are SJL's domain and excluding them is defensible. The two
excess exclusions above are not covered by that sentence and hide a freeze the viewer sees. Fix
shape: count every interval after the first ok chunk; key `is_steady_chunk` on subscribed trees
only (or on the base-layer tree alone, since `ok` is base-layer completeness).

### F3 (Medium, impl, already known: parent-loss-repair F7) Repair has no fast path: a full PROBE round, a 1.5-RTT session open and a NEIGHBOR round trip precede data

Spec: "T = tau_evict + 10 ms: Peer i takes the per-tree pool P_m ... selects the one with the
highest reliability R and advertised K_avail, opens a QUIC session to it and sends an urgent
NEIGHBOR(TreeID = m, Priority = HIGH)"; "~1 RTT where the peer holds a 0-RTT resumption ticket ...
Peers must retain resumption tickets" (Ch3 §3.3.1).

Impl: `lose_parent` -> `begin_probing` (PROBE to every candidate, round closes at the 200 ms
`probe_timeout` unless all answer) -> `finish_probing` -> `request_next` -> `open_session` (no
ticket notion) -> NEIGHBOR. Numbers: eviction -> ACCEPTED median 495 ms, p95 1.27 s in the churn
run (dead records in the passive set make the probe round wait its full 200 ms); spec ≈ 2 RTT =
20..320 ms. Already filed by parent-loss-repair F7 together with the missing ROSTER/Deputy
election; this walk adds the measured numbers.

### F4 (Low, impl, already known: flash-crowd F6, capacity-shortfall F5) Join retry backoff is 250/500/1000 ms; two shed-deciding rounds are 0.34-0.7 s apart

Spec: "the retry backoff interval must be at least one segment period (1.0 s), so that no shed
decision is ever taken on a sample shorter than a single warm-up window" (Ch1 §1.1.5 §5.3 item 2).
Impl: `join_backoff = 250 ms`, `join_round_failed` uses `250 x 2^min(failed, 2)`, 0 during a
repair or an exempt round. Numbers: node 4 shed at 7.409 s on rounds at 6.884 and 7.409 (525 ms
apart); churn run 31 sheds, round gaps min 342, median 700 ms; 16 sheds at N <= 10 in
`small_swarm`. Known; numbers added.

### F5 (Low, impl, new) The NEIGHBOR deadline is a constant 600 ms counted from before the handshake, where the spec's is tau_sched + RTT after it; past 300 ms RTT a legitimate ACCEPTED arrives late and leaves a phantom child

Spec: "silence past tau_sched + RTT means unreachable" (App B `NEIGHBOR` reply row; App D §D.4.3b).
Impl: `request_next` sets `deadline = now + probe_timeout * 3` (600 ms) before `open_session`;
`run_timers` on expiry closes the session and asks the next candidate; `on_accepted` ignores a
reply whose sender is not `current`. Numbers: handshake 1 RTT + NEIGHBOR 1 RTT = 2 RTT, so the
constant is exceeded at RTT > 300 ms (satellite paths the spec names), or > 240 ms with the sim's
1.5-RTT handshake; the spec's own worst path is 250 ms (375 + 250 = 625 > 600 in the sim model). The
parent that accepted keeps a `ChildLink` and pushes to a child that has moved on until the
receipt deadline evicts it (3 segments), one slot wasted for ~3 s per occurrence. Debate: the
sim's RTT range (<= 160 ms) never hits it; it is a divergence of shape (constant vs RTT-scaled),
cheap to fix by arming the deadline at `send_neighbor` with `tau_sched + 2 x srtt`.

### F6 (Low, impl, new) An unparented assigned tree is retried at the join backoff (<= 1 s) instead of the 5 s migration cadence

Spec: "An assigned tree the relay cannot join is retried at the upward-migration cadence (§1.2.2,
every 5 s) without the 10 s hysteresis" (Ch1 §1.1.5 §5.3 item 4; core loop line 48
`RetryJoinAtMigrationCadence`). Impl: after a shed the assigned tree keeps `wants_parent()`,
`join_round_failed` puts it in `Backoff` of at most 1 s, and `run_timers` re-probes every
candidate on expiry. Numbers: 5x the spec's probe traffic toward a saturated tree's relays (one
PROBE/PROBE_RESPONSE pair ≈ 240 B per candidate per round; ~40 candidates -> ~10 KB/s per
unparented relay); at 10^6 with 10 % of a tree's relays unparented each saturated relay answers
~4 extra probes/s. Small, but it is the cadence the spec fixed on purpose, and `starved_requery`
(5 s) is already a third cadence for the same condition (capacity-shortfall F3).

### F7 (Low, impl, new) A shed layer is re-subscribed at the hysteresis expiry before any of its trees has a parent

Spec: "the layer is restored only when all of its trees have parents again" (Ch1 §1.1.5 §5.3
item 5). Impl: `unshed_attempt` sets `top_layer += 1` and `subscribed = true` for the layer's
trees, then probes; on failure the layer is shed again 0.54 s later (two rounds), so `top_layer`
flaps 1 -> 2 -> 1 every 10.5 s while the tree stays saturated (node 4 from 17.4 s on). Behavioural
effect is small: rendering follows `layers_complete`, so no frame changes; `layers_subscribed` in
`ChunkPlayed` and `played_full` are wrong for ~0.5 s per cycle, and the extra probe rounds are the
F6 traffic. Fix shape: keep `top_layer` at the shed value until every tree of the layer reports a
parent, then raise it.

### F8 (Low, impl, new) KPI gates are relaxed against Ch8 §8.2 without a record: cdo_max 0.05 against 2 %, sjl_mean 3.6 s against 1.5 s with a stale rationale

Spec: "CDO <= 2.0 %", "SJL <= 1.5 seconds" (Ch8 §8.2). Impl: `KpiThresholds::default()` has
`cdo_max: 0.05`, `sjl_mean_max_s: 3.6` with the comment "M1: no late-join backfill yet, so the first
frame renders Delta_buffer (3 s) after the first manifest. M3 ... targets 1.5 s" — the backfill
exists (parent-pushed manifests + implicit-provider pulls) and the first frame renders 1.25..3.0 s
after the anchor, not 3.0 s. Measured CDO 2.25 % and 2.52 % pass the gate and fail the spec; the
relaxation is not among the recorded decisions in ARCHITECTURE §6. Debate: gates are a project
choice, not a protocol rule; filed Low because the simulator is the spec's own validation plan
(Ch8) and a gate that cannot fail the spec's target measures nothing about it.

### Dropped after debate

- `backfill_from` is dead code on the first attach (`live_edge` is NONE when ACCEPTED is handled,
  because the manifests follow it on the same ordered stream, and the call is made only while
  exactly one tree is attached). No behavioural consequence: `send_recent_manifests` pushes the
  same `live_edge - 1` manifests, which is what the request would have asked for. Noted, not filed.
- JOINING falls through to CONNECTING after 1 s regardless of theta_join (`membership_timers`).
  The spec has no bound; the FORWARD_JOIN walk eventually fills the set. The impl's cap is an
  improvement over a spec silence, not a defect.
- ACTIVE -> DISCOVERY -> CONNECTING on total parent loss skips JOINING and never TERMINATES when
  no records return (spec: TERMINATED after tau_ttl with no records). The Active Set survives, so
  JOINING has nothing to do; the terminate rule is cosmetic for a viewer with a UI. Not filed.
- Source slots computed with the relay formula and no R_src reserve: cold-start-n2 F6,
  capacity-shortfall F4. Not re-filed.
- Manifest window ahead 3 against the spec's 2: media-pipeline-latency F7. Not re-filed.
- tau_evict's RTTVAR in the spec table is unstated and inconsistent (240/320/490): parent-loss-repair
  F5. Not re-filed; F1 is about the implementation never converging, which is independent.

## 6. Spec silences assumed through, and implementation-only rules met on the way

- S1. The spec does not say when a joiner's playout anchor is taken relative to the manifests a
  parent pushes at admission; the impl anchors at the newest manifest seen before the first play
  (recorded in ARCHITECTURE §6). Assumed equivalent to "3.0 s behind the received live edge".
- S2. The spec's "re-anchor on entry to ACTIVE" and "correct drift continuously" (Ch4 §4.3.1) have
  no impl counterpart; on one virtual clock drift cannot occur, so no tick depended on them.
- S3. The spec gives no bound for JOINING; the impl's 1 s cap was walked as given.
- S4. The spec's initial RTTVAR for a new parent is unstated (RFC 6298 says R/2, which the impl
  uses); the spec's own table implies RTT/4..RTT/25.
- S5. Heartbeats on a bursty tree: the impl sends a PING to a child whenever `last_tx >= tau_ping`,
  which with 3 blocks per 250 ms chunk means one heartbeat per chunk gap (observed every 250 ms at
  node 1). The spec allows a QUIC PING frame instead; the cost is one small frame per child per
  chunk and is inside the CDO figures above.
- S6. The data-stall rule (2 s without a block -> parent lost; 2 s unparented with children -> drain)
  is implementation-only (ISSUE-066 addendum, recorded). It did not fire in either run.
- S7. The sim's handshake costs 1.5 RTT and no node holds a resumption ticket; the spec's 1-RTT
  re-attach needs the ticket rule the impl does not have (F3).
- S8. Which frames are "video" for CDO (BLOCK_PROOF, MANIFEST) is unstated; the impl counts them as
  control, the conservative reading.
