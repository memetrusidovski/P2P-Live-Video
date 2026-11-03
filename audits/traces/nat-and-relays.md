# Trace: nat-and-relays — NAT traversal and emergent relays

Walker scenario `nat-and-relays`. Three joins at RTT 80 ms (one-way 40 ms), then the Ch8 §8.3 Scenario C arithmetic. Requirements checked: R4, R7, R11 (required); R2, R5, R6, R8, R10 (touched).

## Scenario and initial state

Constants (protocol/appendix_b_parameters.md unless noted):

| Constant | Value | Source |
|---|---|---|
| RTT | 80 ms (40 ms one-way) | scenario; the spec's reference RTT |
| τ_sched | 100 ms | B.1.1 |
| PROBE timeout | 200 ms per candidate | Ch1 §1.2.2 pseudocode line 130 |
| Retry backoff | ≥ 1.0 s (one segment) | Ch1 §1.1.5 §5.3 item 2 |
| τ_nat | 25 s | B.1.1; Ch2 §2.3.2 step 5 |
| Punch validity | 1 s | B.1.1; App D §D.4.14 |
| τ_evict at 80 ms | 320 ms | B.1.1 |
| W_pull at 80 ms | 1.5 s (floor) | B.1.2 |
| Ω | 1.15 | B.1.2 |
| r_pull | 0.10 | B.1.2 |
| Bridge share | ½ of (1 − r_pull)·u | B.1.2; Ch6 §6.3.1 |
| RELAY_CAPABLE | PUBLIC, u ≥ 20 Mbps, uptime ≥ 30 min, drop ≤ 0.5 % | Ch6 §6.3.1 |
| R_src | 3·b₀·Ω = 3 × 1.5 × 1.15 = 5.175 Mbps | Ch1 §1.1.5 §5.4 (the spec's own ≈ 5.2) |
| Ingress K_R(m) | ⌊((1 − r_pull)u_R − R_src/n_ingress)/(M·B_m·Ω)⌋ ≥ 2 every tree | Ch6 §6.3.3 |
| C₂ tier | 10 bits at N = 5,000; leaf rung −2 | Ch2 §2.2.1 |

Forest for walks (a) and (b): N ≈ 5,000 (s = 0), M = 6 reference mapping (T₁–T₄ at 0.75 Mbps: L₀ on T₁,T₂, L₁ on T₃,T₄; T₅,T₆ at 1.5 Mbps L₂). B_mΩ = 0.8625 Mbps for a 0.75 tree.

Forest for walk (c): N = 2 → 5, M = 2 (3.0, 3.0), B = 6 Mbps, ΩB = 6.9 Mbps, M·B_m·Ω = 6.9.

Nodes:

- **V_a**: viewer behind a port-restricted cone router. Local 192.168.1.10:4000, external 203.0.113.5:61000. Upload 10 Mbps.
- **V_b**: viewer behind CGNAT 198.51.100.7 (per-destination port mapping). Upload 8 Mbps.
- **G1**: a guardian of K_s that answered V_a's and V_b's `GET_PEERS` (the referrer).
- **P1, P3, P4**: `CONE` relays of T₁ (port-restricted). **P2, P5**: `PUBLIC` relays of T₁.
- **R**: `RELAY_CAPABLE` node, PUBLIC, u_R = 20 Mbps, uptime 45 min, assigned T₄ with 20 children.
- **B**: a relay of T₁ at depth 3 with a free slot.
- **S**: broadcaster behind CGNAT, u_S = 15 Mbps. **V1**: first viewer, PUBLIC, 20 Mbps (25 Mbps in the variant), process uptime 45 min. **V2**: CONE viewer, 8 Mbps. **V5**: second PUBLIC 20 Mbps viewer.

Two readings of the punch procedure are carried through the walk, because the spec supports both:

- **Reading A (literal pseudocode)**: Ch1 §1.2.2 lines 127–130 send `PUNCH_REQUEST` and then immediately `send_udp_probe(timeout_ms=200)`. No wait, no re-probe.
- **Reading B (implementation)**: `src/crates/bs-core/src/node/nat.rs:469` `reprobe_after_punch` re-sends the `PROBE` when a `PING` arrives from a candidate under probe.

## Tick table

Columns: t_ms | node | rule (file §) | computation | state after.

### Walk (a): cone-NAT viewer V_a

| t_ms | node | rule (file §) | computation | state after |
|---|---|---|---|---|
| a1: 0 | V_a | Ch2 §2.1.3 step 2 (`3_bootstrap_sequence.md`) | `PING` to seeds S1, S2 (different operators). Frame 4 + 152 + 32 = 188 B each. | BOOTSTRAP; 2 PINGs in flight |
| a2: 80 | V_a | Ch2 §2.1.3 table; App D §D.4.1 | PONG(S1) reflects 203.0.113.5:61000; PONG(S2) reflects 203.0.113.5:61000. Equal to each other, ≠ local → **CONE**. Dynamic puzzle vs 203.0.113.5 at tier(5,000) = 10 → 2¹⁰ = 1,024 expected hashes ≈ 1 ms. | Reachability = CONE; class RELAY permitted (Ch1 §1.2.5 table row 2) |
| a3: 80–240 | V_a | Ch2 §2.1.3 steps 3–5 | Self-lookup + `FIND_VALUE(K_s)`: 2 hops × 80 ms = 160 ms (best case; Ch3 §3.3.1 tier 4 says a fresh DHT walk is 1–3 s at the worst end). | DISCOVERY; routing table populated |
| a4: 240 | V_a → G1..G20 | Ch2 §2.3.2 steps 2–4; §2.3.3 `REGISTER_PEER` | s = max(0, ⌈log₂(5,000/10⁴)⌉) = 0 → registers. Class RELAY, Reachability CONE, AssignedTrees = bit 3 (rendezvous a(V_a) = T₃), RELAY_CAPABLE = 0 (PUBLIC required). Frame 4 + 32 + 152 + 8 + 64 = 260 B × 20 = 5.2 KB. Guardian binds the **observed** 203.0.113.5:61000, verifies PoW against it. τ_nat timer: next keepalive at 240 + 25,000 = **25,240**. Same tick: `GET_PEERS(WantedTrees = 0x3F)` to G1. | registered; keepalive armed |
| a5: 320 | G1 → V_a | Ch2 §2.3.3 `GET_PEERS` response | Count = 20, ⌈20/6⌉ = 4 per tree, 42 B each = 840 B + Stream Record 238 B + 36 B header ≈ 1,114 B. T₁ records: P1 (CONE, h = 2), P2 (PUBLIC, h = 3), two more. V_a stores referrer(P1) = G1. θ_join = max(1, min(4, 4,999)) = 4. | JOINING |
| a6: 320–480 | V_a | Ch1 §1.3.1; App D §D.4.2 | `JOIN` (plain UDP, 4 + 152 + 32 + 4 = 192 B) to 4 PUBLIC membership records in parallel → QUIC upgrade: 2 RTT = 160 ms. |A| = 4 at t = 480. Buffer anchored at X = `LiveEdgeSegmentId`. (Assumption: the 4 membership targets are PUBLIC; a CONE one would need the same punch as a4–a13, which §D.4.14 writes only for the tree-join `PROBE`.) | CONNECTING |
| a7: 480 | V_a | Ch1 §1.2.2 §2.2 step 2, lines 127–130; App D §D.4.14 | P1 is CONE → `PUNCH_REQUEST` request form (4 + 152 + 32 + 32 = 220 B) to G1. **Immediately** `PROBE` (4 + 152 + 32 + 1 = 189 B) to P1 and to P2. Probe deadline = 480 + 200 = **680**. | probing T₁ |
| a8: 520 | P1's NAT; P2 | RFC 4787 port-restricted filtering (Ch6 §6.1.1); App D §D.4.7 | V_a's PROBE reaches P1's NAT. P1 has sent nothing to 203.0.113.5:61000 → **dropped**. P2 (PUBLIC) answers `PROBE_RESPONSE(K_avail = 6, R = 1.0, HopCount = 3, TreeState = SERVING, LiveEdge = X)`. | P1 probe lost |
| a9: 520 | G1 | App D §D.4.14 forward form; Ch2 §2.3.2 step 5 | Verifies V_a's validation block against observed 203.0.113.5. Builds Requester PeerRecord with **observed** address. Forwards (4 + 152 + 42 + 32 = 230 B) to P1 through the mapping P1's last keepalive opened (≤ 25 s old; NAT lifetime assumed ≥ 30 s, SOLUTION-051). | forward in flight |
| a10: 560 | P1 | App D §D.4.14 "The target sends a PING to the requester's address" | `PING` (188 B) to 203.0.113.5:61000. P1's NAT now holds an outbound entry to V_a's ip:port. | P1 mapping open toward V_a |
| a11: 600 | V_a | Ch1 §1.2.2 §2.1 score; App D §D.4.14 | PROBE_RESPONSE(P2): Score = 12·√6·1.0 − 80 − 20·(e^1.5 − 1) = 29.39 − 80 − 69.63 = **−120.2**. PING from P1 arrives (V_a's PRC NAT passes it: V_a sent to P1's ip:port at a7). V_a answers PONG. **Spec rule for a second PROBE: none.** "the requester's PROBE that follows within 1 s then passes" — the only PROBE sent was dropped at a8. Reading A: nothing. Reading B: re-send PROBE now. | Reading A: waiting on timer; Reading B: PROBE #2 to P1 |
| a12: 640 | P1 | App D §D.4.7 | Reading B only: PROBE #2 passes the open mapping. `PROBE_RESPONSE(K_avail = 4, HopCount = 2, SERVING)`. | — |
| a13: 680 | V_a | Ch1 §1.2.2 pseudocode TIMEOUT comment; App D §D.4.14 last sentence | **Reading A**: P1 times out → "retries once through a gossip referrer if it has one" (none on a fresh join) "and otherwise marks the record unpunchable locally for one registration period" (90 s or 180 s — unstated). Candidates = [P2]. **Reading B**: Score(P1) = 12·√4 − 80 − 20·(e^1.0 − 1) = 24 − 80 − 34.37 = **−90.4** > −120.2 → P1 best. Punch cost measured: 480 → 680 = 200 ms = **2.5 RTT** (request 1 RTT to reach P1 via G1, ½ RTT for P1's PING, 1 RTT for the PROBE), against "+1 RTT" (Ch1 §1.2.2 line 128; §1.2.5 table "one extra RTT"). | Reading A: P1 unpunchable; Reading B: P1 chosen |
| a14: 680–840 | V_a → P1 (B) / P2 (A) | App D §D.4.3, §D.4.3b, §D.4.4; Ch1 §1.2.2 Rank Admission case 1 | QUIC handshake 1 RTT + `NEIGHBOR(TreeID = 1, NodeClass = RELAY, AssignedTrees = bit 3)` 1 RTT (no RANK_PROOF, r = 0). Free slot → `ACCEPTED(HopDepth = 2)` at 840 (within τ_sched). V_a depth(T₁) = 3. T₂..T₆ joined in parallel to PUBLIC parents on the same timeline. | parents in all 6 trees at ≈ 840 |
| a15: 840–1,300 | V_a | Ch4 §4.3.1 join sequence steps 3–5 | `MANIFEST_REQUEST(X, 0xFF)` + `0xFD` → manifests + `STREAM_DESCRIPTOR` at 920 → ACTIVE. PUSH from X+1; PULL of segment X from active set: 2 PULL RTT = 160 ms → segment X complete ≈ 1,100–1,300. **SJL ≈ 1.1–1.3 s** (Reading B, or Reading A with the PUBLIC P2). Reading A with a T₁ sample of CONE-only candidates: every candidate TIMEOUT at 680 → "discovery failure (re-query), not a shed signal" (Ch1 §1.1.5 §5.3 item 1) → backoff ≥ 1 s → round 2 at ≈ 1,700 with a fresh sample fails the same way → T₁ (L₀) never joined → never ACTIVE (Ch1 §1.3.1 "does not enter ACTIVE at all"). | ACTIVE at ≈ 920 (Reading B) |
| a16: 25,240 | V_a → G1..G20 | Ch2 §2.3.2 step 5; App D §D.4.14 | τ_nat keepalive: 20 × 188 B = 3.76 KB per 25 s = **1.2 kbps**. Gap 25 s < assumed 30 s mapping lifetime. Against V_a's 0.75 Mbps push per child the keepalive is 0.16 % of one slot's egress. | guardian mappings alive; V_a punchable via G1..G20 |
| a17: 50,240, 75,240, … | V_a | same | Repeats every 25 s for the registration's life. A NAT with a sub-25 s timeout defeats it (accepted residual, SOLUTION-051). | steady |

### Walk (b): symmetric-NAT viewer V_b, bridged

| t_ms | node | rule (file §) | computation | state after |
|---|---|---|---|---|
| b1: 0 | V_b | Ch2 §2.1.3 step 2 | PING to S1, S2. CGNAT allocates :30001 toward S1 and :30077 toward S2. | BOOTSTRAP |
| b2: 80 | V_b | Ch2 §2.1.3 table; Ch1 §1.2.5 table row 3 | Reflections 198.51.100.7:30001 ≠ 198.51.100.7:30077 → **SYMMETRIC** → may not declare RELAY → class **LEAF (0x01)**. (The scenario text says `LEAF_PRIVATE`; the spec reserves 0x02 for onion-routed peers, Ch7 §7.3.2. The bridge mechanics are identical.) PoW tier 10 − δ_class 2 = 8 → 256 hashes. DHT client mode (Ch2 §2.3.2 rule 1). | Reachability SYMMETRIC, class LEAF |
| b3: 240 | V_b | Ch2 §2.3.2 | `REGISTER_PEER(LEAF, SYMMETRIC, AssignedTrees 0x00)`; `GET_PEERS(WantedTrees 0x3F)`. CGNAT allocates :30210 toward G1; G1 records 198.51.100.7:30210. | registered (client mode) |
| b4: 320–480 | V_b | Ch1 §1.3.1 | T₁ records: P2 (PUBLIC, K_avail = 0, SERVING), P1, P3, P4 (CONE). JOIN to 4 PUBLIC membership peers → CONNECTING at 480. | CONNECTING |
| b5: 480 | V_b | Ch1 §1.2.2 §2.2 line 114 filter (`reachability != SYMMETRIC` keeps all four) and lines 127–130 | `PUNCH_REQUEST` × 3 to G1 (targets P1, P3, P4); `PROBE` × 4 immediately. Probe deadline 680. | probing T₁ round 1 |
| b6: 520 | P1/P3/P4 NATs; P2; G1 | Ch6 §6.1.1; App D §D.4.14 | PROBEs at the three CONE NATs: no outbound entry toward 198.51.100.7 yet (the forward has not arrived) → **dropped**. P2 answers K_avail = 0. G1 forwards three punch requests carrying Requester addr = 198.51.100.7:**30210**. | — |
| b7: 560 | P1, P3, P4 | App D §D.4.14 | Each sends `PING` to 198.51.100.7:30210. Their NATs open entries toward 198.51.100.7 (RC: IP only; PRC: ip:30210). | — |
| b8: 600 | V_b's CGNAT; V_b | symmetric NAT semantics (Ch6 §6.1.1 "mapping is per-destination"); Ch1 §1.2.2 table | :30210 is bound to destination G1 only → the three PINGs are **dropped**; V_b never learns the punch landed. PROBE_RESPONSE(P2) K_avail = 0, SERVING → "saturated — counts toward the shed threshold". | — |
| b9: 680 | V_b | Ch1 §1.2.2 TIMEOUT; Ch1 §1.1.5 §5.3 item 1; Ch1 §1.3.1 | P1, P3, P4 TIMEOUT. Reading B cannot fire (no PING arrived); Reading A already failed. The Ch6 §6.1.1 cell "SYMMETRIC → RC: Hole Punch 95 %" needs a PROBE from a *new* V_b mapping arriving at P_x's NAT **after** t = 560; an RC NAT would pass it (IP-only filter). No rule times such a PROBE, so **0 of 3** under both readings. Round 1 = 1 saturated + 3 timeouts: §5.3 defines "every candidate refused" (failed round) and "every candidate timed out" (discovery failure) — the mixed round is undefined. T₁ carries L₀ → never shed; V_b "keeps retrying" and is not ACTIVE. Retry = re-query (`GET_PEERS(StarvedTrees bit 1)`), backoff ≥ 1.0 s. | T₁ unparented; starved bit set |
| b10: 1,680–1,880 | V_b | same | Round 2 with a fresh sample (P5 PUBLIC saturated; P6–P8 CONE): identical outcome at 1,880. **When does V_b ask for a bridge?** Ch6 §6.3.1: "A peer needing a relay issues GET_PEERS with ReqFlags.WANT_RELAY_CAPABLE"; Ch1 §1.2.5: "Where it cannot reach a CONE parent it needs … an emergent relay bridges it." Neither defines "needing". Implementation: `node.rs:983` asks after `silent_rounds ≥ bridge_after_failed_rounds = 2`, but `node.rs:975-979` resets `silent_rounds` whenever any candidate answered — P2/P5 answered → **the implementation never asks** in this walk. Walk assumption from here: V_b asks after 2 rounds, at 1,880. | bridge requested for T₁ |
| b11: 1,880–1,960 | V_b ↔ G1 | Ch2 §2.3.3 `ReqFlags.WANT_RELAY_CAPABLE`; §2.3.2 (RELAY_CAPABLE sample at max(0, s − 3) = 0) | Response: up to 20 RELAY_CAPABLE records. V_b picks R (PUBLIC, 20 Mbps, 45 min, 0.2 % drop; assigned T₄, 20 children, U_bridge = 0; K_R(4) = ⌊18/(1 × 0.75 × 1.15)⌋ = ⌊20.87⌋ = 20). "Prefers distinct relays per tree where the sample offers them." | R chosen |
| b12: 1,960–2,120 | V_b → R | Ch6 §6.1.1 row SYMMETRIC × col PUBLIC = Direct; App D §D.2 (RELAY_PROPOSAL rides QUIC) | JOIN + QUIC handshake 2 RTT = 160 ms. R records V_b's observed address and class LEAF. | session open |
| b13: 2,120 | V_b → R | Ch6 §6.3.3 RELAY_PROPOSAL | `RELAY_PROPOSAL(Target NodeID = 0 [let R choose], Port 0, TreeID = 1, BindFlags = 0)`: 4 + 32 + 2 + 1 + 1 = 40 B. Arrives 2,160. | proposal in flight |
| b14: 2,160 | R | Ch6 §6.3.1 *Cost accounting*; Ch1 §1.2.1 *Two Budgets* | B₁Ω = 0.75 × 1.15 = **0.8625 Mbps**. Cap = ½ × 0.9 × 20 = **9.0 Mbps**. 0 + 0.8625 ≤ 9.0 ✓ (room for ⌊9/0.8625⌋ = 10 bridged 0.75 Mbps trees). Second condition: R must obtain a slot at some parent for T₁. R is not assigned T₁ → runs the Ch1 §1.2.2 join for T₁: PROBE round 200 ms (R is PUBLIC: Reading B punches work since the target's PING reaches R) → `NEIGHBOR` to B (2 RTT) → `ACCEPTED(HopDepth = 3)` at ≈ **2,520**. R depth(T₁) = 4. R does **not** set bit 1 in AssignedTrees. K_R(4) recomputed: ⌊(18 − 0.8625)/0.8625⌋ = ⌊19.87⌋ = **19** → one T₄ child released by `DRAIN_NOTICE(CAPACITY)` (Ch1 §1.2.1 *Slot Count*: a falling K_v is honoured by draining). | R child of B in T₁ (WARMING); R's T₄ fan-out 20 → 19 |
| b15: 2,520–2,560 | R → V_b | Ch6 §6.3.3 "RELAY_BIND is sent by the relay to A once it holds a slot at B" | `RELAY_BIND(Client A = V_b, Client B = B, TreeID = 1, BindFlags = 0)`: 4 + 32 + 32 + 1 + 1 + 2 = 72 B. R has a slot but no verified segment in T₁ yet (WARMING); the spec gates BIND on the slot, not on warm-up. | V_b holds BIND |
| b16: 2,560–2,640 | V_b → R | Ch6 §6.3.1 "A is then an ordinary child of R in T_m"; App D §D.4.3, §D.4.4 | `NEIGHBOR(TreeID = 1, LEAF, 0x00)` over the open session, 1 RTT. R admits against the bridge budget (R has no K_R(1); the child is U_bridge). `ACCEPTED(HopDepth = 4)` at 2,640. V_b depth(T₁) = **5 = h_B + 2** ✓. If V_b is bridged through R in T₂ as well, the two trees are marked correlated (Ch1 §1.2.1 Distinct-Parent Rule). | V_b parent in T₁ = R |
| b17: 2,640–3,650 | R; V_b | Ch1 §1.2.2 Warm-Up Gating; Ch4 §4.1.2 decode → verify → re-encode | R pushes nothing until its first complete segment in T₁ verifies: ≤ 1 s + 250 ms chunk after attach → R SERVING at ≈ 3,600; V_b's first T₁ blocks ≈ **3,650**. V_b is ACTIVE only when both L₀ trees (T₁, T₂) have parents; with T₂ bridged in parallel the timeline is the same. First complete verified segment at V_b ≈ **4,700 ms** → SJL ≈ 4.7 s (R2 target 1.5 s). The implementation's own gate for this population sets `sjl_mean_max_s = 8.0` (`src/scenarios/nat_symmetric.toml:49`). | V_b ACTIVE ≈ 3,650; first GOP ≈ 4,700 |
| b18: each segment from 4,700 | V_b → R; R → B | Ch5 §5.2.1 `RxFlags.RELAYED`; §5.2.2 weights and caps; B.1.2 receipt deadline | V_b issues `PROOF_OF_UPLOAD(TreeID 1, RELAYED)`: popcount ≈ 6 blocks × 16 KB = 94 KB. Credit to R = 3 × min(94 KB, cap 1.5 × 750 kbps × 1 s = 140.6 KB) = **281 KB** per segment vs 94 KB for an ordinary child. R issues B an ordinary receipt (94 KB, w = 1). Deadline: receipt for k before the first block of k + 2. | receipts flowing both edges |
| b19: steady state, R at cap | R | Ch6 §6.3.2; Ch1 §1.2.1 K_v with U_bridge removed; App D §D.4.17 | 10 bridged children (U_bridge = 8.625) + K_R(4) = ⌊(18 − 8.625)/0.8625⌋ = ⌊10.87⌋ = 10 own children. Θ^rate = (10 × 3 + 10 × 1) × 94 KB/s = 3,750 KB/s vs pure relaying 20 × 94 = 1,875 → **2.0×** ✓ ("never more"). Verifier side: `RANK_PROOF` pair checks cover signature, window, per-receipt cap — nothing checks `RELAYED`. A prover whose 20 ordinary children set the flag proves 3 × 1,875 = 5,625 KB/s → **3.0×**, past the stated bound, and every check passes. | — |
| b20: T_r (R dies) | V_b | Ch3 §3.3.1; Ch1 §1.3.1 CHURN_REPAIR(T₁); Ch4 §4.3.1 W_pull | τ_evict = 320 ms → V_b evicts R for T₁ at T_r + 320. Pool 𝒫₁ (RELAY, bit 1, ≠ SYMMETRIC) holds CONE relays V_b cannot reach (b9) and the PUBLIC ones that were saturated. If a PUBLIC standby has a slot: ordinary ≈ 2 RTT repair ✓. Otherwise a full bridge cycle: GET_PEERS 80 + session to R′ 160 + R′ probe round 200 + R′ NEIGHBOR 160 + BIND 40 + NEIGHBOR 80 = **720 ms**, plus R′ warm-up ≤ 1,000 ms if R′ did not already hold T₁ → **1.0–2.0 s after eviction, 1.3–2.4 s after loss**. W_pull = 1.5 s at 80 ms RTT → the worst end misses the PULL zone in an L₀ tree → freeze. | repaired at T_r + 1.3 s (best) / + 2.4 s (worst) |

### Walk (c): broadcaster S behind CGNAT binding ingress relays

| t_ms | node | rule (file §) | computation | state after |
|---|---|---|---|---|
| c1: 0–80 | S | Ch2 §2.1.3; Ch1 §1.2.5; Ch2 §2.3.2 *The Source Is Discoverable* | Reflections differ → SYMMETRIC. Ch1 §1.2.5: SYMMETRIC "may not declare RELAY … defaults to LEAF". Ch2 §2.3.2: "The source registers under K_s as an ordinary RELAY-class peer with Flags.SOURCE". Both rules cannot hold; under either, the tree-join filter (reachability ≠ SYMMETRIC) excludes S from candidate lists. Walk: S registers with Flags.SOURCE, Reachability SYMMETRIC. | S registered, unreachable |
| c2: 240–320 | S ↔ guardians | Ch2 §2.3.2 `STORE_RECORD`; Ch6 §6.3.3 step 1 | `STORE_RECORD` (NumTrees 2; T₁ = L₀+L₁ 3,000 kbps, T₂ = L₂ 3,000 kbps; LiveEdge 1). `GET_PEERS(WANT_RELAY_CAPABLE, 0x03)` → **Count = 0** (no viewer yet). "A source that can bind no ingress relay cannot broadcast." Re-query interval: unstated (impl `nat.rs:181`: 2 s). | S waiting |
| c3: 1,000 | V1 | Ch2 §2.1.3; Ch6 §6.3.1 criteria; Ch1 §1.2.2 lines 111–121 | V1 bootstraps PUBLIC (reflections = local). RELAY_CAPABLE needs "rolling uptime ≥ 30 minutes": clock undefined. Stream-membership reading: V1 has 0 min and cannot join (no reachable root) → it can never reach 30 min → S never binds → **deadlock**. Process-uptime reading (impl `nat.rs:103-111`, `started_at`): capable. Walk continues on the process-uptime reading. V1's own join: `GET_PEERS(0x03)` returns S only; filtered (SYMMETRIC) → candidate_list empty → coverage-grant branch: no second-ranked relay; `candidate_list.append(source_record)` → S is SYMMETRIC, "never probed" → round yields nothing. | V1 in CONNECTING, registered RELAY_CAPABLE |
| c4: 2,240–2,480 | S → V1 | Ch6 §6.3.3 step 1; Ch6 §6.1.1 (SYMMETRIC → PUBLIC direct) | Re-query returns V1. QUIC session 2 RTT → 2,480. `RELAY_PROPOSAL(Target 0, TreeID 0, BindFlags = SOURCE_INGRESS)` "signed with SK_Source inside the QUIC session so the relay can verify it is the publisher". The RELAY_PROPOSAL byte diagram (Ch6 §6.3.3) has **no signature field**. The session identity is NodeID = Blake3(PK_node ∥ N_static) (Ch2 §2.2.4); nothing states PK_node = PK_Publisher. | proposal at V1 (2,520) |
| c5: 2,520 | V1 | Ch6 §6.3.3 *An ingress relay is a root*; B.1.2 *Ingress relay* | Authentication: either (i) compare session PK_node to the Stream Record's PublisherPubKey — valid only if the source's node key is the publisher key, which no rule requires — or (ii) trust the guardian-served SOURCE flag — self-declared, unsigned, copied verbatim (Ch2 §2.3.3). Implementation does (ii): `membership.rs:108-113` sets `source_record` from any record carrying `flags.source`/`source_ingress`, including `FORWARD_JOIN` origins (`membership.rs:439`), and `nat.rs:258-261` accepts on `source_record.node_id == peer && flags.source`. Capacity: K_R(m) = ⌊(0.9 × 20 − R_src/n)/(2 × 3.0 × 1.15)⌋ = ⌊(18 − 5.175/n)/6.9⌋. **n = 1** (the only configuration N = 2 allows): ⌊1.859⌋ = **1 < 2 → REJECTED**. n = 2 (the spec's "u_R ≥ 18.2 Mbps at M = 2" is derived with 2.6 = R_src/2): ⌊2.234⌋ = 2 → accepted. Floor for a sole ingress relay: 0.9u ≥ 13.8 + 5.175 → **u ≥ 21.1 Mbps** (M = 6: 0.9u ≥ 20.7 + 5.175 → 28.75 Mbps). Under the reading "three copies of L₀, however it is currently striped" a copy at M = 2 is a 3.0 Mbps tree slot: R_src = 3 × 3.0 × 1.15 = 10.35 → n = 2 gives ⌊(18 − 5.175)/6.9⌋ = 1 → rejected even with two. Walk continues with u_V1 = 25 Mbps: ⌊(22.5 − 5.175)/6.9⌋ = ⌊2.51⌋ = **2** per tree ✓. | V1 accepts (25 Mbps) |
| c6: 2,560–2,600 | V1 → S; V1 → guardians | Ch6 §6.3.3 step 2; Ch2 §2.3.3 ("must re-register whenever any of them changes") | `RELAY_BIND(A = S, B = V1, TreeID 0, SOURCE_INGRESS)`. V1 re-registers: class RELAY, Flags SOURCE_INGRESS, AssignedTrees 0x03, exempt from sampling. Rendezvous assignment set aside; t_v = M = 2. Parent in both trees = S at depth 0; V1 advertises HopCount 0. | V1 is hop 0 |
| c7: 2,600 | S | Ch6 §6.3.3 step 2; Ch1 §1.1.5 §5.4 "delegates this pool to its ingress relays" | S pushes both 3.0 Mbps slices to V1: ΩB = **6.9 Mbps** of u_S = 15 (room for a second: 13.8 ≤ 15 ✓). S holds back no reserve of its own. V1 sets aside R_src/n = 5.175 Mbps. Trigger for spending it: §5.4 "serves base-layer slots directly when it observes starvation signals: GET_PEERS re-queries whose StarvedTrees … returned to the publisher on every Stream Record store" — the `STORE_RECORD_ACK` goes to **S**; no frame carries the count or an "open k slots" instruction from S to V1. | 5.175 Mbps idle at V1, trigger unreachable |
| c8: 2,600–3,600 | V1 | Ch1 §1.2.2 Warm-Up Gating | WARMING in both trees until one full segment verifies from S → SERVING ≈ 3,600. V1 is also the only viewer and plays at depth 0 → N = 2 streams; SJL(V1) ≈ 3.6 s, bounded by the ingress bind, not by the join. | forest: S → V1 |
| c9: 10,000 | V2 (CONE, 8 Mbps) | Ch1 §1.2.2 §2.1; Ch6 §6.3.3 step 3 ("joiners treat a SOURCE_INGRESS record exactly as they treat the source … depth 0") | `GET_PEERS(0x03)` → V1 (RELAY, SOURCE_INGRESS, 0x03, PUBLIC, K_avail = 2 per tree, HopCount 0); S filtered. Score(V1) = 12·√2 − 80 − 0 = **−63.0**. `NEIGHBOR(T₁)`, `NEIGHBOR(T₂)` → `ACCEPTED(HopDepth 0)` → V2 depth 1 in both; only one parent exists → trees marked correlated (Distinct-Parent Rule). V2's own slots: K = ⌊7.2/(1 × 3.0 × 1.15)⌋ = ⌊2.09⌋ = 2 in its assigned tree. | N = 3 |
| c10: 10,000–30,000 | V3, V4 | same | V3 fills V1's second slot per tree; V4 finds V1 saturated (K_avail 0, SERVING) → joins V2/V3 after their warm-up at depth 2. Source egress stays **6.9 Mbps** (R8) regardless of N. | N = 5, depth ≤ 2 |
| c11: 30,000+ | S, V5 | Ch6 §6.3.3 ("Two ingress relays are recommended") | V5 (PUBLIC, 20 Mbps) registers RELAY_CAPABLE. S binds it with n = 2: R_src/2 = 2.5875; K = ⌊(18 − 2.5875)/6.9⌋ = 2 per tree ✓. S egress 13.8 Mbps. V1's share also falls to 2.5875, raising K_V1(m) to ⌊(22.5 − 2.5875)/6.9⌋ = 2 (unchanged at 25 Mbps; at 30 Mbps 2 → 3). **No frame tells V1 that n changed**; V1 keeps 5.175 idle. | two roots |
| c12: T_s (S dies, no STREAM_END) | V1, V5 | Ch3 §3.3.1; Ch1 §1.3.1 | S silent → τ_evict 320 ms → V1 evicts S in both trees → no candidates (V1 is the root) → "every parent lost → DISCOVERY". V1 remains registered `SOURCE_INGRESS` (no rule unbinds; re-registration is triggered only by class/tree/flag changes V1 does not make) → joiners keep scoring V1 at depth 0 for up to 135 s and receive nothing; only the reliability term corrects. | stale root for ≤ 135 s |

### Walk (d): Ch8 §8.3 Scenario C — 75 % behind symmetric NAT, per 100 private leaves (N = 133)

| t_ms | node | rule (file §) | computation | state after |
|---|---|---|---|---|
| d1: steady | publisher | Ch1 §1.2.5 §5.1 leaf-fraction bound; §1.2.4 fold mappings | ℓ = 0.75. M = 6 (2,2,2): ū ≥ M·B₀·Ω/((1 − r_pull)(1 − ℓ)) = 6 × 0.75 × 1.15/(0.9 × 0.25) = **23 Mbps** mean relay upload — the 10 Mbps reference fails. Folded to L₀ alone (B₀ = 0.25): 6 × 0.25 × 1.15/0.225 = **7.67 Mbps** → 10 Mbps relays carry 480p only. Resolution lost, not playback (R5 shape). | 480p swarm |
| d2: steady | relays | Ch1 §1.2.1 K_v; Ch2 §2.3.2 "0.6 (the CONE share)" | Reachable nodes 33 of 133: 20 CONE, 13 PUBLIC. 10 Mbps relay at B_m = 0.25: K = ⌊9/0.2875⌋ = **31**. Relays per tree 33/6 = 5.5 → 170 slots vs 132 needed ✓. PUBLIC relays per tree 13/6 = 2.2 → **67 PUBLIC slots per tree**; symmetric demand **100 per tree**. If PUBLIC slots go to symmetric leaves first: 33 per tree must use CONE parents; if the 33 reachable peers also take PUBLIC slots (no rule steers them away): 66 per tree. | 33–66 symmetric leaves per tree on CONE or bridge |
| d3: steady | symmetric leaves | Ch6 §6.1.1 matrix; walk (b) b6–b9 | Under the spec's procedure (b9) symmetric → CONE is 0 %: 33–66 per tree need a bridge → × 6 trees = **198–396 bridge edges** per 100 private leaves. Under the matrix with "most home routers are port-restricted" (10 %): 30–59 per tree → **178–356**. | — |
| d4: steady | RELAY_CAPABLE nodes | Ch6 §6.3.1 cost cap | Bridge cost 0.25 × 1.15 = 0.2875 Mbps; a 20 Mbps relay carries ⌊9/0.2875⌋ = 31 bridges (plus one upstream slot per bridged tree). Needed: 178–396/31 = **6–13 RELAY_CAPABLE nodes per 133 peers**, i.e. 46–100 % of the 13 PUBLIC nodes must be ≥ 20 Mbps with ≥ 30 min uptime, in a swarm whose reference mean upload is 10 Mbps. Supply if every PUBLIC node qualifies: 13 × 31 = 403 ≥ 396 — only just, with zero slack and zero other PUBLIC relay duty. Recruitment converges only in the all-PUBLIC-are-20-Mbps corner; the implementation's gate provisions 8 relays of 50–70 Mbps for 48 symmetric viewers (`nat_symmetric.toml:27-29`), i.e. 17 relay-capable nodes per 100 private leaves at 2.5–3.5× the floor. | does not converge at the reference upload |

Ticks walked: 44 (17 + 20 + 12 − overlaps counted once per row = 44 rows; 41 distinct time points).

## Requirement checks

| Req | Verdict | Evidence |
|---|---|---|
| R1 | not_applicable | Bridged depth = h_B + 2 (b16), inside D_max by the depth rule; Δ_buffer unchanged. Not exercised further. |
| R2 | fail (bridged symmetric viewer); pass (cone viewer, Reading B) | V_a: first verified GOP ≈ 1.1–1.3 s (a15). V_b: ≈ 4.7 s (b17) against 1.5 s; the implementation's own gate admits 8 s mean for this population. Under Reading A with CONE-only samples V_a never joins (a15). |
| R4 | fail | c3: RELAY_CAPABLE's 30 min uptime has no defined clock; on the stream-membership reading a NAT-blocked source at N = 2 can never bind. c5: at the 20 Mbps floor K_R = ⌊(18 − 5.175)/6.9⌋ = 1 < 2 for the first (sole) ingress relay; the spec's 18.2 Mbps figure assumes n_ingress = 2 already bound; floor is 21.1 Mbps (M = 2), 28.75 Mbps (M = 6). c1: SYMMETRIC source must register RELAY-class and may not. |
| R5 | fail at the worst end for bridged peers; pass otherwise | b20: repair of a lost bridge = 1.3–2.4 s after loss vs W_pull = 1.5 s at 80 ms RTT; an L₀ tree freezes at the worst end. Non-bridged repair is τ_evict + 2 RTT = 480 ms ✓. |
| R6 | pass | V_b (cannot upload inbound) receives L₀ via the bridge (b16–b17) and may earn PULL standing outbound (Ch1 §1.2.5). |
| R7 | pass, with the ingress caveat filed as a finding | a9/b6: forwarded punch carries the referrer-observed address; a4: PoW vs observed source; b14/b17: R decodes, verifies, re-encodes before pushing (Ch6 §6.3.1, Ch4 §4.1.2). c4–c5: the publisher's identity behind `SOURCE_INGRESS` is not verifiable from any frame; content stays protected by manifest signatures, so no unverified byte is forwarded. |
| R8 | pass | c7, c10, c11: source egress = n_ingress × 6.9 Mbps = 6.9 or 13.8 Mbps, independent of N. |
| R9 | undefined | Bridged-peer PSR is owed to Ch8 (SOLUTION-050 validation owed); the b20 worst end implies PSR > 0 for bridged L₀ on each relay loss. |
| R10 | pass | a16: τ_nat keepalive 1.2 kbps per CONE relay; punch 220 + 230 + 188 B ≈ 640 B per CONE candidate per round; RELAY_PROPOSAL/BIND 112 B per bridge — all ≪ 2 % of a 0.75 Mbps push slot. |
| R11 | A, B not_applicable; C fail | d1–d4: 178–396 bridge edges per 100 private leaves need 6–13 RELAY_CAPABLE 20 Mbps PUBLIC nodes per 133 peers — 46–100 % of all PUBLIC nodes — against a reference mean upload of 10 Mbps; and the spec's own punch procedure makes symmetric → CONE 0 % (b9), the regime Scenario C is about. |

## Candidate findings

Each survived the debate noted under it. Severity per AUDITOR-AGENT.md.

### F1 — High — The punch procedure sends the PROBE before the target's PING can open the mapping, and a SYMMETRIC requester can never learn when to re-send

- **Rests on:** App D §D.4.14: "The target sends a `PING` to the requester's address, opening its NAT mapping; the requester's `PROBE` that follows within 1 s then passes." Ch1 §1.2.2 lines 127–130: `send_punch_request(...)` immediately followed by `send_udp_probe(..., timeout_ms=200)`. Ch1 §1.2.5: "probed after a `PUNCH_REQUEST` rendezvous … one extra RTT."
- **With inputs** RTT 80 ms, requester V_a (CONE) or V_b (SYMMETRIC), target P1 (CONE) referred by guardian G1, **the spec produces**: PROBE at the target's NAT at t + 40 ms; the target's PING leaves at t + 80 ms (a8–a10, b6–b7). The PROBE is dropped at every RTT > 0. For a CONE requester the target's PING arrives at t + 120 ms, but no rule says to probe again (a11); for a SYMMETRIC requester the PING is dropped at its own NAT (b8) so no trigger ever exists.
- **Wrong because**: under the literal text every guardian-referred CONE parent is marked unpunchable on first contact (a13), and the Ch6 §6.1.1 cell "SYMMETRIC → RC: Hole Punch 95 %" is 0 % under the spec's own procedure, which turns every symmetric leaf facing CONE-only slots into a bridge customer (d3). The cost is 2.5 RTT (200 ms at 80 ms), not "+1 RTT".
- **Debate**: "follows within 1 s" could mean "wait for the PING". That rescues CONE requesters only if an implementer invents the re-probe, and does nothing for SYMMETRIC requesters. The implementation (`nat.rs:469`) invents the re-probe and still has no path for the symmetric case; its simulator replaces the mechanism with a configured coin (`bs-sim/src/network.rs:119`, `symmetric_cone_punch`). SOLUTION-032's "validation owed" item is the success *rate*; the ordering defect is not a tuning value.
- **Proposed fix**: define the requester's PROBE as sent on receipt of the target's PING (CONE/PUBLIC requester) and, for a SYMMETRIC requester, as a blind re-send ≥ 1 RTT_guardian + ½ RTT after the request; restate the cost as ≈ 2.5 RTT; touch Ch1 §1.2.2 pseudocode, §1.2.5 table, App D §D.4.14, Ch6 §6.1.1 note.
- already_known: no.

### F2 — Medium — No rule says when a NAT-blocked peer asks for a bridge, and a mixed round (saturated + timeouts) has no classification

- **Rests on:** Ch6 §6.3.1: "A peer needing a relay issues `GET_PEERS` with `ReqFlags.WANT_RELAY_CAPABLE`." Ch1 §1.2.5: "Where it cannot reach a `CONE` parent it needs … an emergent relay bridges it." Ch1 §1.1.5 §5.3 item 1 defines all-refused and all-timeout rounds only.
- **With inputs** V_b's round 1 (1 saturated PUBLIC + 3 CONE timeouts, b9) **the spec produces** no decision: not a failed round, not a discovery failure, no bridge trigger. T₁ carries L₀, so V_b is never ACTIVE while it loops (Ch1 §1.3.1).
- **Wrong because** two implementers diverge: one triggers on shed-style failed rounds (which exclude timeouts and so never fire for a peer whose only problem is reachability), one on consecutive silent rounds (the implementation, `bridge_after_failed_rounds = 2`), and the consuming rule — the base-layer floor for SYMMETRIC peers — depends on the value.
- **Debate**: "needing" is arguably obvious; but the only two signals the spec defines (refusal, timeout) each have a different consequence written for them, and neither names the bridge.
- **Proposed fix**: "A non-PUBLIC peer requests a bridge for T_m after k = 2 consecutive rounds in which no reachable candidate had a free slot (timeouts and saturated refusals both count)"; add k to Appendix B; touch Ch6 §6.3.1, Ch1 §1.1.5 §5.3.
- already_known: no.

### F3 — Medium — Ingress-relay cold start at N = 2 depends on the first viewer satisfying the 30-minute uptime criterion, whose clock is undefined

- **Rests on:** Ch6 §6.3.1: "High Stability: Rolling uptime ≥ 30 minutes". Ch6 §6.3.3: "A broadcaster on a home connection behind CGNAT — a primary use case at N = 2–10 … must … bind one or more ingress relays … A source that can bind no ingress relay cannot broadcast." Ch2 §2.3.2: RELAY_CAPABLE is a flag of a registration under K_s.
- **With inputs** N = 2, S SYMMETRIC, V1 PUBLIC 20 Mbps (c3) **the spec produces**: V1's uptime in this swarm is 0 min and cannot grow because there is no reachable root; under the stream-membership reading S never binds. Under the process-uptime reading (what the implementation measures, `nat.rs:103-111`) S binds only if the one viewer is also PUBLIC, ≥ 20 Mbps and ≥ 30 min old.
- **Wrong because** the stated primary use case cannot start at N = 2 under one reading and starts only for a specially provisioned viewer under the other; the drop-rate criterion (≤ 0.5 %) likewise has no measurement before any media flows.
- **Debate**: a NAT-blocked streamer can be told to bring a friend with a public 20 Mbps box; but the spec claims R4 for every mechanism, and the source already applies its own K_R(m) ≥ 2 gate, which makes the uptime/drop criteria redundant for ingress.
- **Proposed fix**: define uptime as node-process uptime; exempt `SOURCE_INGRESS` binding from the uptime and drop-rate criteria (the source's K_R(m) ≥ 2 check and reachability suffice); touch Ch6 §6.3.1, §6.3.3.
- already_known: no.

### F4 — Medium — The ingress-relay floor "20 Mbps suffices at M = 2" is computed with two relays already bound; the first relay is bound alone and fails K_R(m) ≥ 2

- **Rests on:** Ch6 §6.3.3: "The source accepts a relay as ingress only if K_R(m) ≥ 2 for every tree … With two ingress relays on the reference ladder that is 13.8 + 2.6 ⇒ u_R ≥ 18.2 Mbps at M = 2 … so the 20 Mbps `RELAY_CAPABLE` floor suffices for the small swarm an ingress relay is usually bound in."
- **Numbers** (c5): n_ingress = 1, u_R = 20, M = 2: ⌊(18 − 5.175)/6.9⌋ = ⌊1.859⌋ = **1** → rejected. n = 2: ⌊(18 − 2.5875)/6.9⌋ = ⌊2.234⌋ = 2 → accepted. Floor for the sole relay: u ≥ (13.8 + 5.175)/0.9 = **21.1 Mbps**; at M = 6: (20.7 + 5.175)/0.9 = **28.75 Mbps**. Under the "copy of L₀ however it is currently striped" reading, R_src at M = 2 is 3 × 3.0 × 1.15 = 10.35 and even n = 2 at 20 Mbps gives ⌊1.859⌋ = 1.
- **Wrong because** at N = 2 exactly one candidate exists, so n_ingress = 1 is the only case, and the one node the criteria admit (20 Mbps) is rejected by the source's own rule; the spec does not say which n the check uses before the second relay exists.
- **Debate**: the check could use the *target* n = 2; then the first relay holds half the pool and the other half is nowhere until the second binds. Either way the text must say so.
- **Proposed fix**: evaluate K_R(m) with n_ingress = number bound after this bind; state the sole-relay floor (21.1 / 28.75 Mbps) or raise the RELAY_CAPABLE floor to 22 Mbps; define R_src per the matrix in force (b₀ vs the L₀-carrying tree's bitrate); touch Ch6 §6.3.3, App B *Ingress relay*, Ch1 §1.1.5 §5.4.
- already_known: no.

### F5 — High — `RELAY_PROPOSAL(SOURCE_INGRESS)` has no signature field and no rule binds the session identity to PK_Publisher, so a relay cannot verify the proposer is the broadcaster

- **Rests on:** Ch6 §6.3.3 step 1: "sends `RELAY_PROPOSAL` with `BindFlags.SOURCE_INGRESS`, signed with SK_Source inside the QUIC session so the relay can verify it is the publisher." The RELAY_PROPOSAL byte diagram (same file) carries Target NodeID, Port, TreeID, BindFlags only. AUDITOR-AGENT.md: the byte diagram is the specification. Ch2 §2.2.4: NodeID = Blake3(PK_node ∥ N_static); Ch2 §2.3.3: `NodeClass`, `AssignedTrees`, `Flags` "are the peer's self-description and are copied verbatim into the Peer Record guardians serve"; the Registration Signature "covers $K_s \parallel \text{NodeID} \parallel \text{Port} \parallel \text{NodeClass} \parallel \text{AssignedTrees} \parallel \text{Flags} \parallel \text{Timestamp}$" under the registrant's own key, and the 42/54-byte Peer Record a guardian serves carries no signature field — so `Flags.SOURCE` is attested by nobody but the registrant.
- **With inputs** any identity I (one static puzzle, ≈ 10 ms) that registers with `Flags.SOURCE` and non-SYMMETRIC reachability, then sends `RELAY_PROPOSAL(SOURCE_INGRESS)` to every RELAY_CAPABLE record, **the spec produces** no check a relay can run: the frame has no signature and the session key is not stated to be the publisher key.
- **Wrong because** an accepting relay sets its assignment aside, takes I as parent at depth 0 in every tree, re-registers `SOURCE_INGRESS` and is scored at hop 0 by every joiner (c6, c9) while forwarding nothing verifiable; the swarm's ≥ 20 Mbps PUBLIC backbone is removed for the cost of one identity. The spec does not say when an ingress relay unbinds (c12), so the damage lasts ≥ 135 s per relay even after the fake source goes silent.
- **Debate**: a relay could compare the session's PK_node with the Stream Record's PublisherPubKey; that works only if the source's node key *is* the publisher key, which no sentence requires, and an implementer who does not assume it has only the self-declared flag. SOLUTION-021's residual ("a hostile parent claiming depth 0") covers a single lying parent, not the capture of every relay-capable node.
- **Proposed fix**: add a 64-byte publisher signature over (Relay NodeID ∥ K_s ∥ Timestamp) to RELAY_PROPOSAL when SOURCE_INGRESS is set, verified against the Stream Record's PublisherPubKey; or require NodeID_source = Blake3(PK_Publisher ∥ N_static) and say so; define unbind on source loss; touch Ch6 §6.3.3 diagram, App D §D.3 row 0x30, Ch2 §2.3.1.
- already_known: no.

### F6 — Medium — The delegated base-layer pool has no trigger: the starvation count reaches the source, the capacity sits at the ingress relay, and no frame connects them

- **Rests on:** Ch1 §1.1.5 §5.4: "The pool serves base-layer slots directly when it observes starvation signals: `GET_PEERS` re-queries whose `StarvedTrees` bitmap … returned to the publisher on every Stream Record store … A source that is itself behind NAT delegates this pool to its ingress relays." Ch6 §6.3.3: "each carries its share of the source's base-layer pool R_src." Ch2 §2.3.2: STORE_RECORD_ACK "is the only guardian → publisher path."
- **With inputs** S behind NAT, V1 ingress with R_src/n = 5.175 Mbps set aside (c7), StarvedCount[1] rising at the guardians, **the spec produces**: S learns it; V1 never does; no frame from S to V1 exists for it, and n_ingress changes (c11) are equally uncommunicated.
- **Wrong because** two implementers diverge: V1 never spends the pool (5.175 Mbps idle on a 20–25 Mbps node that the source already rejected at K = 1) or V1 treats it as ordinary slots (no reserve exists). Pattern #4 of issues/README.md.
- **Proposed fix**: either the ingress relay opens reserve slots on its own observation (a `NEIGHBOR(T_m)` from a requester whose `GET_PEERS` set the starved bit — not visible to it either) or, simpler, S forwards a per-tree "open k reserve slots" instruction on the ingress session (a `MANIFEST_UPDATE`-adjacent control frame, or a field in STORE_RECORD the relay reads); state what the relay does with R_src/n when n changes; touch Ch1 §1.1.5 §5.4, Ch6 §6.3.3, App D.
- already_known: no.

### F7 — Medium — `RxFlags.RELAYED` is asserted by the downloader and checked by nobody, so the ×3 weight and the "never more than 2×" bound are unenforceable at a verifier

- **Rests on:** Ch6 §6.3.2: "Verifiers weight `RELAYED` receipts 3× … a relay that bridges to the cap earns 0.5 × 3 + 0.5 × 1 = 2× the rank of pure tree relaying on the same upload, never more." Ch5 §5.2.2 RANK_PROOF checks: signature, window, per-receipt cap, adjacent-pair ordering.
- **Numbers** (b19): honest R at cap: (10 × 3 + 10 × 1) × 94 KB/s = 3,750 vs 1,875 → 2.0×. A relay whose 20 ordinary children set RELAYED: 20 × 3 × 94 = 5,625 → **3.0×**, every sampled pair passes. With the ISSUE-057/SOLUTION-057 leaf-Sybil set (rank ≈ 220 from 100 Sybils) the flag lifts it to ≈ 660.
- **Wrong because** the bound is argued from the relay's budget, which no verifier can observe; the flag is in the child's signed payload and the child has no reason to be honest about it when the parent asks.
- **Debate**: honest children have no incentive to lie *for* a parent — but Sybil children (already the accepted residual of SOLUTION-057) are the parent, and the flag triples the gap the residual already accepts.
- **Proposed fix**: credit RELAYED only up to ½ of a prover's receipt count (mirrors the budget cap, verifiable from the sorted list) or only when the receipt's `TreeID` is one the prover does not list in `AssignedTrees` (verifiable from the prover's registration); touch Ch6 §6.3.2, Ch5 §5.2.2, App D §D.4.17.
- already_known: no (adjacent to SOLUTION-057's residual "downloader diversity attestation").

### F8 — Medium — A bridged peer's repair is a full bridge cycle, which at the worst end exceeds the PULL zone

- **Rests on:** Ch3 §3.3.1: "T = τ_evict + ≈ 2 RTT: Standby peer accepts." Ch4 §4.3.1: W_pull = 1.5 s at ≤ 150 ms RTT "must cover one parent repair … plus two PULL round-trips." Ch6 §6.3.3: "`RELAY_BIND` is sent … once it holds a slot at B."
- **Numbers** (b20): eviction 320 ms + GET_PEERS 80 + session 160 + R′ probe 200 + R′ NEIGHBOR 160 + BIND 40 + NEIGHBOR 80 = 1,040 ms, + R′ warm-up ≤ 1,000 ms → **1.3–2.4 s after loss** vs W_pull 1.5 s. At 250 ms RTT: 490 + 2,250 ≈ 2.7–3.7 s vs W_pull 2.0 s.
- **Wrong because** R5 promises sub-second repair and "never playback" loss; for a bridged L₀ tree the worst end freezes, and nothing in the spec lets a bridged peer hold a standby bridge or lets a relay pre-join a tree for a prospective bridge.
- **Debate**: the best end (R′ already holds T_m, or a PUBLIC standby has a slot) is inside W_pull; the peer was bridged precisely because no PUBLIC slot existed, so the worst end is the expected one.
- **Proposed fix**: a bridged peer keeps one standby RELAY_CAPABLE record per bridged tree and may send a RELAY_PROPOSAL to it at eviction time without a new GET_PEERS; a relay may answer RELAY_BIND only after its first verified segment in T_m (removes the warm-up from the child's timeline, costs the relay nothing); touch Ch6 §6.3.3, Ch3 §3.3.1.
- already_known: no.

### F9 — Low — Scenario C arithmetic: bridge supply at the 20 Mbps floor cannot meet 75 % symmetric demand at the reference upload (already known as validation owed)

- **Rests on:** Ch8 §8.3 Scenario C: "tests if the Emergent Relayer algorithm (Chapter 6) can recruit and scale public-IP relays autonomously without incurring TURN server fallback dependencies." Ch6 §6.3.1 sizes the mechanism for "~15 % of peers".
- **Numbers** (d2–d4): per 133 peers, 13 PUBLIC; symmetric demand 100 per tree vs 67 PUBLIC slots per tree; 178–396 bridge edges per 100 private leaves; 31 bridges per 20 Mbps relay → 6–13 RELAY_CAPABLE nodes, 46–100 % of all PUBLIC nodes, at a reference mean upload of 10 Mbps.
- **Wrong because** the claim "served without TURN" at 75 % holds only if essentially every PUBLIC node is a 20 Mbps, 30-minute-old relay; the specification offers no fraction and no fallback.
- already_known: yes — SOLUTION-032 and SOLUTION-050 "Validation owed (Chapter 8): Scenario C". Recorded here because the numbers are sharper than the owed item.

### F10 — High (impl) — The implementation authenticates the broadcaster behind `SOURCE_INGRESS` from a self-declared, unsigned Peer Record flag

- **Rests on:** `src/crates/bs-core/src/node/membership.rs:106-113` — `passive_insert` sets `source_record` from any record with `flags.source_ingress`, or `flags.source` when the record is non-SYMMETRIC or no source is known; callers include `FORWARD_JOIN` origins (`membership.rs:439`) and gossip (`:516`, `:601`). `src/crates/bs-core/src/node/nat.rs:256-261` — `on_ingress_proposal` accepts when `source_record.node_id == peer && flags.source`. `src/crates/bs-core/src/node/dht.rs:742` — guardians consult `flags.source` only for sampling, never against the Stream Record's `publisher_pubkey`.
- **With inputs** an attacker record (PUBLIC, `Flags.SOURCE`) delivered by gossip to a RELAY_CAPABLE node whose real source is SYMMETRIC, **the implementation produces**: the attacker's record replaces the real source's ("a reachable source record is never replaced by an unreachable one" — the reverse direction *is* a replacement), the proposal is accepted, every tree's parent is overwritten (`nat.rs:285-308`), and the node re-registers `SOURCE_INGRESS` for the attacker.
- **Wrong because** the spec's intent is a publisher signature (F5); the implementation substitutes trust in a field the spec says is copied verbatim and unsigned.
- already_known: no.

### F11 — Medium (impl) — A saturated answer resets `silent_rounds`, so a NAT-blocked peer never asks for a bridge while any PUBLIC relay in its sample is full

- **Rests on:** `src/crates/bs-core/src/node.rs:975-985`: `silent_rounds += 1` only when `last_answered == 0`, reset to 0 when `last_answered > 0`; bridge requested when `silent_rounds >= bridge_after_failed_rounds` (2).
- **With inputs** walk (b) rounds 1–2 (one saturated PUBLIC + three unreachable CONE each round, b9–b10) **the implementation produces** `silent_rounds = 0` after every round; `request_bridge` never runs; V_b loops on re-queries without reaching ACTIVE.
- **Wrong because** the implementation's own rule comment ("A bridge is for a NAT-blocked peer whose probed parents never answer") treats a full PUBLIC parent as an answer that disqualifies the bridge, while the spec (Ch1 §1.2.5) names exactly this case — "Where it cannot reach a CONE parent it needs" — as the bridge's purpose. Shares the cause of F2.
- already_known: no.

### F13 — Low (impl) — The implementation's ingress-relay admission omits the R_src/n_ingress term the spec subtracts

- **Rests on:** Ch6 §6.3.3: "The source accepts a relay as ingress only if $K_R(m) \ge 2$ for **every** tree … $0.9\,u_R \ge 2 M B_{\max} \Omega + R_{\text{src}}/n_{\text{ingress}}$." App B *Ingress relay* row: $K_R(m) = \lfloor ((1-r_{\text{pull}})u_R - R_{\text{src}}/n_{\text{ingress}}) / (M B_m \Omega) \rfloor$. `src/crates/bs-core/src/node/nat.rs:267-273`: `enough = matrix.trees.iter().all(|t| self.params.slots(self.cfg.upload_kbps, m, t.bitrate_kbps, parity) >= 2)` — the ordinary slot formula, no $R_{\text{src}}$ term, and no $n_{\text{ingress}}$ input.
- **With inputs** $u_R = 20$ Mbps, $M = 2$, $B_{\max} = 3.0$ (c5) **the implementation produces** $\lfloor 18/6.9 \rfloor = 2 \to$ accept; the spec at $n_{\text{ingress}} = 1$ produces $\lfloor (18 - 5.175)/6.9 \rfloor = 1 \to$ reject, and at $n = 2$ produces $2 \to$ accept.
- **Wrong because** the implementation admits an ingress relay that, if it also honoured the delegated pool (F6), would have $18 - 13.8 - 5.175 < 0$ Mbps left — the pool and the two slots per tree cannot both be funded on the node it accepts. Low because F4 shows the spec's own $n_{\text{ingress}}$ is undetermined at the first bind and F6 shows the pool has no trigger, so the divergence has no visible consequence until those are fixed; it is recorded so the fixer does not take the implementation as the spec's reading.
- already_known: no (follows from F4/F6; separate because the fix is in `nat.rs`, not `protocol/`).

### F12 — Low — A NAT-blocked source must register as RELAY-class and may not

- **Rests on:** Ch2 §2.3.2: "The source registers under K_s as an ordinary `RELAY`-class peer with `Flags.SOURCE` set." Ch1 §1.2.5 table: SYMMETRIC "may declare RELAY? no — defaults to LEAF."
- **With inputs** S SYMMETRIC (c1) **the spec produces** two legal registrations with different consequences: as RELAY it is counted in N_relay (one step of the M = 2 → 3 ladder at the 12-relay threshold) and served in `WantedTrees` samples (then filtered by every joiner, a wasted record of 20); as LEAF it is served only to membership queries.
- **Wrong because** two implementers diverge and the ladder input differs by one in the regime where one relay is a ladder step; no behavioural failure beyond that.
- already_known: no.

## Spec silences assumed through

1. **Second PROBE after a punch** (a11): no rule; walked both readings. Candidate finding F1.
2. **"One registration period"** for the unpunchable mark (App D §D.4.14): 90 s (refresh) or 180 s (τ_ttl)? Assumed 90 s; no consuming rule depends on which.
3. **Punch validity 1 s** (App B): no node enforces it; the target accepts a PROBE from any source at any time (unknown-source budget, Ch7 §7.2.1). Assumed decorative; no finding.
4. **Mixed round classification** (b9): assumed "failed" for shed purposes and "silent" for bridge purposes. Candidate finding F2.
5. **Bridge trigger** (b10): assumed 2 rounds (implementation constant). F2.
6. **JOIN to a CONE membership peer during JOINING** (a6): §D.4.14 is written for the tree-join PROBE; assumed the same punch applies to JOIN, and that membership targets were PUBLIC.
7. **RELAY_BIND before the relay's first verified segment** (b15): the text gates BIND on the slot; assumed BIND is sent on ACCEPTED and the child waits through the relay's warm-up. Feeds F8.
8. **Source's re-query interval for RELAY_CAPABLE records** (c2): none stated; assumed 2 s (implementation).
9. **Uptime clock for RELAY_CAPABLE** (c3): assumed node-process uptime. F3.
10. **n_ingress used in K_R(m) at the first bind** (c5): walked both. F4.
11. **R_src at M = 2** ("three copies of L₀, however it is currently striped"): assumed the spec's 5.175; the striped reading gives 10.35. Noted in F4.
12. **How the ingress relay learns the starvation signal and n_ingress changes** (c7, c11): no frame. F6.
13. **Ingress unbind on source loss** (c12) and re-bind when both relays leave (SOLUTION-032 residual): assumed none; noted in F5.
14. **Class of a SYMMETRIC peer**: the spec says LEAF, the scenario text said LEAF_PRIVATE; walked as LEAF.
15. **Whether non-symmetric peers avoid PUBLIC parents** (d2): no steering rule; walked both bounds.
16. **Whether a bridged child issues receipts to R during R's WARMING** (b17): assumed no blocks → no receipts → no deadline (receipts are due only after blocks).

Comparison points with the implementation used above: `nat.rs:323-352` (reflection → reachability, matches Ch2 §2.1.3), `nat.rs:509-532` (τ_nat keepalive, matches), `nat.rs:773-780` (bridge budget ½(1 − r_pull)u, matches), `nat.rs:267-273` (K_R ≥ 2 check uses `slots(u, M, B_m)` without any R_src term — diverges from Ch6 §6.3.3, which subtracts R_src/n_ingress; at 20 Mbps, M = 2 the implementation computes ⌊18/6.9⌋ = 2 and accepts where the spec's n = 1 arithmetic rejects), `node.rs:730-731` (PROBE sent immediately after the punch, as the spec's pseudocode), `nat.rs:469` (re-probe on PING, beyond the spec).

## Re-verification (second walker, 2026-10-30)

Every quoted sentence in F1–F12 was re-opened in the cited file and found verbatim, with one correction: F5 had attributed "is not signed" to Ch2 §2.3.3; the file instead says the Registration Signature covers `Flags` under the registrant's own key and the served Peer Record carries no signature field. F5 is reworded to the text that is there; its conclusion is unchanged. Implementation line references (`membership.rs:106-113`, `nat.rs:256-273`, `nat.rs:469`, `node.rs:975-985`, `params.rs:267`, `bs-sim/src/network.rs:119`, `scenarios/nat_symmetric.toml:29,49`) were re-read and match. The K_R divergence noted in the comparison paragraph above is now F13. Solutions checked for prior acceptance: SOLUTION-032 and SOLUTION-050 list Scenario C under "Validation owed" (F9 already_known); neither lists the punch ordering, the bridge trigger, the ingress signature, the pool trigger, the `RELAYED` verifier gap or the bridged-repair timeline as accepted residual risk. Open tickets ISSUE-061..072 do not cover any of F1–F8, F10–F13.
