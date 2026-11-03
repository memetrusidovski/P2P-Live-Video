# Trace: dht-at-scale — discovery and registration at N = 1,000,000

Walker scenario `dht-at-scale`. The guardian side of the DHT at one million viewers, stepped on a virtual clock: the publisher's write path, a registrant's lifecycle, one joiner's path from `GET_PEERS` to a usable parent list, guardian-set churn, and the sampling arithmetic. Requirements checked: R2, R3, R8, R10 (required); R7 (touched).

## Scenario and initial state

Constants (protocol/appendix_b_parameters.md unless noted):

| Constant | Value | Source |
|---|---|---|
| N | 1,000,000, steady state, stream older than 30 min | scenario |
| Leaf fraction ℓ | 0.30 → N_relay = 700,000 (Ch1 §1.2.5 bounds ℓ ≤ 42 % at 10 Mbps relays; 0.5 would be unsustainable and is another walker's scenario) | Ch1 §1.2.5 §5.3 |
| M | 6, reference mapping (T1–T4 at 0.75 Mbps, T5–T6 at 1.5 Mbps) | Ch1 §1.2.1 §1.4 |
| ū_relay | 10 Mbps → K_v = 10 on a 0.75 Mbps tree, 5 on a 1.5 Mbps tree | Ch1 §1.2.1 worked values |
| Relays per tree | 116,667; slots per L0/L1 tree 1.17 M (17 % spare for 10⁶ subscribers) | derived |
| Churn | 1 % per minute = 166.7 joins/s and 166.7 departures/s (mean session 100 min). The spec's own steady-state figure, 5-minute sessions = 3,333 joins/s (Ch2 §2.3.2 *Query Discipline*), is carried as the worst end | scenario; Ch2 §2.3.2 |
| Relay reachability | 40 % PUBLIC, 60 % CONE (the spec's CONE share, Ch2 §2.3.2 step 5) | Ch2 §2.3.2 |
| RELAY_CAPABLE share | PUBLIC ∧ u ≥ 20 Mbps (taken as 25 % of PUBLIC relays) ∧ uptime ≥ 30 min (74 % of nodes at 1 %/min exponential ages) → g = 7.4 % of relays = 51,800 nodes = 5.2 % of N | Ch6 §6.3.1 |
| N₀ | 10⁴; s = max(0, ⌈log₂(N_est / N₀)⌉) | B.1.2; Ch2 §2.3.2 |
| τ_ttl / refresh / active | 180 s / 90 s / 135 s | B.1.1, B.1.2 |
| τ_nat | 25 s (CONE registrant PING to each guardian) | B.1.1 |
| Starved-count window | 10 s; QueryCount and StarvedCount never scaled | B.1.1 |
| Pool floor / re-query | 3 relays per subscribed tree; ≥ 30 s between re-queries per tree | B.1.2, B.1.1 |
| k, α | 20, 3 | B.1.2; Ch2 §2.1.2 |
| k-bucket prefix cap | exactly 1 of 20 per /24 or /48, unconditional | B.1.2 *Prefix cap*; App C §C.2.1 |
| GLOBAL_NEW_PPS / RATE_PPS_UNKNOWN | 5,000 / 50 pps | B.1.4 |
| RTT | 80 ms reference (40 one-way); 250 ms worst end | Ch1 §1.1.3; Ch3 §3.3.1 |
| τ_gossip | 1,000 ms | B.1.1 |
| θ_join | max(1, min(4, N − 1)) = 4 | Ch1 §1.3.1 |
| Frame sizes (bytes on the wire, + 28 UDP/IPv4) | `REGISTER_PEER` 260; `GET_PEERS` req 192; `GET_PEERS` resp 36 + 20×42 + 238 = 1,114 (IPv4), 1,354 (IPv6); `PING` 188; `PONG` 163; `FIND_*` req 220 / resp 878; `STORE_RECORD` 4 + 152 + 238 = 394 (280-byte record during a pending matrix: 436); `STORE_RECORD_ACK` 72; `PROBE` 189; `PROBE_RESPONSE` 48 | Ch2 §2.3.3; App D §D.4.1, §D.4.6, §D.4.7 |

Nodes: **P** publisher (PUBLIC, u_S = 15 Mbps). **G1..G20** guardians of K_s (reachable relay-class nodes, ordinary 10 Mbps relays). **R** an ordinary 10 Mbps CONE relay whose NodeID is in the registration sample. **V** a joiner (PUBLIC relay-class, 10 Mbps) pressing play at t = 0 of walk C. **Q** a node that joins the DHT and lands among the 20 closest to K_s (walk D).

Two spreads of `GET_PEERS` over the guardians are carried through, because the spec asserts one and the lookup produces the other (capacity-shortfall F2): **uniform** (each query reaches a random α = 3 of the 20) and **closest-α** (every query reaches the 3 guardians nearest K_s, which is what an S/Kademlia `FIND_VALUE` terminates at and what the implementation does, `src/crates/bs-core/src/node/dht.rs` `dht_send_get_peers`: targets = `holders`).

Sampling arithmetic used throughout (Ch2 §2.3.2 *Sampled Registration*; see tick A3 for how the state is reached):

| Quantity | s = 7 (the spec's figure for 10⁶) | s = 8 (what the rule produces here) |
|---|---|---|
| Ordinary registrations (N − N_rc)/2^s | 948,200 / 128 = 7,408 | 3,704 |
| RELAY_CAPABLE registrations N_rc / 2^{s−3} | 51,800 / 16 = 3,238 | 1,619 |
| ActiveCount (sum) | 10,645 | 5,323 |
| N_est = ActiveCount · 2^s | 1,362,560 | 1,362,688 |
| Next s = ⌈log₂(N_est / 10⁴)⌉ | ⌈7.09⌉ = **8** | ⌈7.09⌉ = 8 (stable) |
| Registered relays (ordinary (700,000 − 51,800)/2^s + RC) | 5,064 + 3,238 = 8,302 | 2,532 + 1,619 = 4,151 |
| RelayCount · 2^s | 1,062,656 | 1,062,656 (true 700,000: +52 %) |
| Registered relays per tree (RC nodes hold 2 trees at 20 Mbps) | ≈ 1,920 | ≈ 960 (422 ordinary + 540 RC) |
| Sample slots per L0/L1 tree | ≈ 19,200 | ≈ 9,600 (422×10 + 540×10) |

## Tick table

Columns: t_ms | node | rule (file §) | computation | state after.

### Walk A: publisher write path and guardian load, one segment at steady state

| t_ms | node | rule (file §) | computation | state after |
|---|---|---|---|---|
| A1: 0 | P → G1..G20 | Ch2 §2.3.2 *The Publisher's Write Path*; §2.3.3 `STORE_RECORD` | Record 238 B (no pending matrix), frame 394 B + 28 = 422 B × 20 = 8.4 KB per segment = **67.5 kbps** egress. P knows the 20 guardians from its own `FIND_NODE` lookup (cadence unstated, S2; 30 s assumed, as the implementation). | 20 STORE_RECORDs in flight |
| A2: 40 | G_i | §2.3.3 `STORE_RECORD` acceptance | Verify PublisherPubKey hashes to K_s; Ed25519 verify; `ManifestVersion`/`LiveEdgeSegmentId` not lower than held → `Accepted = 1`. Counts over active registrations (refreshed < 135 s): `ActiveCount` 5,323, `RelayCount` 4,151, `PerTreeRelayCount[m]` ≈ 960, `QueryCount` = distinct `GET_PEERS` senders in 10 s, `StarvedCount[m]`. | ACK 72 + 28 = 100 B sent |
| A3: 80 | P | §2.3.2 publisher table; Ch1 §1.1.5 §5.6 | Median over 20 ACKs. N̂ = 5,323 × 256 = 1,362,688 (true 10⁶: **+36 %**, the RC registrations sampled at 2^{−5} are scaled by 2^8 like every other registration). N_relay = 4,151 × 256 = 1,062,656 (true 700,000: **+52 %**). s = ⌈log₂(136.3)⌉ = 8, stable (the spec's own "s = 7 at N = 10⁶" is what the rule produces only with no RELAY_CAPABLE nodes). Ladder: N_relay ≥ 30 → M = 6. PoW tier(N̂ ≥ 10⁵) = 14. θ_join = 4. | record for segment k+1 will carry s = 8, SwarmSize 1.36 M, RelayCount 1.06 M |
| A4: 80 | P | Ch1 §1.1.5 §5.6 *Estimate*; *Fold rule* | QueryCount per guardian (uniform): joins 166.7/s × 2 queries each (membership 0x00 + trees 0x3F, S1) × α/k = 50/s → 500 distinct in 10 s, plus the starved re-query of tick C9 → another 250. `StarvedCount[m]` = 250 for every m (every joiner re-queries once with `StarvedTrees = 0x3F`, tick C9). ŝ_m = (20/3) × 250 / 1,362,688 = **0.12 %** < 5 %: no fold. Closest-α: QueryCount 5,000, StarvedCount 1,667 at G1–G3, 0 at G4–G20 → median 0 (known, capacity-shortfall F2). | no matrix change |
| A5: 0–1,000 | G_i | §2.3.2 *Sampled Registration*; step 4 | REGISTER_PEER inbound: 5,323 / 90 s = **59.1/s** × 288 B = 17.0 KB/s; 2 Ed25519 verifications + 1 PoW check each = 118 verifies/s. (Un-sampled: 11,111/s; the spec's s = 7 figure 87/s; here 59/s because s = 8.) | 5,323 registrations held, < 0.6 MB |
| A6: 0–1,000 | G_i | §2.3.2 *Query Discipline*; §2.3.3 `GET_PEERS` | GET_PEERS inbound (uniform): 166.7 joins/s × (2 + 1 starved re-query) × 3/20 = **75/s** × 220 B = 16.5 KB/s. Outbound responses 75 × 1,142 B = **85.7 KB/s = 0.69 Mbps** (IPv6 1,382 B: 0.83 Mbps). Closest-α: 500/s in, 571 KB/s = **4.6 Mbps** out at G1–G3. Per-tree reading of Ch1 §1.2.2 (one `GET_PEERS` per tree, finding F3): ×3.5 on these figures. | |
| A7: 0–1,000 | G_i | §2.3.2 step 5 (CONE keepalive) | CONE registrants: 2,532 ordinary relays × 0.6 = 1,519 (RC nodes are PUBLIC) → 1,519 / 25 s = **60.8 PING/s** per guardian × 216 B = 13.1 KB/s in; PONG 191 B → 11.6 KB/s out. Spec's "< 200 pps per guardian" holds. (Whether CONE *leaf* registrants also keepalive is unstated, S3; +1,111/25 = 44 pps if they do.) | |
| A8: 0–1,000 | G_i | Ch2 §2.1.2–§2.1.3; App D §D.4.6 | Lookup traffic terminating near K_s: registrants re-resolving guardians 59.1 lookups/s × ≈ 6 final-hop RPCs / 20 = 18/s; joiners' `FIND_VALUE` 166.7 × 3 paths = 500/s → 25/s uniform (167/s closest-α). ≈ 43 FIND/s × 220 B = 9.5 KB/s in; responses 906 B → 39 KB/s out. | |
| A9: 1,000 | G_i | totals; Ch7 §7.2.1 budgets | **Per guardian, uniform spread, 1 %/min churn:** inbound 59 + 75 + 61 + 43 + 1 = **239 pps**, 57 KB/s (0.45 Mbps); outbound 85.7 + 11.6 + 39 + 0.1 = **136 KB/s = 1.09 Mbps**; ≈ 300 Ed25519 verifications/s (≈ 3 % of one core). Against GLOBAL_NEW_PPS = 5,000: 4.8 %. **Closest-α:** inbound 59 + 500 + 61 + 185 ≈ **805 pps**, outbound 4.6 + 0.3 + 1.3 ≈ **6.2 Mbps** at G1–G3 — 65 % of a 10 Mbps guardian's uplink, which its slot count has already committed (flash-crowd F5, known). **Spec's worst end (5-min sessions, 3,333 joins/s), uniform:** GET_PEERS 1,500/s, inbound ≈ 2,100 pps, outbound 1,500 × 1,142 = **13.7 Mbps** — exceeds the whole uplink; **closest-α:** 10,000 GET_PEERS/s + 3,400 FIND/s ≈ **13,500 pps > GLOBAL_NEW_PPS = 5,000**: the guardian's own shield drops ≈ 60 % of pre-session frames, the failure SOLUTION-037 was written to remove. | steady state |

### Walk B: one registrant's lifecycle

| t_ms | node | rule (file §) | computation | state after |
|---|---|---|---|---|
| B1: 0 | R | §2.3.2 *Sampled Registration* | Reads s = 8 from the Stream Record it holds. Blake3(NodeID)[0:2] mod 256 = 0 → in sample. (How R learns a *later* change of s is undefined — flash-crowd F1, known; assumed through, S8.) | will register |
| B2: 0 | R → G1..G20 | §2.3.2 steps 1–3; §2.3.3 `REGISTER_PEER` | Needs the 20 closest to K_s: `FIND_NODE(K_s)`, 5 hops × 80 ms = 400 ms (S5). Sends 20 × 288 B = 5.8 KB. Payload: NodeClass RELAY, AssignedTrees bit a(R), Flags Reachability = CONE, RELAY_CAPABLE = 0. | registered at t ≈ 400 (uniform count; the walk keeps t = 0 as the refresh epoch) |
| B3: 40 | G_i | §2.3.2 step 3; Ch2 §2.2.3 | Binds the **observed** UDP source address; dynamic PoW at tier 14 (accepts tier 12 under the verifier grace band) against that address; validation-block signature and registration signature verified. No `REGISTER_RESPONSE` frame exists (flash-crowd F7, known), so R cannot tell a dropped registration from an accepted one. | record active until t + 135,000, retained until t + 180,000 |
| B4: 25,000 | R → G1..G20 | §2.3.2 step 5; B.1.1 τ_nat | `PING` (216 B) to each guardian: 20 × 216 = 4.3 KB per 25 s = 1.4 kbps per CONE registrant. Guardian's unknown-source bucket (50 pps) sees 1 packet. | mapping alive for `PUNCH_REQUEST` forwarding |
| B5: 50,000 / 75,000 | R | τ_nat | PINGs 2 and 3. A NAT with a 30 s idle timeout (the implementation's `nat_mapping_ttl`) stays open; a sub-25 s NAT does not (SOLUTION-051 residual, accepted). | |
| B6: 90,000 | R → G1..G20 | §2.3.2 step 4 | Refresh `REGISTER_PEER`. If R's class, trees or flags changed earlier, it re-registered at once (e.g., becoming RELAY_CAPABLE at 30 min of uptime, which also moves it to the 2^{−(s−3)} sample). | active window restarts |
| B7: 30,000 (variant) | R leaves silently after B2 | B.1.2 *Active registration* | R departs at t = 30,000. Its registration stays **active and served** until t = 135,000 (105 s) and retained to 180,000. Fraction of served records that are dead at steady state: departures in the trailing (135 − 45) s = 1.5 % at 1 %/min; **30 %** at the spec's 5-min sessions. Each dead record costs a joiner one 200 ms `PROBE` timeout, taken in parallel with the live probes. SOLUTION-026 records the count lag as accepted risk. | |
| B8: 135,000 | G_i | B.1.2 | R's record leaves `ActiveCount`, `RelayCount`, `PerTreeRelayCount` and the `GET_PEERS` pool. | counted-out |
| B9: 180,000 | G_i | B.1.1 τ_ttl | Record deleted. | gone |

### Walk C: joiner V, from pressing play to a usable parent list (reference RTT 80 ms; worst end 250 ms in brackets)

| t_ms | node | rule (file §) | computation | state after |
|---|---|---|---|---|
| C1: 0 | V | Ch2 §2.1.3 steps 1–4 | BOOTSTRAP: PING two seeds (80 ms), self-lookup; both reflections equal local → PUBLIC. Dynamic puzzle tier 14 = 16,384 hashes ≈ 2.5 ms. (Assumed already done before "play"; SJL starts here per Ch8 §8.2.) | DISCOVERY |
| C2: 0 → 400 [1,250] | V | §2.1.3 step 5; §2.1.2 | `FIND_VALUE(K_s)` over α = 3 disjoint paths through a DHT of 700,000 reachable relay-class nodes: ≈ 5 hops × RTT = **400 ms** [1,250]. The spec's own figure is 0.3–0.5 s (Ch4 §4.3.1 anchor table). Terminates at the holders — the guardians nearest K_s. | holds Stream Record from the value hit |
| C3: 400 | V → 3 holders | §2.3.2 *Stream Discovery*; §2.3.3 `GET_PEERS` | Two queries to each of α = 3 guardians (S1): `WantedTrees = 0x3F` and the membership query `0x00`. 6 × 220 B = 1.3 KB. The holders are the 3 closest — the closest-α spread of walk A. | 6 requests in flight |
| C4: 480 [1,500] | G → V | §2.3.3 `GET_PEERS` response | Tree query: Count = 20, ⌈20/6⌉ = 4 records per tree, drawn uniformly from ≈ 960 registered relays per tree → ≈ **12 distinct candidates per tree** over 3 responses (overlap ≈ 1 %). Membership query: 20 registered peers of any class. Stream Record L = 238: s = 8, SwarmSize 1,362,688, RelayCount 1,062,656, LiveEdgeSegmentId X, matrix in force. θ_join = max(1, min(4, 1,362,687)) = 4. | JOINING; pools P_m = 12 DHT records each |
| C5: 480 → 640 [2,000] | V → 4 membership records | Ch1 §1.3.1; Ch3 §3.1.2; App D §D.4.2 | `JOIN` → QUIC upgrade, 2 RTT. Each acceptor adds V to its Active Set, evicting a random member if full, and sends `FORWARD_JOIN` (TTL 4). `ACCEPTED(JOIN)` carries no Peer Records; the first `GOSSIP_EXCHANGE` arrives at each neighbour's next τ_gossip tick (S4). | CONNECTING at 640; pools still DHT-only |
| C6: 640 [2,000] | V | Ch1 §1.2.2 §2.2 step 2; App D §D.4.14 | Round 1 in all six trees: 12 candidates per tree, 60 % CONE → `PUNCH_REQUEST` to the referring guardian (+1 RTT, reading B of nat-and-relays), PUBLIC probed at once. `PROBE` 189 B × 72 = 13.6 KB. | probing |
| C7: 720–800 [2,250–2,500] | candidates → V | App D §D.4.7; Ch1 §1.2.2 *Warm-Up Gating* table | Every `PROBE_RESPONSE`: `TreeState = SERVING`, **`K_avail = 0`**. Reason: the registered relays are the landing zone of every DHT-directed join in the swarm. Sample slots per L0 tree ≈ 9,600; at stream start they had the swarm's 17 % spare = 1,630 slots, consumed by 166.7 joiners/s in **9.8 s**; thereafter slots free in the sample at 9,600 × 0.01/60 = **1.6 slots/s against 166.7 joiners/s** → P(a given joiner finds a free slot among its 12 sample candidates) ≈ **1 %**. `available_slots > 0` filter leaves `scored_candidates` empty in every tree. | round 1 fails in 6/6 trees |
| C8: 800 [2,500] | V | Ch1 §1.2.2 §2.2 return; Ch1 §1.1.5 §5.3 items 1–3 | `FAILURE_RETRY_BACKOFF`; backoff ≥ 1.0 s (one segment). Failed-round counter = 1 for T3–T6 (shed after 2); T1–T2 never shed but ACTIVE is gated on them. | waiting until ≥ 1,800 |
| C9: 800 [2,500] | V → α guardians | Ch2 §2.3.2 "Retry means re-query"; §2.3.3 `StarvedTrees` | Re-query `GET_PEERS(WantedTrees = 0x3F, StarvedTrees = 0x3F)` — "bit m−1 set means I have failed to obtain a parent in T_m", literally true. Guardians add V to `StarvedCount[1..6]`. **Every joiner in the swarm does this once**, which is the 250-per-guardian figure of tick A4 and the 7.3 % ŝ_m of tick E3 under a surge. | fresh sample: 12 more saturated records per tree |
| C10: ≈ 640–1,640 | V's 4 neighbours → V | Ch3 §3.2.1; B.1.1 τ_gossip | Each neighbour's `GOSSIP_EXCHANGE` tick lands uniformly in the next 1,000 ms, carrying ≈ 8 records steered to V's `WantedTrees` (all six): by ≈ 1,640, 32 records ≈ **5 un-sampled relays per tree**. | pools P_m ≈ 5 live + 24 saturated |
| C11: 1,800 [3,500] | V | §2.2 step 2 | Round 2: 5 gossip records + 12 fresh DHT records per tree; probe (CONE via the gossip referrer's open session, +1 RTT). | probing |
| C12: 1,960 [4,000] | V | §2.2 step 3 | On L0/L1 trees a random relay has a free slot with p ≈ 0.17 → P(≥ 1 of 5) = 1 − 0.83⁵ = **61 %**. **Usable parent list at t = 1,960 ms [4,000]**; with 39 % probability per tree a third round at 2,960. (On T5–T6 at 10 Mbps relays L2 is unsustainable, Ch1 §1.1.5 §5.2 — capacity, not discovery; another walker's scenario.) | scored candidates |
| C13: 2,120 [4,500] | V ↔ parent | App D §D.4.3, §D.4.4 | `NEIGHBOR(TreeID = m)` over QUIC (handshake + request, 2 RTT) → `ACCEPTED(HopDepth)`. | parents in T1–T4 (61 % case) |
| C14: 2,200 [4,750] | V → first parent | Ch4 §4.3.1 steps 3–4; App D §D.4.16 | `MANIFEST_REQUEST(X, 0xFF)` + `0xFD` descriptor, 1 RTT. ACTIVE. First renderable chunk after segment X backfill or X+1 push: SJL ≥ **≈ 2.5 s** [≈ 5.3 s]. Had the sample had a free slot (round 1 succeeding at C7), the list would have been usable at **800 ms** and ACCEPTED at 960: the saturated sample costs the whole margin of the 1.5 s budget plus 0.5 s. | ACTIVE |

### Walk D: guardian-set churn, k-bucket cap, matrix change

| t_ms | node | rule (file §) | computation | state after |
|---|---|---|---|---|
| D1: 0 | Q | Ch2 §2.1.3 step 4; §2.3.2 *Who Is a Guardian* | Q (reachable relay) joins the DHT; its NodeID is closer to K_s than G20's. Rate of such events: 166.7 joins/s × 60 × 20/700,000 = 0.29/min; guardian departures 20 × 1 %/min = 0.2/min → the guardian set changes **≈ 0.5 times per minute**. | Q is one of the 20 closest; holds no record, no registrations |
| D2: 0 → 30,000 | registrants, joiners → Q | §2.3.2; §2.3.3 `L = 0` | Registrants find Q at their next refresh (over 90 s); joiners' `FIND_VALUE` does not stop at Q (it holds no value) unless Q is among the first α answerers. A `GET_PEERS` that does reach Q returns Count ≤ its partial set and **L = 0**: "the joiner must then treat swarm-global values as unknown". Q's ACK counts are low for ≤ 90 s; one of 20, absorbed by the median. The spec names no republish and no lookup cadence for P (S2); the implementation re-resolves every 30 s. | consistent within ≈ 30–90 s |
| D3: 30,000 | P → Q | §2.3.2 write path | P's guardian lookup finds Q; next `STORE_RECORD` reaches it. | Q serves L = 238 |
| D4: any | every DHT node | B.1.2 *Prefix cap*; App C §C.2.1 | Bucket toward K_s holds ≤ 1 contact per /24. Among the 20 closest of 700,000 nodes spread over ≈ 10⁵ /24s, P(two share a /24) ≈ C(20,2)/10⁵ = **0.19 %** → the cap displaces an honest guardian in ≈ 1 stream in 500, consistently at every node (the 21st closest serves). Eclipse: owning all 20 guardians of a known StreamID costs 20 distinct /24 or /48 prefixes plus ≈ 20 × 700,000 ≈ 1.4 × 10⁷ hashes (≈ 1–2 s on a GPU, Ch2 §2.2.1); accepted residual (App C, SOLUTION-034/042). **Implementation:** `RoutingTable::insert` (`src/crates/bs-core/src/dht.rs`) has no prefix check at all — finding F4. | pass at this scale; impl divergence |
| D5: 0 | P | Ch1 §1.2.4 §4.5 step 1; Ch2 §2.3.3 Stream Record | Layer fold or resize: `MANIFEST_UPDATE` down the six trees (7 B × 6 matrix) and the record re-stored with `NumTreesNext = 6`, `EffectiveSegmentSeq = S + 5`: record 280 B, STORE 464 B × 20 = 9.3 KB (74 kbps for one segment). `GET_PEERS` response 36 + 840 + 280 = 1,156 B IPv4 / 1,396 IPv6 — under the MTU (spec: ≈ 1,400 ✓). A joiner inside the window holds both matrices and fetches the signed frame with `MANIFEST_REQUEST(0xFE)`. | pending matrix published |
| D6: 0–135,000 | P | Ch2 §2.3.1 *swarm_size / relay_count*; SOLUTION-026 residual | N_relay for the ladder lags departures by ≤ 135 s and refreshes by ≤ 90 s; at 1 %/min the lag is ≤ 2.25 % of relays. Ladder hysteresis (shrink at N_up − 3) and τ_forest = 30 s absorb it. | pass |

### Walk E: the sampling exponent and the RELAY_CAPABLE bias

| t_ms | node | rule (file §) | computation | state after |
|---|---|---|---|---|
| E1: 0 | P | §2.3.2 *Sampled Registration* exceptions | RELAY_CAPABLE nodes register at max(0, s − 3): at s = 8 a 2^{−5} sample, 8× denser. The guardian's `ActiveCount` and `RelayCount` are raw sums over all registrations; the ACK has no field separating them; the publisher scales every registration count by 2^s. Registered = N(1 − f_rc)/2^s + N f_rc/2^{s−3} → N_est = **N · (1 + 7 f_rc)**: 1.36× at f_rc = 5.2 %, 1.7× at 10 %, 1.07× at 1 %. RelayCount: 700,000 (1 − 0.074) / 256 + 51,800 / 32 → × 256 = 1,062,656 = **1.52×** true. | biased N̂, N̂_relay |
| E2: 0 | P | §5.6 *Estimate*; §2.2.1 tiers | ŝ_m is divided by N̂: the fold threshold 0.05 is reached at a true starved fraction of 0.068 (f_rc 5 %) or 0.085 (f_rc 10 %). PoW tier and θ_join are unchanged at this N (both saturated). s is one step higher than the un-biased rule wants (8 vs 7): registrations 5,323 instead of 7,800 — harmless. | |
| E3: launch variant, 30 min | P | Ch6 §6.3.1 criterion 3 (uptime ≥ 30 min); Ch1 §1.1.5 §5.6 *Unfold* | A stream whose audience arrived in a launch crowd has every PUBLIC ≥ 20 Mbps relay cross the 30-minute uptime criterion in the same minute; each sets `RELAY_CAPABLE`, re-registers "immediately" (§2.3.2 step 4) and moves to the 8× denser sample. With 10 % of relays affected, RelayCount · 2^s jumps **1.0 → 1.7×** within one refresh period with no change in the relay population. Unfold rule: "restores a folded layer only when the relay population has grown by ≥ 50 % since the fold" → **fires** for any layer folded in the first 30 minutes. (capacity-shortfall F1 shows the unfold then re-creates the base-layer starvation.) In the steady-state scenario ages are spread and the bias is a constant factor instead. | spurious unfold at minute 30 |
| E4: 0 | V (joiner surge variant) | §5.6 *Fold rule*, τ_fold = 10 s for L0 | Under a join surge of 1 % of N per second (10⁴/s), every joiner's round-1 failure (tick C7) produces one starved re-query with `StarvedTrees = 0x3F`: StarvedCount per guardian (uniform) = 10⁴ × 0.15 × 10 = 15,000 → ŝ_m = 6.67 × 15,000 / 1.36 M = **7.3 % on every tree including T1–T2** → after τ_fold = 10 s the publisher folds the top layer for a shortage that does not exist (the swarm's L0 trees have 17 % spare; only the registered sample is full). Threshold: any sustained join rate ≥ 0.7 % of N per second (≈ 7,000/s at 10⁶; 70/s at 10⁴ with s = 0 — at s = 0 the sample is the population and round 1 fails only under real scarcity, so the false signal exists only above N₀). | discovery-driven fold |

Ticks walked: 34.

## Requirement checks

| Req | Verdict | Evidence |
|---|---|---|
| R2 Startup ≤ 1.5 s | **fail** | Usable parent list at 1,960 ms at the reference RTT (C12), ≥ 4,000 ms at 250 ms RTT; ACCEPTED 2,120 / 4,500; first frame ≈ 2.5 / 5.3 s. Decomposition: lookup 400 [1,250] + GET_PEERS 80 [250] + JOIN 160 [500] + round 1 160 [500] + **≥ 1,000 ms backoff forced by the saturated sample (F1)** + round 2 160 [500]. Without F1 the list is usable at 800 ms [2,500] — pass at the reference, fail at the worst end (flash-crowd F2, known). |
| R3 Scale 10⁶ | **fail** (worst end) / pass at 1 %/min uniform | Guardian load at 1 %/min, uniform: 239 pps in, 1.09 Mbps out, ≈ 300 verifies/s (A9) — bounded, as SOLUTION-037 claims. At the spec's own steady state (5-min sessions) with queries landing on the closest α: ≈ 13,500 pps > GLOBAL_NEW_PPS 5,000 and 13.7 Mbps egress on a 10 Mbps node (A9). The discovery *yield* at 10⁶ is ≈ 1 % per first round (C7, F1); joins complete only through gossip one τ_gossip later. |
| R7 Trust (address binding) | pass | Registration bound to the observed UDP source; dynamic PoW verified against it at tier 14; no self-declared IP in `REGISTER_PEER`, `GET_PEERS` or the Peer Record (B3). Prefix cap on k-buckets holds in the spec; absent from the implementation (F4). |
| R8 Broadcaster cost constant | pass | STORE_RECORD 67.5–74 kbps + ACKs 1.6 kbps + one guardian lookup per 30 s, independent of N (A1, D5). The source is in every tree's `GET_PEERS` sample with p ≈ 3 × 20/960 = 6 % of joiners → 10 PROBEs/s at 1 %/min, 210/s at 5-min sessions (≈ 0.5 Mbps in, 0.13 Mbps out): proportional to the join rate, small, nothing breaks. |
| R10 CDO ≤ 2 % | pass (swarm-wide) | DHT control: 20 guardians × ≈ 1.5 Mbps (in + out) ≈ 30 Mbps, plus the same bytes at the senders, against 6 Mbps × 10⁶ = 6 Tbps of video: 0.001 %. Locally at a guardian: 1.09 Mbps control against ≤ 9.6 Mbps of tree egress = 11 % of that node's uplink, unbudgeted (flash-crowd F5, known). |
| R1, R4, R5, R6, R9, R11 | not_applicable | not touched by this scenario |

## Candidate findings

Each survived the debate noted under it. "Known" items match a prior walker finding, an open ticket or an accepted residual and are listed for dedup, not as new tickets.

### F1 (High, new) — The registration sample is a fixed identity-keyed 1/2^s of the relays and absorbs every DHT-directed join, so above N₀ the DHT entry point yields saturated candidates only; every joiner's first round fails and its starved re-query feeds the fold trigger

Spec: "The guardian returns a **uniformly random sample** of up to 20 active `RELAY`-class registrations whose `AssignedTrees` intersects `WantedTrees`" (Ch2 §2.3.2 *Stream Discovery* step 2); "a peer registers **only if** (Blake3(NodeID)[0:2]) mod 2^s = 0 … a deterministic 2^{−s} sample keyed on identity" (§2.3.2 *Sampled Registration*); "Discovery needs 20 candidates, not a census."

With inputs N = 10⁶, ℓ = 0.3, M = 6, K_v = 10, s = 8, churn 1 %/min, the spec produces a sample of ≈ 960 registered relays per tree holding ≈ 9,600 slots, which receive every DHT-directed `NEIGHBOR` of the swarm; the sample's spare slots are gone 9.8 s after the stream reaches this size, and thereafter slots free in the sample at 1.6/s against 166.7 joiners/s, so ≈ 99 % of joiners' round 1 returns `SERVING, K_avail = 0` from all 12 candidates in every tree (tick C7). That is wrong because (a) the ≥ 1.0 s retry backoff then lands on every join, putting the usable parent list at 1,960 ms instead of 800 ms (C12) and the first frame beyond the 1.5 s SJL target even at the reference RTT; (b) every joiner re-queries with `StarvedTrees = 0x3F` as §2.3.3 instructs, so under a join surge ≥ 0.7 % of N per second the publisher reads ŝ_m ≥ 5 % on the L0 trees and folds the top layer after τ_fold = 10 s for a shortage that exists only in the sample (E4); (c) the same sample receives every membership `JOIN` (166.7 × 4 = 667/s onto 7,800 registered peers; 13,333/s at 5-min sessions, i.e. one eviction per 0.6 s per registered peer, turning over its Active Set faster than τ_gossip). The served fraction of first rounds is (1 − ℓ) K_v / (M 2^s): 58 % at s = 1 (N ≈ 2 × 10⁴), 15 % at s = 3, 1 % at s = 7. At s = 0 the sample is the population and the mechanism is sound.

Debate: the spec says the DHT is "the entry point" and gossip "the steady state" — but the tree-join algorithm (Ch1 §1.2.2 §2.2) takes its candidates from `get_peers` plus a pool that is empty at a joiner's first round (C5, C10), and Ch2 §2.3.2 argues the yield of the 20 records ("all 20 records are candidates for it"). SOLUTION-028's validation item (uniformity of the sampler) and SOLUTION-037's residual (join success when the joiner is out of sample) do not cover a sample that is uniform and full. Not an instance of ISSUE-061/063 (those are admission deadlocks at a saturated parent, not a saturated *directory*).

Proposed fix: let a saturated candidate answer as a directory — `PROBE_RESPONSE` carries up to n Peer Records from the responder's pool P_m for the probed tree when its `K_avail = 0`, so round 1 reaches un-sampled relays at +1 RTT instead of +1 s; alternatively rotate the sample by epoch (Blake3(NodeID ‖ ⌊LiveEdgeSegmentId / 135⌋) mod 2^s) so the join load is spread over all relays in time; and exclude a joiner's first-round failure against DHT-only candidates from `StarvedTrees` (or let the guardian set the bit only on a *re-*query). Touches Ch2 §2.3.2–§2.3.3, App D §D.4.7, Ch1 §1.2.2 §2.2, Ch1 §1.1.5 §5.6; cluster with flash-crowd F2 and capacity-shortfall F2/F3.

### F2 (Medium, new) — RELAY_CAPABLE registrations are sampled at 2^{−(s−3)} but counted and scaled at 2^s, biasing N̂ by (1 + 7 f_rc) and N̂_relay more, and firing the unfold when the 30-minute uptime criterion flips a launch cohort

Spec: "emergent-relay-capable nodes (`Flags.RELAY_CAPABLE`), which use max(0, s − 3) — an 8× denser sample" (Ch2 §2.3.2); "Only the three registration counts are scaled by 2^s" (§2.3.2 publisher table); `STORE_RECORD_ACK` carries `ActiveCount`, `RelayCount`, `PerTreeRelayCount` and no relay-capable count (§2.3.3); "restores a folded layer only when the relay population has grown by ≥ 50 % since the fold" (Ch1 §1.1.5 §5.6).

With inputs N = 10⁶, f_rc = 5.2 % of N (PUBLIC ∧ ≥ 20 Mbps ∧ uptime ≥ 30 min), s = 8, the spec produces N̂ = 1,362,688 (+36 %) and N̂_relay = 1,062,656 (+52 %) (A3, E1), and for a launch cohort crossing 30 minutes of uptime together a 1.7× step in RelayCount · 2^s within one refresh period with no relay added (E3). That is wrong because the fold threshold s_fold = 0.05 is reached only at a true 6.8–8.5 % starvation (E2), and the 1.7× step satisfies the ≥ 1.5× unfold rule, so a layer folded during a launch is unfolded at minute 30 of every such stream into the starvation that caused the fold (capacity-shortfall F1 quantifies the freeze). The spec calls these nodes "rare"; at 5–10 % of N they are not, and IPv6 hosts are PUBLIC by default.

Debate: a guardian could exclude RC registrations from the counts — then every RC relay is uncounted, which is worse for the ladder; a publisher could divide by (1 + 7 f_rc) — it has no field giving f_rc. Two implementers diverge; the reference implementation applies no RC exemption at all (F5), so it is unbiased by accident and non-conformant.

Proposed fix: `STORE_RECORD_ACK` gains `RelayCapableCount 4B`; the publisher scales it by 2^{max(0, s−3)} and the remainder of `ActiveCount`/`RelayCount`/`PerTreeRelayCount` by 2^s. Touches Ch2 §2.3.2 table, §2.3.3 ACK layout, App D registry row 0x1B, proto. Cluster with flash-crowd F1 (estimator) and capacity-shortfall F1 (unfold).

### F3 (Medium, new) — The number of `GET_PEERS` a join issues is defined two ways, 7× apart in guardian load

Spec: Ch1 §1.2.2 §2.2 step 1, per tree: `records = passive_pool[target_tree_m] + dht_interface.get_peers(stream_id, wanted_trees=bit(target_tree_m))`; Ch2 §2.3.2 *Query Discipline*: "A peer queries the DHT at join … ≈ 3,300 joins per second spread over the guardians, a few hundred queries per second each"; §2.3.3 `WantedTrees`: "stratified so that each wanted tree receives ⌈20 / popcount(WantedTrees)⌉ records".

With inputs 3,333 joins/s, α = 3, k = 20, the per-tree reading produces 6 + 1 (membership) = 7 queries per join → 3,500 `GET_PEERS`/s per guardian uniform (4.0 MB/s = 32 Mbps of responses), against 500/s under the one-query reading the load figures are written for (A6, A9). The two readings also give a joiner 20 vs 4 candidates per tree. That is wrong because the guardian budget, the `QueryCount` semantics and the ŝ_m estimator (one querier per join assumed) all depend on which it is, and two implementers will pick differently (the reference implementation sends one query with the wanted set: `request_discovery` → `dht_discover(wanted, starved)`).

Proposed fix: one sentence in Ch2 §2.3.2 — a join issues one `GET_PEERS` with `WantedTrees` = its join set (and the membership query, or fold membership into `RespFlags`), per-tree queries only for a thin pool — and the §2.2 pseudocode amended to match. Touches Ch1 §1.2.2 §2.2, Ch2 §2.3.2. Cluster with flash-crowd F5.

### F4 (Medium, impl) — The implementation's k-buckets have no per-prefix cap

Spec: "k-buckets are capped at exactly 1 of 20 per prefix unconditionally" (App B *Prefix cap*); "X admits exactly one k-bucket entry per bucket from one /24 (IPv4) or /48 (IPv6), unconditionally, so filling even one bucket needs ≥ 20 distinct prefixes" (App C §C.2.1).

`RoutingTable::insert` (`src/crates/bs-core/src/dht.rs`, lines ≈ 100–125) accepts any contact while `b.len() < self.k`; `usable_contact` (`node/dht.rs:210`) checks class and reachability only; the only prefix logic in the crate is `incentive::prefix_key` for eviction cooldowns. With 20 identities from one /24 (≈ 0.2 s of hashing at C₁ = 16) an attacker fills a target's bucket toward K_s, and the eclipse bound P_hijack ≤ f^α with f the *prefix* fraction does not hold for this implementation. Tick D4.

### F5 (Low, impl) — The implementation applies neither the RELAY_CAPABLE 2^{−(s−3)} sample nor the SOURCE_INGRESS exemption

`register_self` (`node/dht.rs:741–742`): `sampled = in_register_sample(&rec.node_id, s) || rec.flags.source`. Spec (Ch2 §2.3.2): three exceptions — source, `SOURCE_INGRESS`, and `RELAY_CAPABLE` at max(0, s − 3). At N > N₀ a NAT-blocked broadcaster's ingress relay and most relay-capable nodes are invisible to `GET_PEERS(WANT_RELAY_CAPABLE)`; no simulator scenario exceeds N₀, so it is untested.

### Known (listed for dedup, not re-filed)

- **K1** — flash-crowd F1: s ratchets and no frame tells a registrant that s changed (B1, S8).
- **K2** — capacity-shortfall F2: queries land on the closest α guardians, so median StarvedCount is 0 and the load is concentrated (A4, A9). New numbers here: at the spec's 5-min sessions the concentration puts 13,500 pps on G1–G3 against GLOBAL_NEW_PPS = 5,000.
- **K3** — flash-crowd F5: guardian discovery egress is unbudgeted (A9: 1.09 Mbps uniform at 1 %/min, 6.2 Mbps closest-α, 13.7 Mbps at 5-min sessions).
- **K4** — flash-crowd F2: SJL > 1.5 s at the worst end even without F1 (C14).
- **K5** — capacity-shortfall F3: starved re-query cadence (C9 assumes one re-query per failed round).
- **K6** — flash-crowd F7: no `REGISTER_RESPONSE` frame (B3).
- **Accepted residuals dropped:** count lag ≤ 135 s (SOLUTION-026; B7, D6); 20-prefix eclipse of a known K_s (App C; D4); sub-25 s NAT timeouts (SOLUTION-051; B5); "≈ 500 queries/s per guardian, bounded but not small" (SOLUTION-037) — superseded by K3's arithmetic rather than re-filed.

## Spec silences assumed through

- **S1** — How many `GET_PEERS` a join issues (one with 0x3F plus one membership query assumed). Filed as F3.
- **S2** — How the publisher and registrants learn that the 20 closest nodes to K_s have changed; no republish rule, no lookup cadence (30 s assumed, as the implementation). Consequence bounded by the median and by L = 0 handling (D2); not filed.
- **S3** — Whether CONE *leaf* registrants send the τ_nat keepalive, and how a membership `JOIN` reaches a CONE record (the punch is written for `PROBE` only). Assumed PUBLIC JOIN targets; +44 pps per guardian if leaves keepalive (A7). Noted by nat-and-relays; not re-filed.
- **S4** — Whether a new Active Set connection triggers an immediate `GOSSIP_EXCHANGE`. Assumed no (HyParView and the implementation give none); the first records arrive within one τ_gossip (C10). Media-pipeline F6 covers the cadence.
- **S5** — Lookup hop count at 7 × 10⁵ DHT nodes: 5 hops assumed, matching the spec's 0.3–0.5 s (C2).
- **S6** — Whether a guardian rejects a `REGISTER_PEER` from a NodeID outside the current sample. Assumed not; F1's rotation fix would need it stated.
- **S7** — RELAY_CAPABLE share of the swarm: 5.2 % of N assumed from the Ch6 §6.3.1 criteria and 1 %/min ages (E1); the finding scales as (1 + 7 f_rc).
- **S8** — A registrant re-reads s at its 90 s refresh (needed for the s = 7 → 8 step at A3 to settle; the spec gives no mechanism — K1).
