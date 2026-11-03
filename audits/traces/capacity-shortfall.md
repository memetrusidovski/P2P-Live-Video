# Trace: capacity-shortfall (shed and fold)

Walker output for the BitStream protocol audit. Scenario id `capacity-shortfall`.
Every constant comes from `protocol/appendix_b_parameters.md` unless a chapter is cited.

## Scenario and initial state

| Quantity | Value | Source |
|---|---|---|
| Swarm | source S plus N = 500 `RELAY`-class viewers, leaf fraction ℓ = 0 | scenario |
| Mean upload ū_relay | 5 Mbps (walked at the mean; floors noted where a distribution would bite) | scenario |
| Ladder | L0 480p 1.5 Mbps, L1 720p 1.5 Mbps, L2 1080p 3.0 Mbps, B = 6 Mbps | Ch1 §1.2.4 §4.2 |
| Forest | M = 6 (N_relay = 500 ≥ 30), mapping (2,2,2): T1,T2 = L0 at 750 kbps; T3,T4 = L1 at 750; T5,T6 = L2 at 1500 | Ch1 §1.2.1 §1.4, §1.2.4 §4.2.1 |
| Overhead | Ω = 1.15, r_pull = 0.10 | App B |
| Source upload u_S | 20 Mbps (not given by the scenario; assumed, see silences) | Ch6 §6.3.1 relay-capable floor |
| Source reserve R_src | 3 · 1.5 · 1.15 = 5.175 Mbps | Ch1 §1.1.5 §5.4 |
| RTT | 80 ms reference; probe timeout 200 ms | Ch1 §1.2.2, App B |
| Retry backoff | ≥ 1.0 s (one segment) | Ch1 §1.1.5 §5.3 item 2 |
| Shed | 2 failed rounds; 10 s hysteresis; WARMING exempts a round | Ch1 §1.1.5 §5.3 |
| Fold | s_fold = 0.05 over τ_fold = 10 s (L0) or 30 s; k = 20, α = 3; τ_forest = 30 s; window 5 segments; τ_grace = 8 segments | App B, Ch1 §1.1.5 §5.6 |
| Unfold | N_relay ≥ 1.5 × count at fold | Ch1 §1.1.5 §5.6 |
| Walked viewer V | 4 Mbps relay, rendezvous assignment T5, wants L0..L2 | scenario |
| Clock | t = 0 when V enters `CONNECTING`; segment 100 starts at t = 0; one segment per second | |

### Derived numbers at t = 0

Sustainability (Ch1 §1.1.5 §5.1): Σu = 2500 Mbps against N·B·Ω/(1 − r_pull) = 500 · 6 · 1.278 = 3833 Mbps. Short by 35 %.

Per-tree thresholds, U = M · B_m · Ω / (1 − r_pull): 750 kbps stripes need 6 · 0.75 · 1.15 / 0.9 = 5.75 Mbps; 1500 kbps stripes need 11.5 Mbps. Both exceed 5 Mbps, so every tree starves before the fold.

Slot counts K_v = ⌊0.9 · u / (B_m · Ω)⌋ (Ch1 §1.2.1 §1.3):

| Node | B_m = 750 | B_m = 1500 | B_m = 500 (after fold) |
|---|---|---|---|
| 5 Mbps relay | ⌊4.5 / 0.8625⌋ = 5 | ⌊4.5 / 1.725⌋ = 2 | ⌊4.5 / 0.575⌋ = 7 |
| V, 4 Mbps | ⌊3.6 / 0.8625⌋ = 4 | ⌊3.6 / 1.725⌋ = 2 | ⌊3.6 / 0.575⌋ = 6 |
| Source K_S = ⌊(20 − 5.175) / (6 · B_m · 1.15)⌋ | ⌊14.825 / 5.175⌋ = 2 | ⌊14.825 / 10.35⌋ = 1 | ⌊14.825 / 3.45⌋ = 4 |

Relays per tree: 500 / 6 ≈ 83. Supply per tree (source slots plus relay slots) against 500 subscribers:

| Tree | Supply | Starved | Fraction |
|---|---|---|---|
| T1, T2 (L0) | 2 + 83 · 5 = 417 | 83 | 16.6 % |
| T3, T4 (L1) | 2 + 83 · 5 = 417 | 83 | 16.6 % |
| T5, T6 (L2) | 1 + 83 · 2 = 167 | 333 | 66.6 % |

Of the 83 relays assigned to T5, 333/500 of them, about 55, hold no parent in T5 and report `UNPARENTED` (ISSUE-061: a relay has no admission priority in its own tree).

Depth check for T5 at (2,2,2): one source child, then 2 slots per relay: 1 + 2 + 4 + 8 + 16 + 32 + 64 at depths 1..7 = 127 relays placed, the last 20 relays and 44 subscribers at depth 7 under 32 relays (64 slots), 40 subscribers at depth 8. Total 167 at exactly D_max = 8. The tree is slot-limited, not depth-limited.

Source egress before the fold: 4 · 2 · 0.8625 + 2 · 1 · 1.725 = 10.35 Mbps, plus reserve 5.175 = 15.5 Mbps ≤ 20.

## Tick table

| # | t_ms | node | rule (file §) | computation | state after |
|---|---|---|---|---|---|
| 1 | 0 | V | Ch1 §1.3.1 *Subscribed Tree Set*; §1.3.2 line 28 | T_sub = {1..6} (layer prefix L0..L2); T_assigned = {5}; T_join = {1..6}. max_trees = max(1, ⌊3.6 / (1.15 · 6)⌋ = 0) = 1 | V in `CONNECTING`; sends `GET_PEERS(WantedTrees = 0x3F, StarvedTrees = 0x00)` |
| 2 | 0 | S | Ch2 §2.3.2 *Write Path*; Ch1 §1.1.5 §5.4 | `STORE_RECORD` seg 100; K_S = (2,2,2,2,1,1); egress 10.35 + 5.175 = 15.5 Mbps | 20 guardians hold record v4; `STORE_RECORD_ACK` counts: ActiveCount 501, RelayCount 500, StarvedCount all 0 (no retries yet) |
| 3 | 80 | V | Ch2 §2.3.3 `GET_PEERS` | ⌈20 / 6⌉ = 4 records per wanted tree; Stream Record: M = 6, version 4, SwarmSize 501, RelayCount 500, s = max(0, ⌈log2(501 / 10⁴)⌉) = 0 | 24 candidates; live edge seg 100 known |
| 4 | 80 | V | Ch1 §1.2.2 §2.2 step 2 | 24 `PROBE`s in parallel, timeout 200 ms, RTT 80 ms | responses due at 160 ms |
| 5 | 160 | V | Ch1 §1.2.2 `TreeState` table; §1.1.5 §5.3 item 1 | T1..T4: 3 of 4 candidates `SERVING`, K_avail > 0 (83 % of slots filled swarm-wide). Score of a K_avail = 2, h = 3, RTT 80 candidate: 12·√2 − 80 − 20·(e^1.5 − 1) = 17.0 − 80 − 69.6 = −132.6. T5, T6: every candidate `SERVING` K_avail = 0 or `UNPARENTED`; none `WARMING` | T5, T6: failed_rounds = 1 (`FAILURE_RETRY_BACKOFF` without any `NEIGHBOR`, since no candidate passed `available_slots > 0`); backoff ≥ 1.0 s, next round at 1160 ms |
| 6 | 160 | V | Ch1 §1.2.2 §2.2 step 4; *Rank Admission* case 1 | `NEIGHBOR(TreeID = m, NodeClass = RELAY, AssignedTrees = 0x10)` to the best candidate in T1..T4; no `RANK_PROOF`, r_new = 0 | 4 joins in flight |
| 7 | 240 | V | App D §D.4.4 `ACCEPTED` | `ACCEPTED(HopDepth = 4)` in T1..T4 → h_self = 5 | parents in 4 of 6 subscribed trees. Still `CONNECTING`: `ACTIVE` needs a parent in every tree of T_sub, and T5, T6 are in T_sub until shed |
| 8 | 240..1160 | V | Ch1 §1.3.1 `CONNECTING` | blocks arrive on T1..T4; buffer anchors on seg 100; nothing decodes in `CONNECTING` | buffered, not rendering |
| 9 | 1160 | V | Ch1 §1.2.2 §2.2 retry; Ch2 §2.3.2 *Retry means re-query* | `GET_PEERS(WantedTrees = 0x30, StarvedTrees = 0x30)`: the α ≈ 3 guardians reached count V under StarvedCount[5], [6]. Probes at 1240 ms: all saturated again | at 1320 ms failed_rounds = 2 for T5 and T6 |
| 10 | 1320 | V | Ch1 §1.1.5 §5.3 items 3, 4, 5; §1.3.1 | shed L2: L_sub = {L0, L1}; T_sub = {1..4}; T6 leaves T_join; T5 stays (assigned), retried every 5 s with no hysteresis; hysteresis for L2 until 11 320 ms. `ACTIVE` condition now met | V `ACTIVE`, decoding 720p; V `UNPARENTED` in T5, advertises K_avail = 0, TreeState = `UNPARENTED` there |
| 11 | 1000..2000 | swarm | same rules as V, applied to the late 16.6 % / 66.6 % | 83 peers per L0 tree and per L1 tree, 333 per L2 tree hold no parent; each re-queries at its retry cadence with the starved bit; 55 relays of T5 and of T6 are `UNPARENTED` | steady-state starvation from t ≈ 1.3 s |
| 12 | 2000 | guardian g | Ch2 §2.3.3 `STORE_RECORD_ACK`; App B *Starved-count window* | 10 s window of distinct starved NodeIDs. Spec model (each query reaches a uniform α of k): StarvedCount[1] = 83 · 3/20 = 12.5, StarvedCount[5] = 333 · 0.15 = 50. See *Candidate findings* F2, F3 for the other models | g answers the seg 102 store with those counts |
| 13 | 2000 | S | Ch1 §1.1.5 §5.6 *Estimate* | ŝ_1 = (20/3) · 12.5 / 501 = 0.166; ŝ_3 = 0.166; ŝ_5 = (20/3) · 50 / 501 = 0.665. Fold rule: T1, T2 (L0) and T3, T4 (L1) are below l_top = L2 and starving; L0 window 10 s | publisher keeps a 10 s history; mean so far (0, 0.15) too short to judge (silence S2: full window assumed) |
| 14 | 6320 | V | Ch1 §1.1.5 §5.3 item 4; §1.3.2 line 47 | assigned-tree retry for T5: `GET_PEERS(0x10, Starved 0x10)`, probes, all saturated | no shed accounting (assigned tree); next retry 11 320 ms |
| 15 | 10 000 | S | Ch1 §1.1.5 §5.6 *Fold rule*; §1.2.4 §4.2.1 fold mappings; §4.5 step 1 | mean ŝ_1 over ACKs at t = 1..10 s = (0 + 0.15 + 8 · 0.166) / 10 = 0.148 > 0.05 with L0 starving → fold L2. New list L0, L1 at M = 6 → (3,3): T1..T3 = L0 stripes 0..2 at 500 kbps priority 100; T4..T6 = L1 stripes 0..2 at 500 kbps priority 50. EffectiveSegmentSeq = 110 + 5 = 115. No prior matrix change, so no dwell. relays_at_fold = 500 | `MANIFEST_UPDATE` v5 down all trees; Stream Record re-stored with NumTreesNext = 6 and the pending matrix |
| 16 | 10 080 | V | Ch1 §1.2.4 §4.5 steps 2, 4; *Content changes are not confined to new trees*; §1.1.5 §5.3 item 6 | v5 > v4, signature verifies. Rendezvous over M = 6 unchanged: no relay moves, no new tree, no pre-emission. Every tree's entry changed → grace until seg 115 + 8 = 123 (t = 23 s). V will need T6 (L1 stripe 2): joins "from the announcement onward as slots allow": T6 is still a 1500 kbps L2 stripe with K_v = 2, saturated → failed round at 10.3 s, not counted (grace) | V waits; K_v unchanged until the switch |
| 17 | 11 320 | V | §5.3 items 4, 5 | L2 hysteresis expires: moot, L2 is no longer in the mapping. T5 retry: saturated, grace | no change |
| 18 | 15 000 (seg 115) | S, relays | §4.5 step 5; §4.2.1 "folding never raises any remaining tree's bitrate" | source emits (3,3) only. K_S(m) = 4 every tree; egress 6 · 4 · 0.575 = 13.8 + 5.175 = 18.98 Mbps ≤ 20. Relays recompute K_v = 7 (V: 6). Bitrates fell 750 → 500 and 1500 → 500, so no K_v fell and no `DRAIN_NOTICE` is sent | free slots open: T1, T2: 83 · 2 + 2 = 168; T3, T4: 168; T5, T6: 28 parented relays · 5 + 3 = 143 |
| 19 | 15 000..15 300 | swarm | §2.2 join; §5.3 item 6 | fill-in round 1. T1, T2: 83 waiting, 168 free → all 83 join by 15.24 s; their base-layer freeze lasted 15.0 s. T3 (now L0 stripe 2): the 83 peers that lacked it join. T5, T6: 333 waiting, 143 free → 143 join, among them ≈ 24 `UNPARENTED` relays, now `WARMING` | base layer complete for every peer at 15.24 s |
| 20 | 15 240 | V | §2.2 join; §4.2.1 striping | V's T6 `NEIGHBOR` accepted (probability 143/333 = 0.43; assumed). V holds T1, T2, T3 (L0), T4, T6 (L1 stripes 0, 2) and lacks T5 (stripe 1): L1 undecodable | V renders 480p; pulls stripe-1 blocks from the PULL zone (Ch4 §4.3.1) |
| 21 | 16 320 | V | §5.3 item 4 cadence; §2.1 warm-up gating | T5 retry: the 24 relays that warmed at ≈ 16.2 s opened 24 · 7 = 168 slots → V `ACCEPTED` at 16.46 s | V `WARMING` in T5 |
| 22 | 17 500 | V | §2.1 warm-up gating | first verified segment in T5 → `SERVING`, K_avail = 6; L1 complete | V at 720p, forwarding T5 to up to 6 children |
| 23 | 17 500 | swarm | §5.1 per-tree condition | third round completes T5, T6 (ideal 2.5 s after the switch; ISSUE-071 measured 8 to 19 s). (3,3) needs ū ≥ 6 · 0.5 · 1.15 / 0.9 = 3.83 Mbps ≤ 5: supply 4 + 83 · 7 = 585 ≥ 500 per tree | no starvation; everyone at 720p |
| 24 | 23 000 (seg 123) | all | §5.3 item 6 | grace ends | failed rounds count again |
| 25 | 25 000..30 000 | guardians, S | App B *Starved-count window*; §5.6 | starved re-queries stopped at 15.3 s; windows clear by 25.3 s; publisher mean < 0.05 from ≈ 30 s; dwell until 40 s | stable folded state |
| 26 | 120 000 | S | §5.6 *Unfold*; App B *Unfold trigger* | RelayCount 750 ≥ 1.5 · 500 (growth; ℓ still 0, N = 751). Dwell satisfied. Unfold to L0, L1, L2 → (2,2,2): T1, T2 = L0 at 750; T3, T4 = L1 at 750; T5, T6 = L2 at 1500. Effective = 220 + 5 = 225 (t = 125 s). relays_at_fold = 750 | `MANIFEST_UPDATE` v6 |
| 27 | 120 000 | relays | §4.5 last paragraph; §1.2.2 *Drain Path* | K_v will fall at the switch: 750 kbps trees 7 → 5, 1500 kbps trees 7 → 2. Each relay with more children than the new K_v sends `DRAIN_NOTICE(CAPACITY, deadline 225)` to its lowest-ranked surplus children. T1, T2: supply 2 + 125 · 5 = 627 of 750 → 123 children drained per L0 tree (16.4 %). T3, T4: 123 each. T5, T6: 1 + 125 · 2 = 251 of 750 → 499 each | drains in progress |
| 28 | 120 000..125 000 | drained L0 children | Ch3 §3.3 warm repair; §2.2 | every candidate in T1 reports K_avail = 0 (the tree is 123 over capacity); rounds fail; grace from the matrix change, and L0 is never shed anyway | no new parent found |
| 29 | 125 000 (seg 225) | relays | *Drain Path* step 3 | `DISCONNECT(CAPACITY)` to the 123 un-reattached children per L0 tree | base-layer freeze for 16.4 % of the swarm begins; L2 trees: 499 per tree lack a parent and shed L2 after grace (t = 133 s) plus 2 rounds ≈ 135 s |
| 30 | 126 000..135 000 | guardians, S | §5.6 *Fold rule*; *Unfold* last sentences | ŝ_1 = (20/3) · (123 · 0.15) / 751 = 0.164 > 0.05; 10 s mean crosses at ≈ 135 s (spec, measured from the switch) or ≈ 143 s (implementation, from Effective + 8). Dwell: "applies between any two matrix changes, fold, unfold or resize alike, and a fold never pre-empts it" → earliest fold at 120 + 30 = 150 s | publisher blocked by the dwell while L0 is frozen |
| 31 | 150 000 | S | §5.6 | re-fold to (3,3), v7, Effective = 250 + 5 = 255 (t = 155 s); relays_at_fold = 750 | `MANIFEST_UPDATE` v7 |
| 32 | 155 300 | swarm | §4.5 step 5; fill-in as tick 19 | K_v back to 7; the 123 drained peers per L0 tree re-join within one round | freeze over: 125.0 → 155.3 s = 30.3 s for 16.4 % of peers |
| 33 | ≥ 155 300 | S | §5.6 *Unfold* | next unfold at N_relay ≥ 1125, then 1688, ... From 500 to 10⁶ relays: ln(2000) / ln(1.5) = 18.7 → 18 unfold-refold cycles, each a ≈ 30 s base-layer freeze for ≈ 16 % of the swarm | cycle repeats on every 1.5× growth step |

## Requirement checks

| Req | Verdict | Evidence |
|---|---|---|
| R5 (resolution lost, never playback) | fail | Base layer frozen for 83 of 500 peers (16.6 %) from t = 0.2 s to 15.24 s before the first fold (ticks 11, 19; accepted transient in SOLUTION-058). Frozen again for 123 of 750 peers (16.4 %) for 30.3 s at every unfold (ticks 26 to 32, finding F1). The shed itself is clean: V lost only L2 at 1.32 s and kept T1..T4 flowing (tick 10). Parent-loss repair not exercised. |
| R6 (contribution buys placement; non-uploaders get the base layer) | pass | V (rank 0) loses L2 slots to ranked joiners under Rank Admission case 2 and keeps L0, L1. Leaf share R_leaf = ⌈0.2 · 5⌉ = 1 per L0 relay = 83 displacement slots per L0 tree, so a 0-upload device gets L0 by displacing a pure subscriber (Ch1 §1.2.2 case 3). Leaf bound (§1.2.5): before the fold ℓ ≤ 1 − 5.175 / 4.5 < 0 (no leaf absorbable without displacing a relay); after the fold ℓ ≤ 1 − 3.45 / 4.5 = 23 %. |
| R8 (broadcaster cost bounded) | pass | Source egress 15.5 Mbps before the fold, 18.98 Mbps after, both ≤ u_S = 20 Mbps and independent of N (ticks 2, 18). R_src = 5.175 Mbps constant. The reserve covers ⌊5.175 / 0.8625⌋ = 6 stripe-slots, 3 per L0 tree, against 83 starved per tree: bounded cost, marginal coverage, as §5.4 states. |
| R9 (PSR ≤ 0.1 %) | fail | First fold: 83 peers starved 15 s in a 600 s stream = 83 · 15 / (500 · 600) = 0.42 %. Each unfold cycle: 0.164 · 30 s per 1.5× growth step; at one step per 300 s that is 1.6 %. Caveat: the starved peers before the first fold never reached `ACTIVE`, so under a literal reading of Ch8 §8.2 (playout intervals of a playing client) they are an SJL failure, not PSR. The unfold freeze hits peers that were playing, so it counts under any reading. |
| R1, R2, R3, R4, R7, R10, R11 | not_applicable | Not exercised by this scenario. Observation for R2: under shortage a joiner cannot enter `ACTIVE` until it sheds the unjoinable top layer, 1.32 s after its first probe (tick 10), because `ACTIVE` is gated on every subscribed tree. |

Scenario-specific checks: the base layer **does** freeze (above); a relay **does** keep its assigned tree (V stayed in T_join for T5 while `UNPARENTED` from 1.32 s to 16.46 s, retried every 5 s with no hysteresis, and joined at the fill-in: ticks 10, 14, 17, 21).

## Candidate findings

### F1. Growth-gated unfold re-creates the base-layer starvation that caused the fold (High, new)

Spec: "The source therefore restores a folded layer **only when the relay population has grown by ≥ 50 % since the fold** (N_relay from the guardians' counts), which adds capacity in the only way the publisher can see" (Ch1 §1.1.5 §5.6 *Unfold*; App B *Unfold trigger*).

With inputs ū_relay = 5 Mbps, ℓ = 0, M = 6, N_relay growing 500 → 750, the spec produces an unfold to (2,2,2) at tick 26. That is wrong because the per-tree condition the fold was judged on, ū_relay ≥ M · B_m · Ω / ((1 − ℓ)(1 − r_pull)) (§5.1), has no N_relay term: supply (N_relay / M) · K_v and demand N both grew 1.5×, and 5 Mbps is still below the 5.75 Mbps the L0 stripes need. Numbers: 2 + 125 · 5 = 627 L0 slots for 750 subscribers, 123 peers drained with `DRAIN_NOTICE(CAPACITY)` and cut at the switch; the re-fold waits τ_fold and the 30 s dwell ("a fold never pre-empts it"), so the freeze lasts 30.3 s. The cycle repeats at every 1.5× step, 18 times between 500 and 10⁶ relays. The unfold is also the one matrix change that raises an L0 tree's bitrate, so its drains release base-layer children into a tree with no free slot, which the drain path cannot absorb.

Charitable reading checked: relay growth helps only when it lowers ℓ (relays replacing leaves). The publisher has ActiveCount and RelayCount, so ℓ is visible to it; the rule reads RelayCount alone. SOLUTION-058's residual covers the opposite case (upload improves, count does not) and does not record this one.

Proposed fix: gate the unfold on growth of the relay *fraction* RelayCount / ActiveCount by enough to move the per-tree threshold for the folded layer (the only term of §5.1 the publisher can see), or on both counts growing with the fraction rising; never on RelayCount alone. State that an unfold's K_v fall drains children, and let a fold that follows an unfold pre-empt the dwell when the starved layer is L0. Touches Ch1 §1.1.5 §5.6, App B, `src/crates/bs-core/src/node/resize.rs` (`forest_consider`, which implements the same rule).

### F2. The median over guardians of StarvedCount is zero when queries land on the closest α guardians (Medium, new, impl)

Spec: "A `GET_PEERS` reaches ≈ α of the k = 20 guardians, so each guardian sees a fraction ≈ α / k of the peers that re-queried with a starved bit in its 10 s window ... ŝ_m = (k / α) · median_g StarvedCount[m] / N̂" (Ch1 §1.1.5 §5.6 *Estimate*).

With inputs 83 starved peers in T1 and S/Kademlia lookups that terminate at the closest nodes to K_s over α disjoint paths (Ch2 §2.1.2), the spec produces StarvedCount[1] ≈ 83 at the 3 closest guardians and 0 at the other 17, so median_g = 0 and ŝ_1 = 0: the fold never fires and the base layer stays frozen for 16.6 % of peers. The second clause of the quoted sentence follows from the first only if every query's α guardians are a uniform random subset of the 20, which nothing in Ch2 §2.1 provides. The implementation hit this and diverged on purpose: `node/resize.rs` lines 95 to 107 compute Σ_g StarvedCount[m] / (α · N̂) with the comment "the spec's median assumes queries spread over the k guardians. Ours go to the α record holders, so most guardians report zero."

Proposed fix: replace the median with the sum over responding guardians divided by α (each querier is counted α times whichever guardians it reaches), with a cap at 1.0 and a trimmed sum if one hostile guardian must be tolerated; or state in Ch2 §2.3.2 that the final `GET_PEERS` goes to α guardians chosen uniformly from the k found. Touches Ch1 §1.1.5 §5.6, Ch2 §2.3.2 write-path table, App B.

### F3. The starved re-query cadence is defined three ways and the estimator and guardian load both depend on it (Medium, new)

Spec texts: "the retry backoff interval **must** be at least one segment period (1.0 s)" (Ch1 §1.1.5 §5.3 item 2); "**Retry means re-query.** `FAILURE_RETRY_BACKOFF` re-issues `GET_PEERS`" (Ch2 §2.3.2); "afterwards **only** when a per-tree candidate pool is thin ... it re-queries that tree at most every 30 s. There is no unconditional periodic re-query" (Ch2 §2.3.2 *Query Discipline*). The estimator "assumes ... that starved peers re-query at the retry cadence" and accepts a 2× error (SOLUTION-040 residual risk).

With inputs 83 starved peers in T1 (true fraction 0.166) the spec produces, depending on the reading: re-query every 1 s onto a fresh uniform α-subset → each guardian sees 83 · (1 − 0.85¹⁰) = 66.7 → ŝ_1 = 0.89; once per 10 s window → 0.166; once per 30 s → 83 · 0.15 · (10 / 30) = 4.15 → ŝ_1 = 0.055. The spread is 16×, not 2×, and the 30 s reading misses the fold entirely at a true starvation of 15 % (ū_relay ≈ 5.2 Mbps). Scale sweep at N = 10⁶ with the same 16.6 % starved per L0 tree and the 1 s floor: 166 000 queries/s, each reaching 3 guardians, is 24 900 pps per guardian against the XDP ceiling GLOBAL_NEW_PPS = 5000 (App B §B.1.4) and the "few hundred queries per second each" design figure of Ch2 §2.3.2; the shield drops the queries and the counts collapse at the scale the fold exists for. The implementation uses a fourth cadence, 5 s per tree (`node.rs` `starved_requery`).

Proposed fix: one sentence defining the starved re-query as once per guardian window (10 s) per starved tree, independent of the probe retry; the retry re-probes the per-tree pool between re-queries. Touches Ch1 §1.1.5 §5.3 and §5.6, Ch2 §2.3.2 *Query Discipline*, App B *Starved-count window*.

### F4. The source reserve has no trigger threshold, no release rule and no steering, and the reference implementation has no reserve (Low, new, impl)

Spec: "The pool serves base-layer slots directly when it observes starvation signals ... from which the publisher forms the starved-fraction estimate ŝ_m of §5.6. The reserve therefore has a concrete, counted trigger rather than an inferred one, and it is the *last* response: §5.6 re-maps the forest first" (Ch1 §1.1.5 §5.4).

With inputs ŝ_1 = 0.166 at tick 13 the spec does not say whether the source opens its 6 reserve stripe-slots (3 per L0 tree) now, after the fold at tick 15, or only when no fold remains (a single-layer mapping); nor when it closes them; nor how a starved peer reaches them (the source is in a tree's `GET_PEERS` sample with probability ≈ 20 / 84 and advertises K_avail per tree). Two implementers diverge by 15 s on when 6 peers are served. Consequence is small at the reference ladder, which is why this is Low. The implementation's source computes slots with the same `Params::slots` as a relay and withholds nothing (`node.rs` `recompute_slots`; no `R_src` anywhere in `src/`), so K_S(750) = ⌊0.85 · 20000 / (6 · 862.5)⌋ = 3 instead of 2 and the "mechanism of last resort" that Ch1 §1.3.1 relies on does not exist there.

Proposed fix: state the trigger (ŝ_m > s_fold for an L0 tree after the last possible fold, or after τ_fold with no fold available), the release (ŝ_m < s_fold / 2 for τ_fold), and that the reserve is exposed as K_avail in the L0 trees. SOLUTION-015 closed the wire trigger, not the threshold.

### F5. Implementation retry backoff is 250 ms, below the 1.0 s floor the spec requires (Low, impl)

Spec: "the retry backoff interval **must** be at least one segment period (1.0 s), so that no shed decision is ever taken on a sample shorter than a single warm-up window" (Ch1 §1.1.5 §5.3 item 2). `Params::join_backoff = 250 ms`, doubled per failed round and capped at 4× (`node.rs` `join_round_failed`), gives rounds at 250, 500, 1000 ms: V would shed L2 at ≈ 0.6 s instead of 1.32 s. ISSUE-063 mentions the 250 to 500 ms cadence while bounding the warming exemption but does not file the divergence.

### Already-known findings met on this walk

| Tick | Open ticket | What the walk showed |
|---|---|---|
| 11, 19 | ISSUE-061 | 55 of the 83 relays of T5 are `UNPARENTED` before the fold; they are not warm at the fold, so "the join is one round" (§5.6) does not hold for them and the fill-in is 3 rounds at best. |
| 5, 9 | ISSUE-063 | A saturated tree with churn would show a `WARMING` candidate in most rounds; the walk assumed none, so that the shed fires. |
| 15, 30 | ISSUE-071 | Dwell from stream start and measuring from Effective + D_max move the first fold to ≥ 30 s and the re-fold to 143 s. |
| 5 | ISSUE-072 | With the 5 % headroom K_v(750) = 4, supply 334 of 500 → 33 % starved per L0 tree instead of 16.6 %. |

## Spec silences assumed through

| # | Silence | Assumption taken | Divergence risk |
|---|---|---|---|
| S1 | u_S is not given | 20 Mbps (Ch6 §6.3.1 relay-capable floor) | changes K_S by ±1 per tree; no rule changes |
| S2 | "the mean of ŝ_m over the last τ_fold of acknowledgements" when fewer than τ_fold ACKs carry starvation | full 10 s window required; first fold at t = 10 s | an implementer averaging what exists folds at t ≈ 2 s; 8 s difference, same outcome |
| S3 | when a peer first sets a `StarvedTrees` bit | after its first failed round, on the next re-query | after the shed instead delays the count by one round |
| S4 | whether the 10 s hysteresis on a shed layer survives that layer leaving the mapping | timer is per layer; the fold ends it, trees 5 and 6 joined from the announcement | a timer kept per tree set delays those joins to 11.3 s |
| S5 | which α of the k guardians a `GET_PEERS` reaches | the spec's uniform model for the main walk; alternatives in F2 | F2 |
| S6 | whether an unfold's K_v fall drains children | yes, by §4.5 last paragraph (`DRAIN_NOTICE`, `CAPACITY`) | §5.6 says nothing about the unfold's drains |
| S7 | whether the source opens reserve slots before or after the fold | not opened (the fold cleared the starvation) | F4 |
| S8 | whether a fold that follows an unfold is a "matrix change" bound by the dwell | yes, per "fold, unfold or resize alike" | F1 cost is 30 s instead of 15 s |
| S9 | mean upload is a mean | walked every relay at 5 Mbps; a distribution with floor effects (a 2 Mbps relay holds 2 slots, an 8 Mbps relay 8) lowers mean K_v below ⌊mean⌋ | starved fraction rises; no rule changes |
