# Trace: flash-crowd

Flash crowd at N = 10,000: 2,000 joiners arrive within 10 s. One joiner is walked tick by tick; the aggregate load on the 20 guardians and the depth 1 to 3 parents is computed alongside.

## Scenario and initial state

| Item | Value | Source |
|---|---|---|
| Swarm | 9,999 viewers + source, 7,000 `RELAY` at 10 Mbps mean, 3,000 `LEAF` | scenario |
| Forest | M = 6, reference mapping: trees 1 to 4 at 0.75 Mbps (L0 on 1 and 2, L1 on 3 and 4), trees 5 and 6 at 1.5 Mbps (L2) | Ch1 §1.2.4 §4.2.1 |
| Ω, r_pull | 1.15, 0.10 | App B |
| K_v per relay | L0/L1 stripe: ⌊9 / (0.75 × 1.15)⌋ = ⌊10.43⌋ = 10; L2 stripe: ⌊9 / 1.725⌋ = 5 | Ch1 §1.2.1 §1.3 |
| Relays per tree | 7,000 / 6 ≈ 1,167 | Ch1 §1.2.5 §5.3 |
| Source | u_S = 100 Mbps (assumed); R_src = 3 × 1.5 × 1.15 = 5.175 Mbps; K_S = ⌊94.8 / 5.175⌋ = 18 per L0/L1 tree, ⌊94.8 / 10.35⌋ = 9 per L2 tree | Ch1 §1.1.5 §5.4 |
| Slot supply, L0/L1 tree | 1,167 × 10 + 18 = 11,688 against 9,999 children: 1,689 free (14 %) | arithmetic |
| Slot supply, L2 tree | 1,167 × 5 + 9 = 5,844 against 9,999 wanting it: 4,155 peers (42 %) hold no L2 parent and have shed L2. Matches Ch1 §1.1.5 §5.2: 1080p at M = 6 needs ū_relay ≥ 11.5 / (1 − 0.3) = 16.4 Mbps | Ch1 §1.1.5 §5.2 |
| Tree shape (assumed converged, relays shallow) | depth 1: 18 relays; depth 2: 180 relays; depth 3: 969 relays + 831 subscribers; depth 4: 8,001 subscribers in 9,690 slots. Mean depth 3.78, max 4. Free slots all at depth 4. | arithmetic |
| Registration | s = ⌈log2(10,000 / 10,000)⌉ = 0: every peer registered | Ch2 §2.3.2 |
| Guardians | 20 relay-class nodes at 10 Mbps; each relays its own tree with 10 slots full | Ch2 §2.3.2 |
| RTT | 80 ms reference; 250 ms worst where it matters | Ch1 §1.1.3 |
| Arrivals | 200 joiners/s for 10 s, 70 % relay / 30 % leaf, each solving PoW on arrival | scenario |
| Walked joiner J | `RELAY`, 10 Mbps, `PUBLIC`, rendezvous assignment a(J) = tree 3 (L1), rank 0, presses play at T = 5,000 ms | scenario |
| Walked leaf L | `LEAF`, arrives at T = 5,900 ms | scenario |

T is the global clock from the first arrival. t is J's clock from pressing play (t = T − 5,000).

The relay-shallow tree shape is an assumption. In L0 trees nothing moves a subscriber out of a relay's way (ISSUE-061), so a tree that grew in arrival order is deeper. The depth numbers below hold for the converged shape only.

## Tick table

| t_ms | node | rule (file §) | computation | state after |
|---|---|---|---|---|
| T=0 | swarm | Ch1 §1.2.1 §1.3 slot count; §1.2.4 §4.2.1 mapping | K_v = 10 / 10 / 10 / 10 / 5 / 5; supply 11,688 per L0/L1 tree, 5,844 per L2 tree; demand 9,999 | 1,689 free in each of trees 1 to 4; trees 5 and 6 full, 4,155 peers shed L2 |
| T=0 | first joiners | Ch2 §2.2.1 PoW; §2.1.3 bootstrap | static 2^16 ≈ 10 ms; seed PING 80 ms; dynamic tier 12 (4,096 iterations); self-lookup 3 hops × 80 = 240 ms | first `REGISTER_PEER` lands at T ≈ 600 |
| T=1,000 | publisher | Ch2 §2.3.2 *Sampled Registration*: s = max(0, ⌈log2(N_est / N_0)⌉), N_est = median(ActiveCount) × 2^s | ActiveCount ≈ 10,080; N_est = 10,080 × 2^0; log2(1.008) = 0.011; s = 1 | Stream Record s = 1; half of new joiners stop registering |
| T=2,000 | publisher | same | ActiveCount ≈ 10,280 (the 10,000 old registrants never learn s; no periodic `GET_PEERS`, Ch2 §2.3.2 *Query Discipline*); N_est = 10,280 × 2^1 = 20,560; s = ⌈1.04⌉ = 2 | s = 2 |
| T=3,000 to 5,000 | publisher | same, once per segment | N_est = 10,330 × 4 = 41,320 → s = 3; 10,355 × 8 = 82,840 → s = 4; 10,370 × 16 = 165,900 → s = 5. `SwarmSize` 165,900 ≥ 10^5 → PoW tier 14 (Ch2 §2.2.1) from T = 4,000 | s = 5 at T = 5,000; joiners now solve 16,384 iterations |
| t=0 | J | Ch1 §1.3.1 BOOTSTRAP; Ch2 §2.1.3 step 2 | static PoW ≈ 10 ms; PING two seeds, PONG at t = 90; both reflections equal local address | `PUBLIC`; dynamic PoW solved at tier 12 (4 ms desktop) before any record is read (silence S1) |
| t=95 | J | Ch2 §2.1.3 steps 3 to 4, DISCOVERY | `FIND_NODE` self-lookup, α = 3, 3 hops × 80 ms | t = 335: k-buckets populated |
| t=335 | J | Ch2 §2.3.2 `GET_PEERS`(K_s, WantedTrees = 0x3F, StarvedTrees = 0), JOINING | `FIND_VALUE` toward K_s, 3 hops × 80 ms; 3 guardians answer | t = 575: 3 × 20 records, ⌈20 / 6⌉ = 4 per tree capped to 20 (4,4,3,3,3,3; silence S2); union ≈ 10 distinct relays per tree; Stream Record: s = 5, `SwarmSize` 331,000, live edge X (0 to 1 s stale), matrix |
| t=575 | J | Ch2 §2.3.2 sample test Blake3(NodeID)[0:2] mod 2^5 = 0 | probability 1/32; J is a 10 Mbps relay, not `RELAY_CAPABLE` (20 Mbps), no s − 3 exemption | J does not register; its future 10 slots in tree 3 are visible only through gossip |
| t=575 | J, contact c | Ch3 §3.1.2 `JOIN` + `FORWARD_JOIN`; Ch1 §1.3.1 θ_join = max(1, min(4, N − 1)) = 4 | QUIC handshake + JOIN 80 ms: c admits J at t = 655 (|A| = 1); FORWARD_JOIN TTL 4, 40 ms per hop, final nodes connect at t ≈ 815, handshake 80 ms | t ≈ 895: |A| ≥ 4 → CONNECTING |
| t=900 | J | Ch1 §1.2.2 §2.2 step 2, all six trees in parallel | `PROBE` to ≈ 10 candidates per tree, 152 B validation block each, timeout 200 ms | responses at t = 980; non-responders time out at t = 1,100 |
| t=980 | J, tree 1 (L0) | Ch1 §1.2.2 §2.1 score = 12·min(√K, 16)·R − RTT − 20(e^{0.5h} − 1); §2.2 step 2 filter `available_slots > 0` | free at T = 5.9 s: 1,689 − 1,180 = 509 over 969 depth-3 relays, mean 0.53, P(≥ 1 free) = 41 %. Sample: 2 depth-2 SERVING K = 0 (saturated, counted), 1 WARMING new relay (exempt flag), 6 depth-3 SERVING K ∈ {0,0,0,0,1,2}, 1 depth-4 new relay K = 10. Scores: depth 3 K = 2: 17.0 − 20 − 69.6 = −72.6; depth 3 K = 1: 12 − 20 − 69.6 = −77.6; depth 4 K = 10: 37.9 − 20 − 127.8 = −109.8 | best = depth-3 relay with K = 2; P(all 10 unusable) ≈ 0.59^7 × 0.15^2 ≈ 0.05 % |
| t=980 | J → P1, tree 1 | Ch1 §1.2.2 *Rank Admission* case 1; App D §D.4.3/§D.4.4 | QUIC handshake 80 ms + `NEIGHBOR`(TreeID 1, RELAY, AssignedTrees bit 3)/`ACCEPTED`(HopDepth 3) 80 ms. Race: P1 receives ≈ 800 NEIGHBOR/s ÷ 3,876 depth-3 relays ≈ 0.2/s; P(slot taken in 160 ms) ≈ 3 % | t = 1,140: J at depth 4 in tree 1 |
| t=980 to 1,140 | J, trees 2, 3, 4 | same | same outcome; tree 3 is J's assigned tree: J reports `TreeState = WARMING` there until its first verified segment (Ch1 §1.2.2 *Warm-Up Gating*) | t = 1,140: parents in trees 1 to 4 at depth 4 |
| t=980 | J, trees 5, 6 (L2) | Ch1 §1.2.2 §2.2 step 2; *Rank Admission* cases 2 and 5; Ch1 §1.1.5 §5.3 item 1 | all 10 candidates SERVING K = 0 (5,844 slots, 9,999 + 1,180 wanting); ≈ 1 UNPARENTED new tree-5 relay (counts as failed; it cannot enter either, ISSUE-061). Case 2 needs r_new > 1.25 × 0: 0 > 0 is false. No `NEIGHBOR` is sent at all: the step 2 filter drops every K = 0 candidate | round 1 failed; `FAILURE_RETRY_BACKOFF`; backoff ≥ 1.0 s (§5.3 item 2) |
| t=1,140 | J → P1 | Ch4 §4.3.1 step 3 `MANIFEST_REQUEST`(segment X chunks + descriptor 0xFD) | 1 RTT | t = 1,220: manifests and descriptor held; pushed blocks of X+1 arriving since t = 1,140 |
| t=1,220 | J | Ch1 §1.3.1 CONNECTING → ACTIVE "parents connected for all subscribed trees"; T_sub = trees of {L0, L1, L2} = all six until L2 is shed | parents in 4 of 6 subscribed trees | J stays in CONNECTING although L0 and L1 data is arriving (finding F3) |
| t=2,100 | J, trees 5, 6 | Ch2 §2.3.2 *Retry means re-query*; App D §D.4.6b StarvedTrees = 0x30 | `GET_PEERS` direct to the cached guardians, 80 ms; probes at 2,180; answers at 2,260: fresh sample, all K = 0 again | round 2 failed at t ≈ 2,300 |
| t=2,300 | J | Ch1 §1.1.5 §5.3 item 3 shed rule (2 consecutive failed rounds); item 5 hysteresis | shed L2: T_sub = {1,2,3,4}; decoder at L1 (720p); retry not before t = 12,300, then at the 5 s migration cadence (t ∈ [12.3, 17.3] s) | ACTIVE at t = 2,300 (all subscribed trees have parents) |
| t=2,300 | J | Ch4 §4.3.1 re-anchor: X_edge = max(record X, HighestPushedSeq); deadline 3.0 s behind | playout position = X_edge − 3; segment X−2 due at t = 3,300, X−1 at 4,300, X at 5,300, X+1 (pushed) at 6,300 | backfill of X−2 .. X needed by PULL |
| t=2,300 | J ↔ 8 active-set members | Ch5 §5.1.3: PullSlots = ⌊1.0 / 0.5⌋ = 2 per 10 Mbps member; optimistic_slots = min(⌈J_new/3⌉, 4, ⌊2/2⌋) = 1; windows every 2 s, unsynchronised; β_pull = 0.5 Mbps per slot | J_new local ≈ 200 × 8 / 10,000 × 2 s ≈ 0.3 per member, queue head. Expected open slots within 1 s = 8 × 0.5 = 4 → 2 Mbps. L0 of one segment = 1.5 Mbit → 0.75 s once 4 slots are open; full 6 Mbit segment would need 12 slots | X−2 (due 3,300) cannot complete: slots open on average at 2,300 + 1,000 |
| t≈3,300 to 4,300 | J | Ch4 §4.3.1 playout deadline; Ch8 §8.2 SJL | X−1 (L0 only) pulled by ≈ 3,300 to 4,050; rendered at its deadline 4,300 (strict) or on completion ≈ 3,300 (lenient, silence S3) | first frame at t = 3.3 to 4.3 s. Lenient best case (anchor at 1,220, 3 slots open at once): 2.2 s. SJL target 1.5 s missed (F2) |
| t=2,140 to 3,140 | J, tree 3 | Ch1 §1.2.2 *Warm-Up Gating*: first verified segment in the assigned tree, segment boundary uniform in [0, 1) s | K_avail(3) = 10, SERVING; `REGISTER_PEER` would be immediate on the `AssignedTrees` change (Ch2 §2.3.2 step 4) but J is out of the 2^−5 sample | J's 10 L1 slots are reachable only through `SHUFFLE`/`GOSSIP_EXCHANGE` WantedTrees |
| T=5,900 | leaf L, tree 1 | Ch1 §1.2.2 §2.2 step 2; *Rank Admission* case 3; leaf share R_leaf = ⌈0.2 × 10⌉ = 2 | best candidate P2, depth 3, K = 1 at probe time; another joiner took the slot 60 ms earlier (3 % race). `NEIGHBOR`(LEAF) arrives at a full P2: leaf children 1 < 2 → ACCEPTED; P2 drains its lowest-ranked relay-class child D that does not relay tree 1 (tie: most recent admission), `DRAIN_NOTICE`(DISPLACED, deadline seg + 5) | L seated at depth 4. Without the race, L's algorithm never sends a NEIGHBOR to a full parent, so case 3 is unreachable by design (F4) |
| T=5,900 | P2 | Ch1 §1.2.2 *Handover budget*: overlap iff r_pull u_v ≥ B_m Ω | 1.0 ≥ 0.8625 Mbps → overlapped; PullSlots = ⌊(1.0 − 0.8625) / 0.5⌋ = 0 for up to 5 segments | P2 serves 11 children; all PULL service from P2 choked for ≤ 5 s |
| T=5,900 + 1 RTT | D | Ch1 §1.2.2 *Drain Path* step 2, warm repair | D re-selects with P2 still delivering: 509 free depth-4 slots, round 1 success | D at depth 4 under another depth-3 relay; `DISCONNECT_CHOKE` to P2; no gap |
| T=6,000 | publisher | Ch1 §1.1.5 §5.6 ŝ_m = (k/α) × StarvedCount[m] / N̂; Ch2 §2.3.3 window 10 s | StarvedCount[5] per guardian ≈ 1,200 joiners × 1 starved re-query × 3/20 = 180; true N̂ = 10,400 → ŝ_5 = 6.67 × 180 / 10,400 = 0.115; published N̂ = 10,400 × 2^6 = 665,600 → ŝ_5 = 0.0018 | top layer never folds either way; the estimator is blinded 64× for any lower layer while s is wrong (F1) |
| T=10,000 | crowd end | depth from tick 13 scores | tree 1: 1,689 joiners seated at depth 4, 311 at depth 5 under the 233 new tree-1 relays; mean depth (37,782 + 1,689 × 4 + 311 × 5) / 11,999 = 3.84, max 5. L2: 2,000 joiners × 2 failed rounds, 0 admitted | depth distribution: 18 / 180 / 1,800 / 9,690 / 311 |
| T=10,000 | each guardian | Ch7 §7.2.1 budgets; Ch2 §2.3.2 load | over 10 s: `GET_PEERS` (2,000 initial + 2,000 starved re-queries) × 3/20 = 600 → 60/s; `REGISTER_PEER` ≈ 400 in-sample joiners → 20/s plus 10,000/90 = 111/s refresh; CONE keepalives 7,000 × 0.6 / 25 = 168/s; total ≈ 360 pps ≪ GLOBAL_NEW_PPS 5,000; ≤ 5 frames per joiner ≪ RATE_PPS_UNKNOWN 50; Ed25519 ≈ 320/s | pps and CPU fine. Egress: 60 × 1,150 B = 69 KB/s = 0.55 Mbps, not in either budget; with 10 slots × 0.8625 = 8.6 Mbps + 1.0 Mbps PULL the uplink is at 10.2 Mbps (F5) |
| T=10,000 | depth 1 to 3 parents | Ch1 §1.2.2 §2.2 probes | 200 joiners/s × 6 trees × 10 probes = 12,000 probes/s over 7,000 relays = 1.7/s each, one Ed25519 verify per probe; `NEIGHBOR`: 800/s over 3,876 depth-3 relays = 0.2/s each; depth 1 and 2 relays receive no NEIGHBOR (full) | negligible per-node control cost |
| T=12,300 to 17,300 | J, trees 5, 6 | Ch1 §1.1.5 §5.3 item 5; §1.2.2 §2.2 | hysteresis over; migration loop runs `execute_tree_join`: every candidate K = 0 → no NEIGHBOR → failed → shed again. At T ≈ 65 s J serves 10 tree-3 children: Θ^rate = 7.5 Mbps, rank 1.25 > 1.25 × 0, enough to preempt any rank-0 child by case 2, but the join algorithm never asks a full parent | J never obtains L2 (R6 fail, F4) |
| T=15,000 | publisher | Ch2 §2.3.2 sampling, once per segment | s = 15 (implementation cap `min(15)`; spec unbounded); registration probability 2^−15; `SwarmSize` = 10,400 × 32,768 = 340,787,200; `RelayCount` = 7,280 × 32,768 | new relays are undiscoverable through the DHT; ladder reads 238 M relays |
| T=90,000 to 135,000 | 10,000 old registrants | Ch2 §2.3.2 step 4 refresh every 90 s | each refreshes without reading s (nothing delivers it); ActiveCount stays ≈ 10,400 | the "age out over 135 s" settling of SOLUTION-037 does not happen |
| T≈300,000 | guardians | 5-minute mean session (Ch2 §2.3.2) | old registrants leave; replacements register at 2^−15; ActiveCount → 1 (source) → N_est = 32,768 → s = 2 → next second N_est = 4 → s = 0 → everyone registers → ratchet repeats | s oscillates with period ≈ session length |

## Requirement checks

| Req | Verdict | Evidence |
|---|---|---|
| R2 SJL ≤ 1.5 s | fail | Strict reading: 3.3 to 4.3 s (ticks at t = 2,300 to 4,300). Lenient best case 2.2 s. Pre-media path alone is 1.14 s (discovery 0.58 + JOIN 0.32 + probe/handshake 0.24). The CONNECTING gate adds ≥ 1.1 s whenever the top layer is full. Ch4 §4.3.1, Ch5 §5.1.3, Ch1 §1.3.1, Ch8 §8.2. The implementation's own gates are `sjl_mean_max_s = 3.6` and `sjl_p95_max_s = 4.5` (`bs-sim/src/scenario.rs`); no simulator scenario exercises a flash crowd. |
| R3 depth ≤ 7, D_max 8 | pass (converged shape) | mean 3.84, max 5 after the crowd (tick T = 10,000). Holds only if relays sit above subscribers; in L0 trees no rule produces that order (ISSUE-061). |
| R6 fairness | fail | J at rank 1.25 cannot request a full L2 parent because §2.2 step 2 drops every K_avail = 0 candidate before step 4 (tick T = 12,300). Leaf floor: L is served by displacement only in the 3 % race case; without the race a leaf at a saturated L0 parent is never admitted (tick T = 5,900). A rank-0 newcomer getting L0/L1 and being refused L2 is as designed. |
| R9 PSR ≤ 0.1 % | pass for the walk | no rule takes a slot from a sitting child except the warm displacement (tick T = 5,900 + 1 RTT, no gap). Depth-3 relays go from 8.3/10 to 10/10 children during the crowd, so the ISSUE-072 queueing starvation applies at full relays; the swarm-wide number is owed to simulation. |
| R10 CDO ≤ 2 % | pass swarm-wide | crowd control ≈ 2,000 × 65 KB = 130 MB over 10 s = 104 Mbps against 60 to 72 Gbps of video: 0.17 %. Guardians locally: 0.55 Mbps control against 8.6 Mbps video = 6 %. |

## Candidate findings

### F1 (High): the sampling exponent ratchets once N_est exceeds N_0

Rule: Ch2 §2.3.2 *Sampled Registration*: "The publisher chooses s = max(0, ⌈log2(N_est / N_0)⌉) with N_0 = 10^4, and estimates N_est = median_guardians(ActiveCount) · 2^s from the STORE_RECORD_ACK counts."

With inputs ActiveCount = 10,080 and s = 0 the spec produces s = 1; one segment later, with the same ActiveCount (nobody has stopped refreshing), it produces N_est = 20,160 and s = 2; then 3, 4, 5, one step per segment, without bound (ticks T = 1,000 to 15,000). That is wrong because the estimator applies 2^s to registrations made under earlier values of s, and because no frame ever tells a registered peer that s changed: refresh is `REGISTER_PEER`, not `GET_PEERS`, and the Stream Record is read only at join. ActiveCount therefore never halves. SOLUTION-037's residual risk assumes it does ("half the registered peers stop refreshing and age out over 135 s"); that assumption has no mechanism behind it. The hysteresis SOLUTION-037 leaves to implementation (step up at 2 N_0 2^s) stops this crowd at s = 0 but ratchets at equality from the next boundary: 20,000 × 2 = 40,000 = 2 N_0 2^1.

Consequences at T = 15 s: registration probability 2^−15, so every new relay's capacity is invisible to the DHT entry point; `SwarmSize` 340 M, so PoW tier 14 for every joiner from T = 4 s (4× the hashing); `RelayCount` 238 M, so an unfold fires on any folded layer; ŝ_m divided by 2^s, so the base-layer fold trigger is blind. When the old sessions end the count collapses, s returns to 0, and the cycle repeats with the session length as its period.

The implementation (`src/crates/bs-core/src/node/dht.rs` `dht_store_record`) computes `want = ceil(log2(swarm_size / n0)).min(15)` with `swarm_size = median(active) << s` once per segment: the same ratchet, capped at 15. No simulator scenario reaches N > 10,000, so it is untested.

Fix sketch: guardians count registrations per the s they were refreshed under (`REGISTER_PEER` carries s, or the guardian tags each record with the s in force) and report Σ count_i · 2^{s_i}; the publisher changes s at most once per 135 s; a registrant learns the current s from the registration acknowledgement (F7). Touches Ch2 §2.3.2, §2.3.3 `STORE_RECORD_ACK`, App B N_0 row.

### F2 (High): the join sequence cannot render inside 1.5 s

Rule: Ch4 §4.3.1 step 2: "the playout deadline is set Δ_buffer = 3.0 s behind the live edge as usual", step 5: "PULL any blocks of segment X (and any gaps) from active-set neighbors to fill the initial buffer"; Ch5 §5.1.3: "every unchoked neighbour may draw β_pull = 0.5 Mbps" and "one slot rotating every 2 s".

With inputs RTT 80 ms, N = 10^4 and 10 Mbps neighbours the spec produces: media path open at t = 1.14 s; first renderable segment X−2 due 1 s after anchoring; backfilling its 1.5 Mbit base layer needs 3 optimistic slots open at the same instant, but each neighbour opens one slot per 2 s window at an unsynchronised phase, so the expected count open within 1 s is 4 of 8 and X−2 is missed; X−1 renders at 3.3 to 4.3 s. That is wrong because Ch8 §8.2 sets SJL ≤ 1.5 s and README "Design goals" promises it. The deficit is structural: the pre-media path alone (0.58 s discovery + 0.32 s JOIN + 0.24 s probe and handshake) is 1.14 s, and a full 6 Mbit segment at 0.5 Mbps per slot needs 12 slots where a joiner has at most 8 neighbours each offering 1. SOLUTION-023 lists "whether β_pull lets a joiner backfill 750 KB within 1.5 s" as validation owed; the arithmetic answers it without a simulator. The implementation's default gate is 3.6 s mean (`scenario.rs`: "the first frame renders Δ_buffer (3 s) after the first manifest").

Fix sketch: either define SJL as time to the first verified block and drop the 1.5 s claim to what the deadline rule allows (≈ 3.3 s), or let a joiner start playout at the first complete L0 segment and grow the buffer by playing slightly slow; exempt late-join backfill from β_pull (it is the one PULL the reserve is for). Touches Ch4 §4.3.1, Ch5 §5.1.3, Ch8 §8.2, README.

### F3 (Medium): ACTIVE is gated on the top layer, so a full top layer delays every joiner's playback by two failed rounds

Rule: Ch1 §1.3.1: "`ACTIVE` (Parents connected for all **subscribed** trees, live edge anchored)" with T_sub = {m : layer(m) ∈ L_sub} and L_sub = {L0 … L_j} at join.

With inputs: L0 and L1 parents at t = 1.14 s and trees 5 and 6 full (the §5.2 common case at 10 Mbps relays: 1080p needs 16.4 Mbps at ℓ = 0.3), the spec keeps J in CONNECTING until it has shed L2, which takes 2 failed rounds with backoff ≥ 1.0 s each: ACTIVE at t = 2.3 s, while L0 data has been arriving for 1.16 s. That is wrong because §1.3.1 itself says the floor is what gates playback ("A peer that cannot obtain a parent in every L0 tree does not enter ACTIVE at all") and because the delay applies to 100 % of joiners whenever the top layer is oversubscribed, which Ch1 §1.1.5 calls the common case. The implementation gates identically (`check_active`: `all_subscribed && any_parent`) with 250/500 ms backoffs (1.1 s). Related to ISSUE-063 (joiners stuck in CONNECTING under the unbounded warming exemption) but independent of it.

Fix sketch: gate ACTIVE on the L0 trees; render the layer prefix whose trees have parents; keep joining higher layers in the background exactly as assigned trees are joined. Touches Ch1 §1.3.1, Ch1 §1.1.5 §5.3.

### F4 (High): Rank Admission cases 2 and 3 are unreachable from the join algorithm

Rule: Ch1 §1.2.2 §2.2 step 2: "if probe_response.success and probe_response.available_slots > 0: # Step 3: Execute Scoring Function"; step 4 sends `NEIGHBOR` only to `scored_candidates`. Case 2: "No free slot, T_m carries an enhancement layer, and r_new > 1.25 · r_min: ACCEPTED, and the parent preempts". Case 3: "No free slot, T_m carries L0, the requester is leaf-class … ACCEPTED, and the parent displaces".

With inputs J at rank 1.25 and every tree-5 candidate at K_avail = 0, the spec produces a failed round and a shed, never a `NEIGHBOR` (tick T = 12,300); with inputs leaf L and a saturated L0 parent, it produces the same (tick T = 5,900). Both cases begin with "No free slot", and the only way a `NEIGHBOR` reaches a parent with no free slot is the race between probe and request (≈ 3 % per attempt here). That is wrong because §2.2's own text ("Preemption is what turns rank into placement", SOLUTION-023) and §5.5 ("a leaf is served whenever it asks and the share is not yet full", SOLUTION-049) require the request to be sent. The migration loop (§2.3) does not rescue case 3 (a leaf with no L0 parent has nothing to migrate from) and does not rescue case 2 for a peer that has shed the layer (re-join runs the same algorithm). ISSUE-061 is the special case for a relay assigned to the tree; the implementation fixes only that case (`finish_probing`: `else if we_relay_this_tree { saturated.push(...) }`), sends no `NEIGHBOR` to a full parent for a leaf or a ranked joiner, and does not implement case 3 at all (`Params::leaf_slots` has no caller). SOLUTION-005 relies on the filter to hide warming relays; the filter is right for WARMING and wrong for SERVING with K = 0.

Fix sketch: in step 2, keep a SERVING candidate with K_avail = 0 when the requester is leaf-class and the tree carries L0, or when the requester holds a rank proof and the tree carries an enhancement layer (or relays the tree, ISSUE-061); score it with CapacityCredit = 0 and request it after every candidate with a free slot. Count it as saturated only if the parent answers `REJECTED_SATURATED`. Touches Ch1 §1.2.2 §2.2 pseudocode and the TreeState table; cluster with ISSUE-061.

### F5 (Medium): guardian discovery egress has no budget

Rule: Ch1 §1.2.1 *The Two Budgets, Each Owned by One Expression*: tree slots + bridges ≤ (1 − r_pull) u_v; PULL slots + handovers ≤ r_pull u_v. Ch1 §1.1.5 §5.6: "A `GET_PEERS` reaches ≈ α of the k = 20 guardians"; Ch2 §2.3.3: the response is "≈ 1,400 bytes".

With inputs 200 joins/s, 2 `GET_PEERS` per joiner, α/k = 3/20 the spec produces 60 responses/s per guardian = 0.55 Mbps (IPv4) to 0.67 Mbps (IPv6) of egress on a 10 Mbps relay whose budgets already own 9.6 Mbps; the uplink is at 10.2 to 10.3 Mbps for the 10 s of the crowd. At N = 10^6 steady state SOLUTION-037's own 500 queries/s is 4.6 Mbps, half the guardian's uplink. That is wrong because the slot count then over-commits the guardian's link and its tree children starve (the ISSUE-072 mechanism), while SOLUTION-037 records the load as "bounded, but not small" without the budget arithmetic. The 5 % headroom of ISSUE-072 (0.5 Mbps) does not cover it.

Fix sketch: a guardian subtracts its measured DHT egress from the tree budget when computing K_v (release through `DRAIN_NOTICE` reason `CAPACITY`), or the k closest nodes are required to be `RELAY_CAPABLE` (≥ 20 Mbps). Touches Ch1 §1.2.1 §1.3, Ch2 §2.3.2 *Who Is a Guardian*; cluster with ISSUE-072.

### F6 (Low, impl): join retry cadence is 250 ms, the spec requires ≥ 1.0 s

Rule: Ch1 §1.1.5 §5.3 item 2: "the retry backoff interval **must** be at least one segment period (1.0 s), so that no shed decision is ever taken on a sample shorter than a single warm-up window."

`src/crates/bs-core/src/params.rs` sets `join_backoff: 250 ms`; `node.rs` `join_round_failed` uses `join_backoff × 2^min(failed, 2)` = 250, 500, 1,000 ms. With inputs two saturated rounds the implementation sheds after 0.75 s plus two probe rounds; the spec's minimum is 2.0 s plus two rounds. ISSUE-063 cites the 250 to 500 ms cadence as a fact; the divergence itself is not filed.

### F7 (Low): `REGISTER_RESPONSE` is drawn but has no frame

Rule: Ch2 §2.3.2 diagram: "<--- REGISTER_RESPONSE(200 OK) ---". Appendix D lists `REGISTER_PEER` (0x0A) and no response type.

A registrant cannot tell a dropped registration (XDP budget, Ch7 §7.2.1) from an accepted one, and the acknowledgement is the natural carrier of the current s (F1). Pattern #4 of issues/README.md.

### Already known, confirmed by the walk

- ISSUE-061: the ≈ 233 new relays assigned to each L2 tree arrive at rank 0, find every slot held by subscribers, and stay `UNPARENTED` (tick t = 980, trees 5 and 6); each such candidate also counts as a failed candidate for every joiner that samples it.
- ISSUE-063: a WARMING new relay appears in ≈ 30 % of tree-1 samples during the crowd; unbounded, it would exempt those rounds. Not hit here because tree 1 has free slots.
- ISSUE-072: depth-3 relays go from ≈ 8.3 to 10 of 10 children during the crowd and run at 100 % of (1 − r_pull) u_v; the last-admitted children queue.

## Spec silences assumed through

- S1: the PoW tier for the first `GET_PEERS`, before any Stream Record has been read. Assumed tier 12 (the tier for 10^3 ≤ N < 10^5); the guardian's one-tier grace (Ch2 §2.2.1) accepts it against tier 14. No consuming rule diverges beyond ≈ 60 ms of hashing.
- S2: `GET_PEERS` stratification ⌈20 / popcount(WantedTrees)⌉ = 4 per tree gives 24 > 20 at six wanted trees. Assumed round-robin to 4,4,3,3,3,3. Harmless.
- S3: whether a joiner may render a segment before its 3.0 s deadline. Strict (wait for the deadline) and lenient (render on completion) readings differ by ≈ 1 s in SJL; both miss 1.5 s (F2).
- S4: whether a failed re-join of a shed layer after the hysteresis sets `StarvedTrees` on the re-query. Assumed yes; only affects StarvedCount for the top layer, which never folds.
- S5: the source's upload at N = 10^4 (assumed 100 Mbps). Changes K_S (18 / 9 slots) and the depth-1 count, not any finding.
- S6: whether tree joins may start during JOINING. Assumed no (Ch1 §1.3.1 puts tree joins in CONNECTING); the lenient reading saves ≈ 0.3 s of SJL and does not change F2.
- S7: the order in which relays and subscribers occupy depth 1 to 3 in L0 trees after an hour of churn. Assumed relay-shallow; the rule that would produce it is the subject of ISSUE-061.
