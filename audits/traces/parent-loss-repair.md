# Trace: parent-loss-repair

Walker scenario `parent-loss-repair`. Parent loss and repair in one tree, then the churn storm.
Every number below is derived from Appendix B constants and the per-path RTT figures the spec
itself uses (20 / 80 / 250 ms). Where the spec gives a range, the worst end is walked too.

## 1. Scenario and initial state

| Item | Value | Derivation |
|---|---|---|
| N, M | 1,000 peers, M = 4 | Scenario. Ladder rung M = 4 needs 18 <= N_relay < 24 for *growth*; a stable swarm with N_relay = 800 sits at M = 6. The walk keeps M = 4 as given (a swarm that has not yet stepped, or `dynamic_forest` off as in the impl). Nothing below depends on the rung except per-tree bitrates. |
| Slicing matrix at M = 4 | (t0,t1,t2) = (1,1,2); B_m = 1.5 Mbps on every tree | Ch1 §1.2.4 §4.2.1 reference ladder. Tree 1 = L0 (480p base), **Tree 2 = L1 (720p enhancement)**, Trees 3,4 = L2 stripes. |
| Omega | 1.15 | parity floor E = 1, f_frame = 0.085 (App B) |
| Blocks per chunk, tree 2 | 1.5 Mbps x 0.25 s = 375 kbit = 46.9 KB -> 3 x 16 KB blocks (last padded); ~12 blocks/s | Ch4 §4.1.1 |
| Dead relay R* | depth 3, tree 2, 8 children, u = 16 Mbps, t_v^eff = 1 | K_v(2) = floor(0.9 x 16 / (1 x 1.5 x 1.15)) = floor(8.35) = 8. A 16 Mbps node has max_trees = floor(14.4/6.9) = 2 but max_trees^eff = 1 until Theta^rate >= B, so a single-tree relay with 8 slots is legal. Note: an ordinary 10 Mbps relay holds only K_v = floor(9/1.725) = 5 slots per 1.5 Mbps tree. |
| R* death | silent at t = 0; last block left R* at t = -30 ms | Scenario |
| Children o1..o8 (orphans in tree 2) | o1, o2 are RELAY-class and relay tree 2 (bit 1 set); o1: K_avail(2) = 3, o2: K_avail(2) = 1. o3..o8 do not relay tree 2 (expected 8/M = 2 relay it; P(none) = (3/4)^8 = 10 %). All 8 hold distinct parents in trees 1, 3, 4 (distinct-parent rule, Ch1 §1.2.1). | Ch1 §1.2.3 *The Roster Lists Relays of This Tree Only* |
| Last ROSTER(TreeID = 2) from R* | RosterSeq s, sent at t = -400 ms (<= tau_roster old), E = 2 entries: [o1 (K 3), o2 (K 1)], ChildCount k = 8 | App D §D.4.15 |
| Per-tree pool P_2 at every orphan | >= 3 relays of tree 2, reachable, not current parents | Ch3 §3.1.1 floor |
| RTT paths walked | 20 ms (RTTVAR 5), 80 ms (RTTVAR 10), 250 ms (RTTVAR 10) — the RTTVAR values are the only ones that reproduce App B's 240 / 320 / 490 ms; see silence S8 | App B tau_evict row |
| Arrival of R*'s last block at a child | t = -30 + RTT/2: **-20 ms** (20), **+10 ms** (80), **+95 ms** (250) | one-way delay |
| Playout | Delta_buffer = 3.0 s behind the live edge; W_pull = clamp(tau_evict + 6 SRTT, 1.5, 2.0) = **1.5 s** (20: 0.24+0.12), **1.5 s** (80: 0.32+0.48 = 0.80 -> floor), **1.99 s** (250: 0.49+1.50) | Ch4 §4.3.1 |
| PULL service | beta_pull = 0.5 Mbps per unchoked slot; a 10 Mbps neighbour has PullSlots = 2 | Ch5 §5.1.2 |

Deputy arithmetic from the shared roster (Ch1 §1.2.3 §3.2): E = 2, K^(1) = 3, K^(2) = 1, C = 4,
k = 8. D = min(E, ceil(k / K^(1))) = min(2, 3) = 2 deputies. Surplus iff
Blake3(NodeID) mod 2^16 / 2^16 > C/k = 0.5. Fixed hash outcome for the walk: o3 (0.12), o4 (0.31),
o5 (0.44) contend; o6 (0.61), o7 (0.77), o8 (0.93) are surplus. deputy_index: o3 -> 0 (o1),
o4 -> 1 (o2), o5 -> 0 (o1). Loads: o1 takes 2 of 3 free slots, o2 takes 1 of 1. The deputies o1, o2
are assumed exempt from the hash (silence S3).

## 2. Tick table — single parent loss, 20 ms path (RTTVAR 5 ms)

tau_evict = max(200, 2 x 100 + 20 + 4 x 5) = **240 ms** from the last packet received (t = -20).

| t_ms | node | rule (file §) | computation | state after |
|---:|---|---|---|---|
| -400 | R* -> o1..o8 | Ch1 §1.2.3 roster; App D §D.4.15 | ROSTER(2, seq s, k=8, E=2: o1 K3, o2 K1), 12 + 2 x 43 = 98 B per child | every orphan holds the same snapshot, age < 5 s |
| -30 | R* -> o_i | Ch4 §4.1, pacing Ch3 §3.3.2 | last BLOCK_PROOF + 16 symbols of chunk c(-1) leave R* | — |
| -20 | o_i | Ch3 §3.3.2 line 2 | LastReceivedTime(R*) = -20 | timer base |
| 0 | R* | scenario | dies silently; no DISCONNECT, no DRAIN_NOTICE | R* gone from all 4 trees (32 child edges in all; this walk follows the 8 in tree 2) |
| 0..250 | o_i parents in trees 1,3,4 | Ch1 §1.3 core loop line 36 `ProcessStreamingBuffers(JoinTrees \ Repairing)` | tree 2 not yet in Repairing; trees 1,3,4 deliver chunk c0 (3 blocks each) on schedule | live edge X_edge advances to c0 from the three live parents; tree-2 blocks of c0 missing |
| 80 | o_i | Ch3 §3.3.2 line 4: silence > tau_ping = 100 ms | -20 + 100 = 80 | PING sent to R* (control channel); `pinged` |
| 80..220 | o_i | Ch3 §3.3.1 rule 1 | a healthy R* would have sent a heartbeat by -30 + 100 = 70 (arrives 80); none arrives | silence continues |
| 220 | o_i | Ch3 §3.3.2 line 7a–11: silence > tau_evict | -20 + 240 = 220. Siblings fire within +/- 4 RTTVAR = 20 ms of one another (218..238) | R* evicted from Active Set, demoted to Passive (TIMEOUT); tree 2 added to Repairing. **Trees 1,3,4 keep forwarding (core loop line 36; checked at every tick below).** |
| 220 | o3..o8 | Ch1 §1.2.3 §3.2 | roster age 620 ms < 5 s; E = 2 > 0; D = 2; surplus test | o3,o4,o5 elect deputies o1/o2/o1; o6,o7,o8 skip to Ch3 §3.3 |
| 230 | o1, o2 (deputies), o6, o7, o8 | Ch3 §3.3.1 T = tau_evict + 10 | filtered pick from P_2: highest R then K_avail; **no depth criterion in the fast path** (silence S9) | QUIC ClientHello to standby S_i; no ticket (never in Active Set) -> 2 RTT path |
| 230 | o3, o4, o5 | Ch1 §1.2.3 Phase 3 | QUIC ClientHello to o1 / o2 at the roster address; **tau_deputy = 45 ms armed -> fires at 275** | handshake in flight; 1 RTT to complete = 250 |
| 250 | o3, o4, o5 -> deputies | App D §D.4.3 | NEIGHBOR(TreeID=2, Priority=HIGH, NodeClass, AssignedTrees) on the new control stream | deputy must answer within tau_sched = 100 ms |
| 250 | o1,o2,o6,o7,o8 -> S_i | Ch3 §3.3.2 line 20 | NEIGHBOR(2, HIGH) after the 1-RTT handshake | — |
| 255 | o1 | Ch1 §1.2.2 Rank Admission 1: free slot | o1 has K_avail 3, receives o3 and o5 -> ACCEPTED(HopDepth = o1's *stale* depth 4) x 2; o2 -> ACCEPTED to o4 | deputies hold children in a tree they have no parent in (silence S4/S5) |
| 270 | o3, o4, o5 | Ch1 §1.2.3 Phase 3 | ACCEPTED arrives at 230 + 2 x 20 = 270 **< 275: margin 5 ms**. At RTT >= 22.5 ms (no ticket) or >= 45 ms (ticket) the timer fires first — see F1 | o3,o5 children of o1; o4 child of o2; depth 5 provisional |
| 270 | S_i -> o1,o2,o6,o7,o8 | Rank Admission 1 | ACCEPTED(2, HopDepth h_S). S_i picked by R/K_avail, so h_S in {2,3,4}; take h_S = 3 (a sibling of R*) | o1,o2,o6,o7,o8 at depth 4; tree 2 leaves Repairing on ACCEPTED? No: Ch1 §1.3 says RESTORED on re-route success; data starts next block |
| 270..310 | S_i -> o1 | pacing: a chunk's 3 blocks leave over <= 125 ms, so the next block is <= 83 ms away; take 30 ms + 10 ms one-way | first block of c1 at o1 at ~310 | o1 verifies (Ch4 §4.1), sets depth = SenderHopDepth + 1 = 4 (App D §D.4.9), forwards to o3,o5 |
| 320 | o3, o5 | App D §D.4.9 | BLOCK_PROOF from o1 with SenderHopDepth 4 | depth 5; tree 2 streaming again. **Repair complete 340 ms after the last packet, 320 ms after death** (spec table says ~280 counting from tau_evict + re-attach only) |
| 320 | o_i | gap accounting | missing in tree 2: all of c0 (3 blocks) and the first ~1 block of c1 = **4 blocks = 0.52 Mbit** | PULL zone must repair them |
| 1000 | neighbours -> o_i | tau_gossip = 1 s, Ch4 §4.3.2 | BUFFER_STATE_BITFIELD for c0 from every neighbour that subscribes L1 (most of the swarm) | o_i knows who has c0's tree-2 blocks |
| 1500 | o_i | Ch4 §4.3.3 scheduler, W_pull = 1.5 s | deadline(c0) = t_live(c0) + 3.0 = 3.0 s; enters the PULL zone at 3.0 - 1.5 = 1.5 s | PULL_REQUEST(c0, b0..b2, MissingSymbolCount 0, UrgencyMs 1500) to the best provider; **served only if that neighbour has o_i unchoked** (Ch5 §5.1.2 — already ISSUE-066) |
| 1520..2310 | provider -> o_i | beta_pull = 0.5 Mbps -> 16 KB = 131 kbit = 262 ms per block | 3 blocks of c0 by 1.52 + 0.79 = 2.31 s < 3.0 s; the c1 block by 2.57 s < 3.25 s | all 4 gap blocks in; **no starvation; o_i renders 720p throughout** |
| 3000 | o_i | playout | chunk c0 complete in L0 (tree 1) and L1 (tree 2, pulled); L2 stripes delivered | PSR contribution 0 |
| 5000 | o3, o4, o5 | Ch1 §1.2.2 §2.3 migration tick | Score(o1) vs pool: o1 at depth 4 vs S_i at depth 3: Delta_h(4)/2 = 29 ms margin; S_i wins if its RTT is within ~29 ms of o1's and it has a free slot | branch unwinds upward over the following 5 s ticks (accepted design, Ch1 §1.2.2 *Depth Propagation*) |

## 3. Tick table — 80 ms and 250 ms paths (same event order; only the numbers move)

tau_evict: 2 x 100 + 80 + 4 x 10 = **320** (80 ms); 2 x 100 + 250 + 4 x 10 = **490** (250 ms). Last packet at
+10 / +95.

| t_ms (80) | t_ms (250) | node | rule | computation / state after |
|---:|---:|---|---|---|
| 110 | 195 | o_i | tau_ping | PING to R* |
| 330 | 585 | o_i | tau_evict | evict R* for tree 2; Repairing = {2}; trees 1,3,4 unaffected |
| 340 | 595 | o3,o4,o5 | deputy request | ClientHello to deputy; tau_deputy fires at **385 / 640** |
| 420 | 845 | o3,o4,o5 | handshake done | NEIGHBOR sent only now (1 RTT) — already **35 / 205 ms after the timer fired** |
| 385 | 640 | o3,o4,o5 | tau_deputy expiry, Ch1 §1.2.3 Phase 3 *Fallback* | abandon election; start Ch3 §3.3 to a pool standby (2 RTT) |
| 500 | 1095 | deputy -> o3 | ACCEPTED arrives (340 + 2 x 80 / 595 + 2 x 250) | **orphan already in flight to a standby**; spec silent on the late ACCEPTED (S2) — either two parents in tree 2 until one is DISCONNECT_CHOKEd, or a leaked slot at the deputy until the 3-segment receipt deadline |
| 500 | 1095 | S_i -> o1,o2,o6,o7,o8 | standby ACCEPTED | 340 + 160 / 595 + 500 |
| 555 | 1150 | S'_j -> o3,o4,o5 | fallback ACCEPTED | 385 + 10 + 160 / 640 + 10 + 500 |
| ~620 | ~1260 | S -> o_i | first block (next paced block + one-way) | **repair complete ~610 ms / ~1,165 ms after the last packet**; spec table: 480 / 990 measured from death ignoring the one-way arrival offset. With the RTTVAR the impl seeds (SRTT/2 -> 360 / 950 ms tau_evict) the 250 ms figure is ~1.6 s |
| gap | gap | o_i | bytes missed in tree 2 | 80 ms: 10 -> 620 = 0.61 s = 2.4 chunks = **8 blocks (1.05 Mbit)**. 250 ms: 95 -> 1260 = 1.17 s = 4.7 chunks = **14 blocks (1.83 Mbit)** |
| 1500 | 1010 | o_i | PULL zone entry for c0 (W_pull 1.5 / 1.99) | windows: c_n enters at t_live(c_n) + 1.5 (80) or + 1.01 (250), deadline t_live(c_n) + 3.0 |
| — | — | o_i | **one unchoked PULL slot (0.5 Mbps, 262 ms per block), 2 PULL RTTs first** | 80 ms: c0 (3) done 1.5 + 0.16 + 0.79 = 2.45 OK; c1 (3) 3.24 vs 3.25 OK by 10 ms; c2 (2) 3.76 vs 3.50 **late** -> 2 blocks lost, L1 undecodable for c2 (L2 with it), one 250 ms chunk at 480p. 250 ms: c0 done 2.30 OK; c1 3.08 OK; c2 3.87 vs 3.50 **late**; c3 4.66 vs 3.75 **late**; c4 (2) 5.2 vs 4.0 **late** -> **7 of 14 blocks lost**, three chunks at 480p |
| — | — | o_i | **two unchoked slots (1.0 Mbps, 131 ms per block)** | 80 ms: all 8 by 2.7 s OK. 250 ms: c0 2.0, c1 2.4, c2 2.8, c3 3.2, c4 3.5 — all OK |
| — | — | o_i | tree 1 (L0) instead of tree 2 | identical arithmetic: the late blocks are a **freeze** (250 ms per late chunk) instead of a resolution drop — see F2 |

## 4. Churn storm (Ch8 §8.3 A): 30 % of relays die at t = 0

Pre-storm state: N = 1,000, ell = 0.2 -> N_relay = 800, 200 per tree. u_S = 50 Mbps: R_src = 3 x 1.5 x 1.15 = 5.2,
K_S(m) = floor(44.8 / (4 x 1.725)) = 6. Two capacity points:

| | u_relay = 10 Mbps | u_relay = 12 Mbps |
|---|---|---|
| K_v per 1.5 Mbps tree | floor(9/1.725) = 5 | floor(10.8/1.725) = 6 |
| slots per tree before | 200 x 5 + 6 = 1,006 >= 1,000 (margin 0.6 %) | 1,206 (margin 21 %) |
| dead | 240 relays (60 per tree), also 240 subscribers -> N' = 760 | same |
| orphan edges | 760 x (1 - 0.7^4 = 0.76) peers lose >= 1 parent; 760 x 1.2 = 912 edges, **228 per tree**; 30 % of peers (228) lose their **L0** parent | same |
| slots after | 140 x 5 + 6 = 706 < 760 -> **54 peers per tree cannot re-attach**; in tree 1 that is a base-layer **freeze for 7 %** | 846 >= 760, free = 846 - 532 = 314 >= 228 OK |
| recovery of the shortage | fold rule Ch1 §1.1.5 §5.6: starved share 7 % > s_fold 5 % over tau_fold = 10 s (L0), then a 5-segment window -> (2,2) at 0.75 Mbps, K_v = 10 -> 1,406 slots; **freeze lasts >= 15 s** for the 54 | not triggered |
| PSR over a 10-min run | 54/760 x 15/600 = **0.18 % > 0.1 %** (an accepted minimax transient, Ch1 §1.2.4 §4.2.1; SOLUTION-058) | from the repair path alone, below |

Precondition for "no shortage after a 30 % relay storm": 0.7 x S >= N - 0.3 N_relay, i.e. slots/children
>= 1.43 before the storm -> u_relay >= 11.5 Mbps at ell = 0.2, M = 4. The spec states no such
precondition for Scenario A (R11 undefined).

Ticks, 20 ms path, u_relay = 12 Mbps, one orphan o in tree 1 (L0):

| t_ms | node | rule | computation | state after |
|---:|---|---|---|---|
| 0 | 240 relays | scenario | die silently | 912 orphan edges; 25 % of rosters empty (P(no child relays m) = (3/4)^4.85); of the rest the first deputy is dead w.p. 0.3 |
| 220 | o | tau_evict | evict | Repairing = {1} (and any other lost tree, independently) |
| 230 | o | election | deputy dead w.p. 0.3 -> no ACCEPTED | tau_deputy fires at 275; fallback |
| 285 | o | Ch3 §3.3.2 line 15–17 sequential loop | best pool candidate dead w.p. 0.3; `InitiateQUICStream` to a dead host has **no stated timeout** (S1). Impl: 3 x probe_timeout = 600 ms. A QUIC default idle/handshake timeout is ~10 s | with T_hs = 600 ms: expected +0.3 x 0.6 + 0.09 x 1.2 = 0.29 s; P(all 3 dead) = 2.7 % -> tier 4 DHT (1–3 s) |
| 285 | o | **no TreeState check in the fast path** | 30 % of surviving pool candidates lost their own tree-1 parent at t = 0 and are UNPARENTED; the Rank Admission Rule still says "free slot -> ACCEPTED" | o attaches to an unparented relay U; U heartbeats; no blocks until U repairs — or never, if U's own pick closes a cycle (F3) |
| 325 | o | 2 RTT | ACCEPTED from a live, parented candidate (P ~ 0.49 on the first try) | repair ~0.35 s |
| 925 | o | after one dead candidate (T_hs = 600) | ACCEPTED on the second | repair ~0.95 s, 30 % of orphans |
| 1525 | o | two dead (9 %) | third candidate | 1.55 s; PULL zone starts at 1.5 s, gap now ~18 blocks of L0 at 1.5 Mbps -> needs >= 2 slots (as §3) |
| 1500–4500 | o | three dead (2.7 %) -> tier 4 | DHT walk 1–3 s | base-layer gap 2.5–4.5 s > Delta_buffer **3.0 s: freeze** for ~0.8 % of peers per lost L0 edge (228 x 0.027 = 6 peers), ~1–2 s each |
| PSR | swarm | T_hs = 600 ms | 6 x 1.5 s / (760 x 600 s) = **0.002 %** | pass |
| PSR | swarm | T_hs = 10 s (QUIC default) | 30 % of the 228 L0 orphans wait >= 10 s on a dead first candidate: 68 x 8 s / (760 x 600 s) = **0.15 % > 0.1 %** | fail — R9/R11 hinge on S1 |

Trees not hit keep forwarding at every tick: the core loop processes `JoinTrees \ Repairing`, and
`Repairing` is per tree (Ch1 §1.3). Checked: no rule in Ch1 §1.3, Ch3 §3.3 or Ch4 §4.3 pauses
forwarding or PULL service in the other trees during the repair or the storm.

## 5. Requirement checks

| Req | Verdict | Evidence |
|---|---|---|
| R1 | pass | Playout stays 3.0 s behind the live edge, which the three live trees keep advancing; the repaired branch sits one hop deeper (5 <= 7 at N = 1,000). Scenario does not measure glass-to-glass. |
| R2 | not_applicable | No join. |
| R3 | not_applicable | N = 1,000. |
| R4 | not_applicable | At N = 2 the roster is empty and the orphan goes straight to Ch3 §3.3 (defined); not walked. |
| R5 | fail | (a) Repair complete at **0.32 / 0.61 / 1.17 s** after the last packet on 20 / 80 / 250 ms paths (spec's own 0.28 / 0.48 / 0.99 omit the one-way arrival offset); at 250 ms with RFC 9002 RTTVAR seeding (SRTT/2) it is ~1.6 s — "sub-second" holds only to ~150 ms RTT. (b) Scope to one tree holds at every tick. (c) "resolution lost, never playback": at M = 4 the base layer sits on one 1.5 Mbps tree, so a shortage after the storm freezes 7 % of peers for >= 15 s until the fold (known transient, Ch1 §1.2.4, SOLUTION-058). |
| R6 | not_applicable | Leaf orphans are never deputies and still re-attach; no ranking decision in the walk. |
| R7 | pass | Every pulled block is verified against the chunk MANIFEST before use (BLOCK_TRANSMISSION carries the inline proof); roster addresses are parent-observed (App D §D.4.15); NEIGHBOR carries no address. |
| R8 | pass | Source does nothing in the repair; R_src = 5.2 Mbps constant; K_S = 6 unchanged by the storm. |
| R9 | undefined | Single loss: 0 with >= 2 PULL slots; with one slot 2/8 (80 ms) and 7/14 (250 ms) gap blocks are late — on an L0 tree a freeze of 1–3 chunks per event. Storm: 0.002 % with a 600 ms handshake timeout, 0.15 % with a 10 s one; the timeout is unspecified (S1). |
| R10 | pass | During repair: ROSTER 98 B/s per child (8 children: 6.3 kbps = 0.04 % of 16 Mbps); heartbeats fire once per chunk per child in steady state because pacing (<= 125 ms) leaves >= 125 ms idle > tau_ping (4/s x ~60 B x 8 = 15 kbps = 0.1 %); bitfields 1/s x 8 neighbours x ~40 B; PULL_REQUEST 15 B each. Total < 0.5 % << 2 %. |
| R11 | undefined | Churn storm absorbed without a slot shortage only if pre-storm slots/children >= 1.43 (u_relay >= 11.5 Mbps at ell = 0.2); at the 10 Mbps margin 7 % lose L0 for >= 15 s. Starvation from the repair path depends on the unstated handshake timeout (S1) and on cycle avoidance (F3). The spec states neither precondition. |

## 6. Candidate findings

### F1 (High) tau_deputy = 45 ms cannot be met by a sibling that needs 1–2 RTT to answer
- Quote: Ch1 §1.2.3 Phase 3: "B and D immediately send `RELAY_JOIN_REQUEST` ... directly to Node C at the address the roster carries, and arm a **Deputy-response timer tau_deputy = 45 ms**." App B: "tau_deputy | 45 | ms".
- Inputs/numbers: siblings "are not generally in one another's Active Sets" (App D §D.4.15), so the request needs a QUIC handshake (1 RTT, no 0-RTT ticket) plus NEIGHBOR/ACCEPTED (1 RTT) = 2 RTT: **40 ms at 20 ms RTT (5 ms margin), 160 ms at 80 ms, 500 ms at 250 ms**. Even with a ticket, 1 RTT = 80 ms > 45 ms at the spec's reference RTT.
- Wrong because: above ~22 ms RTT every election ends in the 45 ms fallback, so the mechanism that "preserves the subtree and prevents k orphans from thundering-herd onto the same passive-set candidates" never runs on most links at N = 10^6, and costs +45 ms. The late ACCEPTED (t = 500 at 80 ms) reaches an orphan already attached elsewhere — unhandled (S2). SOLUTION-052 costed the failed deputy path at "+45 ms + another repair" in one place and "≈ 1.9 s" (i.e. 2 RTT) in another, so the solution record itself used two values.
- Fix: tau_deputy = 2 x SRTT(deputy from roster, or the orphan's own path RTT as proxy) + tau_sched, floored at 45 ms; state what an orphan does with an ACCEPTED that arrives after fallback (DISCONNECT(TreeID, CHOKE) it). Touches Ch1 §1.2.3, App B, Ch3 §3.3.1 fallback list.
- Not already known: no open ticket; SOLUTION-052 addressed re-attach, not the deputy timer.

### F2 (Medium) The PULL zone is sized for repair latency, not for the bytes the repair leaves behind
- Quote: Ch4 §4.3.1: "The zone must cover one parent repair — detection plus re-attachment, both RTT-scaled — plus two PULL round-trips." and Ch5 §5.1.2: "An unchoked neighbour may draw up to beta_pull sustained from this node."
- Numbers: gap in a 1.5 Mbps tree = 8 blocks (80 ms) / 14 blocks (250 ms); one PULL slot moves one 16 KB block per 262 ms; blocks may not be pulled before t_live + 3.0 − W_pull. With one slot: 2 of 8 blocks late at 80 ms, **7 of 14 late at 250 ms**; with two slots all in. A leaf-class orphan holds at most one optimistic slot per neighbour (Ch5 §5.1.3) and any orphan has PullRate = 0 at every neighbour right after the loss, so by the §5.1.2 rule box it is choked everywhere except optimistic slots. On tree 1 (L0) the late blocks are a freeze.
- Wrong because: the zone is derived from latency only; at the RTT the clamp ceiling was introduced for, the throughput of one slot does not cover the gap inside the zone.
- Fix: size the zone (or an exemption) on gap bytes: W_pull >= repair + 2 RTT + (repair x B_m) / (n_slots x beta_pull); or allow a repairing child to pull from its *new* parent outside TFT for the drain/repair gap (ISSUE-066's option 1 extended to repair), with the parent's reserve charged. Touches Ch4 §4.3.1, Ch5 §5.1.2, App B.
- Adjacent to ISSUE-066 (choke denies repair) but a different fix: even with ISSUE-066 closed, one 1 Mbps reserve at a 10 Mbps new parent serving 3 orphans' 1.8 Mbit gaps in ~2.2 s is 2.2 Mbit of capacity for 5.5 Mbit of demand.

### F3 (High) A parent that heartbeats but never delivers is never evicted; an unparented relay may accept, so orphaned relays can adopt each other
- Quote: Ch3 §3.3.2: "`LastReceivedTime` counts *any* packet from the parent — media, heartbeat or control". Ch1 §1.2.2 Rank Admission Rule: "1. **Free slot in T_m** ...: `ACCEPTED`." Ch3 §3.3.1 T = tau_evict + 10: "selects the one with the highest reliability R and advertised K_avail, opens a QUIC session to it and sends an urgent `NEIGHBOR`" — no PROBE, so `TreeState = UNPARENTED` is never seen.
- Numbers: in the storm 30 % of every pool's surviving tree-m relays are themselves unparented in m at t = 220. Orphans A and B whose pools rank each other first both ACCEPT (free slot), exchange heartbeats every 100 ms, and never see a block. tau_evict never fires; R is self-gossiped by the parent (Ch1 §1.2.2 Component 1), so the migration score does not fall; BLOCK_PROOFs do not flow, so depth never updates. Starvation is unbounded for A, B and every descendant in that tree.
- Wrong because: SOLUTION-022 made heartbeats keep a *repairing* relay alive on purpose, but nothing bounds how long a parent may be alive-and-silent, and nothing stops a relay with no parent in m from taking children in m.
- Fix: (1) a relay UNPARENTED in m answers NEIGHBOR(m) with DISCONNECT(m, REJECTED_SATURATED) or a new REJECTED_UNPARENTED; (2) a data-stall deadline per tree: a parent that has pushed no block for N segments (impl: 2 s) is lost for that tree even if it heartbeats; (3) a cycle guard (never accept your own tree-m parent as a child; never attach under your own child). Touches Ch1 §1.2.2, Ch3 §3.3.1–3.3.2, App D §D.4.3b.
- impl: `src/crates/bs-core/src/node.rs` lines 2709–2735 (data stall, 2 s) and 1135–1145 (cycle guard) implement all three; `src/ARCHITECTURE.md` rows 312, 322, 338 record them as impl-found gaps with no spec ticket.

### F4 (Medium, undefined) No timeout for the QUIC handshake to a dead passive-set candidate
- Quote: Ch3 §3.3.2 line 17: "Result <- InitiateQUICStream(Candidate)" inside a sequential `While` loop; App B `NEIGHBOR reply`: "silence past tau_sched + RTT means unreachable" — this applies after the session exists.
- Numbers: in the storm each pool entry is dead w.p. 0.3. With a 600 ms timeout (impl, `probe_timeout x 3`) the repair-path PSR is 0.002 %; with a QUIC default ~10 s, 30 % of L0 orphans starve >= 7 s -> 0.15 %. Two implementers diverge by an order of magnitude on a number R9/R11 depend on.
- Fix: state the handshake deadline (e.g. 2 x SRTT + 4 RTTVAR of the path that learned the record, floored at 200 ms) and that the first two pool candidates are dialed concurrently. Touches Ch3 §3.3.1–3.3.2, App B.

### F5 (Low) The tau_evict table rests on an unstated RTTVAR, and the W_pull formula exceeds its own clamp
- Quote: App B: "240 ms at 20 ms RTT, 320 at 80, 490 at 250".
- Numbers: the formula reproduces these only with RTTVAR = 5, 10, 10 ms. RFC 9002 seeds RTTVAR = SRTT/2 on the first sample: 240 / 440 / 950 ms; the impl test uses RTT/4: 240 / 360 / 698 (`params.rs` `tau_evict_examples`). At 250 ms: repair = 0.95 + 0.5 = 1.45 s; W_pull formula = 0.95 + 1.5 = 2.45 s, clamped to 2.0 — the clamp silently removes 0.45 s the formula asked for. SOLUTION-022's residual mentions the no-sample case, not the RTTVAR seed.
- Fix: state the RTTVAR assumption in App B, or quote the table as a range; derive the clamp ceiling from the 250 ms row with the stated RTTVAR.

### F6 (Low) The multi-deputy partition rule does not exempt the deputies themselves
- Quote: Ch1 §1.2.3 §3.2: "Every orphan o computes, from the shared roster: ... deputy_index(o) = Blake3(NodeID_o) mod D and sends its `RELAY_JOIN_REQUEST` to deputy deputy_index(o)." and "orphan o is surplus iff ...".
- Numbers: with D = 2, deputy o1 hashing to index 1 and o2 to index 0 attach under each other: a two-node cycle in tree m with no path to the source, kept alive by heartbeats (F3). P = 1/4 per election under a literal reading. Phase 3 prose ("Node C runs Ch3 §3.3 to find its own new parent") implies deputies are exempt but §3.2's rule says "every orphan".
- Fix: one sentence: deputies are excluded from the partition and the surplus test, and k in both formulas is k − E.

### F7 (Medium, impl) The reference implementation has no ROSTER / Deputy election and no fast path
- `grep -ri deputy\|roster src/crates/bs-core/src` hits only `params.rs`. `lose_parent` (node.rs 1888) calls `begin_probing`: a full PROBE round (`probe_timeout` 200 ms), then sequential NEIGHBOR with a 600 ms per-candidate deadline; no 0-RTT ticket path. 20 ms repair ≈ 260 (tau_evict with RTTVAR = SRTT/2) + 200 + 40 ≈ **500 ms** vs the spec's 280. The probe round does give the impl what the spec's fast path lacks (TreeState, depth, fresh K_avail). `src/ARCHITECTURE.md` line 247 lists the roster as milestone M3.

### Already known (not re-filed)
- Gap-repair PULLs are governed by the TFT choke and the 30 s optimistic cooldown — **ISSUE-066**.
- Base-layer freeze for the starved fraction until the fold at the M = 4 minimax rung — accepted transient, Ch1 §1.2.4 §4.2.1 and **SOLUTION-058**.
- Deputy re-attach deepening the branch past D_max and the DEPTH drain unwinding it — accepted, Ch1 §1.2.2 *Depth Propagation*.
- Fresh connection uses the 200 ms floor until the estimator has samples — SOLUTION-022 residual.

## 7. Spec silences assumed through

- S1 Handshake timeout to a dead passive-set candidate: assumed 600 ms (impl); QUIC default ~10 s changes R9/R11 (F4).
- S2 An ACCEPTED that arrives after tau_deputy fired: assumed the orphan DISCONNECT(TreeID, CHOKE)s the deputy; the spec says nothing (F1).
- S3 Whether deputies run the partition/surplus hash: assumed exempt (F6).
- S4 A Deputy's ACCEPTED.HopDepth while it has no parent: assumed its stale pre-loss depth; corrected by BLOCK_PROOF after re-attach.
- S5 Whether a relay UNPARENTED in m may ACCEPT NEIGHBOR(m): the admission rule says yes (F3); assumed yes.
- S6 Unchoker tie-break when every neighbour's PullRate is 0: the pseudocode unchokes the top RegularSlots regardless, the rule box requires >= Threshold_min; assumed the rule box (choked). This decides how many slots an orphan has for F2.
- S7 Which pool candidates the fast path dials concurrently: §3.3.1 says tiers overlap, §3.3.2 is a sequential loop; assumed sequential within tier 1.
- S8 RTTVAR behind the tau_evict table: assumed 5 / 10 / 10 ms to match App B (F5).
- S9 The fast path picks by R and K_avail with no depth term; assumed the pick may land at any depth <= D_max − 2 and the REJECTED_DEPTH rule bounds it.
- S10 Heartbeat cadence in steady state: pacing over <= 125 ms leaves >= 125 ms idle per chunk > tau_ping, so a heartbeat fires every chunk on every edge; assumed harmless (0.1 % of uplink).
- S11 "Advertised K_avail" of passive-set entries: Peer Records carry no K_avail or R (Ch2 §2.3.3); assumed the values come from the most recent PROBE_RESPONSE or gossip the peer happens to hold, else unknown.
