# Trace: media-pipeline-latency

Walk of one 1 s segment from the encoder to a viewer at depth 7 over 80 ms links, on the
reference ladder at the M = 6 rung. Checks R1 (glass-to-glass), R7 (verify before
forward) and R10 (control-to-data overhead). Every constant is from
`protocol/appendix_b_parameters.md` unless a section is named.

## Scenario and initial state

| Item | Value | Source |
| :--- | :--- | :--- |
| Ladder | L0 1.5 Mbps, L1 1.5 Mbps, L2 3.0 Mbps, B = 6 Mbps | Ch1 §1.2.4 §4.2 |
| Forest | M = 6: trees 1,2 = L0 stripes (0.75 Mbps declared each), 3,4 = L1 stripes (0.75), 5,6 = L2 stripes (1.5) | Ch1 §1.2.4 §4.2.1 table |
| Segment under walk | SegmentSeq 100, chunks 0..3, 250 ms each | App B *Chunk* |
| Bytes per chunk | L0 46,875 B, L1 46,875 B, L2 93,750 B; 187,500 B total | 1.5 Mbps × 0.25 s / 8 |
| Block / symbol | 16,384 B blocks, K = 16 symbols of 1,024 B, E_min = 1 | App B B.1.3 |
| Framing | f_frame = 0.085 (12 B symbol header + ~58 B QUIC/UDP/IP ≈ 70 B per symbol, plus one BLOCK_PROOF per block) | Ch1 §1.2.1 §1.3 |
| Ω | (1 + E/16)(1.085): 1.1528 at E = 1, 1.2206 at E = 2 | App B *Ω* |
| Links | RTT 80 ms on every hop (τ_prop = 40 ms), τ_queue = 20 ms | Ch1 §1.1.3 |
| Relays R1..R6 | u_v = 10 Mbps, single-tree, assigned tree 1; K_v(1) = ⌊0.9 × 10 / (0.75 × 1.1528)⌋ = ⌊10.41⌋ = 10, all slots full | Ch1 §1.2.1 §1.3 worked value "10 slots" |
| Source S | u_S = 50 Mbps; R_src = 3 × 1.5 × 1.1528 = 5.19 Mbps; K_S(1) = ⌊(50 − 5.19)/(6 × 0.75 × 1.1528)⌋ = 8 | Ch1 §1.1.5 §5.4 |
| Viewer V | depth 7 in tree 1: S → R1 → R2 → R3 → R4 → R5 → R6 → V | — |
| Loss | ρ = 1 % on every link (main walk); ρ = 5 % branch | Ch4 §4.2.2 |
| Encoder | 0.1–0.3 s; the walk uses 0.2 s and reports the range | Ch1 §1.1.3 table |
| Buffer | Δ_buffer = 3.0 s; W_pull = clamp(τ_evict + 6 SRTT, 1.5, 2.0) = clamp(0.32 + 0.48, …) = 1.5 s | Ch4 §4.3.1 |

Clock: t = 0 is the capture of the first frame of segment 100. The walk follows block
j = 0 of chunk 0 (layer L0, stripe 0, tree 1). Where the spec's model and the
spec's own slot arithmetic disagree, both numbers are shown.

## Tick table

| # | t_ms | node | rule (file §) | computation | state after |
| :-: | --: | :-- | :-- | :-- | :-- |
| 1 | 0 | encoder | Ch4 §4.1.1 (1.0 s closed GOP; 4 chunks of 250 ms) | chunk 0 covers frames [0, 250) ms | capturing |
| 2 | 250 | encoder | Ch4 §4.1.1 *Chunks* | chunk 0's last frame captured; encoder latency 100–300 ms (Ch1 §1.1.3 table) | chunk 0 encoding |
| 3 | 450 | encoder → S | Ch1 §1.1.3 table "Encoder 0.1–0.3 s" | chunk 0 bytes emitted at t = 250 + 200 (range 350–550) | S holds L0 46,875 B, L1 46,875 B, L2 93,750 B |
| 4 | 450 | S | Ch4 §4.1.1; App D §D.4.8 `LayerBlockCount`/`LayerByteLength` | L0: ⌈46,875/16,384⌉ = 3 blocks (49,152 B wire, 2,277 B padding); L1: 3 (2,277 B pad); L2: ⌈93,750/16,384⌉ = 6 (98,304 B, 4,554 B pad). BlockCount = 12, ChunkByteLength = 187,500, wire 196,608 B (+4.86 %) | blocks j = 0..11, layer-major: L0 j 0–2, L1 j 3–5, L2 j 6–11 |
| 5 | 450 | S | Ch4 §4.1.2 Merkle tree | 12 leaves → padded to 16 → depth 4; h_i = Blake3(B_i), internal Blake3(h_x‖h_y); 12 × 16 KB hashing ≈ 60 µs; ProofDepth = 4 (128 B of siblings per block) | root H_root(100, 0) |
| 6 | 450 | S | Ch7 §7.1.1; App D §D.4.8 MANIFEST | body = 4 + 32 + 4 + 1 + 1 + 2 + 8 + 32 + 2 + 4 + 1 + 1 + 2 + 3 × 6 = 112 B + 64 B sig = **176 B** per MANIFEST; Ed25519 sign ≈ 50 µs | MANIFEST(100, 0) signed; signing barrier = 250 ms (tick 2 → tick 3 at zero encoder latency) |
| 7 | 450 | S | Ch1 §1.2.4 §4.2.1 "block j travels on the tree with StripeIndex = j mod t_l" | L0 (t = 2): j' 0, 1, 2 → stripes 0, 1, 0 → **tree 1 gets j = 0, 2; tree 2 gets j = 1**. L1: tree 3 gets 3, 5; tree 4 gets 4. L2 (6 blocks): tree 5 gets 6, 8, 10; tree 6 gets 7, 9, 11 | per second (4 chunks): tree 1 = 8 blocks = 131,072 B/s = **1.049 Mbps** vs declared 0.75; tree 2 = 4 blocks = **0.524 Mbps**; trees 5, 6 = 12 blocks = 1.573 Mbps vs 1.5 |
| 8 | 450 | S | Ch4 §4.2.2 rule 2, `E = max(1, ⌈clamp(2ρK, 0.05K, 0.30K)⌉)` from each child's last `LossRate` | ρ = 0.01: 2 × 0.01 × 16 = 0.32 → clamp to 0.8 → ⌈0.8⌉ = 1 → **E = 1**, Ω = 1.1528. ρ = 0.05: 1.6 → ⌈1.6⌉ = **2**, Ω = 1.2206 | parity per link fixed for segment 100 |
| 9 | 450 | S | Ch3 §3.3.2 source pacing "≤ half the chunk period" | 12 blocks over 125 ms: block j leaves at 450 + 125 j / 12; block 0 at +0, block 2 at +21, block 11 at +115 | pacing schedule set |
| 10 | 450 | S → R1 (tree 1 stream) | App D §D.4.8; Ch7 §7.1 | MANIFEST(100, 0) 176 B on the control stream to every child of every tree (8 per tree × 6) | in flight, 40 ms |
| 11 | 450–460 | S → R1 | App D §D.4.9 BLOCK_PROOF then RAPTORQ_SYMBOL datagrams; Ch4 §4.2.3 | BLOCK_PROOF(seg 100, BlockIndex 0<<12\|0 = 0, ProofDepth 4, SenderHopDepth 0) = 140 B; 16 source symbols ESI 0–15 + 1 parity ESI 16, each 1,024 + 12 B protocol + ~58 B transport = 1,094 B; block wire ≈ 17 × 1,094 + 200 = **18.8 KB ≈ 150 kbit** | symbols of block 0 leave over the 10.4 ms pacing slot |
| 12 | 490 | R1 | Ch7 §7.1.2 `validate_incoming_manifest` | signature vs pinned PK (≈ 50 µs) → ok; not a duplicate; X_edge = 99 → window [91, 101] ∋ 100 → **ACCEPT**; X_edge := 100 (push path) | R1 forwards MANIFEST to its 10 children (tick 15) |
| 13 | 500 | R1 | Ch4 §4.1.2 "reconstruct from any ≥ 16 symbols"; SOLUTION-013 no-op decode | last symbol of block 0 arrives (460 + 40). ρ = 1 %: P(all 16 source symbols arrive) = 0.99^16 = 85.1 % → concatenate, no decode; else RaptorQ decode from 16 of 17 (14.5 %, P_decode ≥ 0.99); P(> 1 lost of 17) = 1.23 % → **cannot reconstruct** (tick 30) | block 0 held (85 + 14.5 % paths) |
| 14 | 500 | R1 | Ch4 §4.1.2 verify: Blake3(16 KB) + 4 sibling hashes ≈ 5 µs; Ch1 §1.2.2 *Depth Propagation* | H'_root == H_root(100, 0) → verified; h_R1(1) = SenderHopDepth + 1 = **1** | block 0 verified at R1, t = 500 |
| 15 | 500–520 | R1 | Ch1 §1.1.3 τ_queue = 20 ms; Ch4 §4.2.2 re-encode E per child (µs) | 10 children, each E = 1 → 17 symbols each | ready to send at 520 |
| 16 | 520 | R1 → 10 children | Ch1 §1.1.3 τ_trans = BlockSize / u_parent "13 ms at 10 Mbps" **vs** Ch1 §1.2.1 §1.3 K_v = 10 on that same 10 Mbps | bytes on the uplink for block 0: 10 × 18.8 KB = 188 KB = 1.50 Mbit → **150 ms** at 10 Mbps (fair-share interleaving: every child's last symbol at +150; child-by-child: first at +15, last at +150). Spec model: 13 ms | R1 busy 520–670 |
| 17 | 670 | R1 | Ch1 §1.2.1 §1.3 (declared B_m = 0.75) vs tick 7 | tree 1 carries 2 blocks per chunk → 2 × 150 = **300 ms of uplink per 250 ms chunk = 120 %**; tree 2 (1 block/chunk) = 60 %. Declared load would be 0.75 × 1.1528 × 10 / 10 = 86 % | R1's egress queue grows 50 ms per chunk on tree 1 (finding F4); the latency chain below assumes the declared load |
| 18 | 710 | R2 | ticks 12–14 repeated at depth 2 | last symbol of block 0 at 670 + 40; verify; h_R2 = 2. **Per-hop increment = 710 − 500 = 210 ms** (40 prop + 150 trans + 20 queue) vs the spec's 73 ms | block 0 verified at R2 |
| 19 | 920 | R3 | same | 710 + 210; h = 3 | verified at R3 |
| 20 | 1,130 | R4 | same | h = 4 | verified at R4 |
| 21 | 1,340 | R5 | same | h = 5 | verified at R5 |
| 22 | 1,550 | R6 | same | h = 6 | verified at R6 |
| 23 | 1,760 | V | Ch4 §4.3.1 "authoritative live edge = highest segment arriving on PUSH"; depth h_V = 7 ≤ D_max − 1 | first block of segment 100 arrives at depth 7 at **t = 1,760** (enc range: 1,660–1,860). Spec model: 450 + 7 × 73 = 961 (861–1,061) | X_edge(V) = 100; playout deadline for segment 100 = 1,760 + 3,000 = **4,760** |
| 24 | 565–1,315 | S | tick 9 repeated for chunks 0–3 | chunk c signed at 250 (c + 1) + 200; its block 11 leaves at +115: chunk 3's last block leaves at 1,000 + 200 + 115 = 1,315 | — |
| 25 | 2,625 | V | hop chain of ticks 11–23 applied to the last block of chunk 3 | 1,315 + 10 + 40 + 20 + 6 × 210 = 2,645; frames of chunk 3 render at 4,760 + 750 = 5,510 → **margin 2,865 ms**; PULL zone (last 1,500 ms before 5,510) untouched | segment 100 complete at V by 2,645 |
| 26 | 1,760 → 2,760 | V (and each Ri) | Ch5 §5.2.1 receipt per segment per tree, due before the first block of segment 102 | PROOF_OF_UPLOAD(seg 100, tree 1): bitmap bit = ChunkIndex × 128 + j, highest set bit 3 × 128 + 2 = 386 → BitmapLen = 49 B; frame = 184 + 49 = **233 B**; one per tree → 6 × 233 = 1.4 KB per segment per peer upstream | receipts queued; deadline 2 segments later |
| 27 | 2,000 | every peer | App B τ_gossip = 1,000 ms; Ch4 §4.3.2 | BUFFER_STATE_BITFIELD: 12 B + N; segment 100 has 12 × 4 = 48 blocks → N = 6 B per segment; span per frame unstated (assume 4 segments) → ≈ 36 B × 8 active neighbours = 0.3 KB/s | bitfields sent |
| 28 | 2,000 | R1..R6 | App D §D.4.15 ROSTER every τ_roster = 1,000 ms to every child in the tree | 12 B + 8 × 43 B (IPv4) = **356 B** per child per second (452 B IPv6); sent to leaf children too | rosters sent |
| 29 | 2,000 | every peer | Ch3 §3.2.1 "the periodic GOSSIP_EXCHANGE every peer already runs" — **no interval stated** (silence S2) | frame = 74 + 8 × 42 = 410 B (IPv4); at the implementation's cadence (τ_gossip = 1 s, `membership.rs:221`) 8 × 410 = 3.3 KB/s per peer | assumed 1 s for R10 |
| 30 | 500 (branch) | R3 | Ch4 §4.1.2 / §4.2.1: 15 of 17 symbols of block 0 arrive (P = 1.23 % at ρ = 1 %; 5.8 % at ρ = 5 %, E = 2) | fewer than K = 16 symbols → no block; Ch4 §4.3.1: PULL only "in the final W_pull before the Playout Deadline"; §4.3.3 scheduler input is the PULL zone (impl `missing_in_window`, `pull.rs:189`) | R3 forwards **nothing** for block 0 |
| 31 | 2,440 (branch) | R3 | Ch4 §4.3.1 PULL zone opens at deadline − W_pull | R3's edge moved on block 2 at ≈ 941; deadline 3,941; zone opens 3,941 − 1,500 = **2,441**; next τ_sched tick → PULL_REQUEST(MissingSymbolCount = 1) or whole block | pull issued 1.94 s after the symbols were lost |
| 32 | 2,780 (branch) | R3 | Ch5 §5.1.2 β_pull = 0.5 Mbps per unchoked slot; RTT 80 | 1 symbol: 80 + ~20 ms; whole block via BLOCK_TRANSMISSION: 80 + 18.8 KB / 0.5 Mbps = 80 + 300 = 380 ms → block 0 at R3 by ≈ 2,560–2,820 | R3 forwards block 0 to its 10 children ≈ 2.1 s late; every descendant (10^4 peers below depth 3 at k = 10) has meanwhile hit its own PULL zone and pulled the block itself: 10^4 × 18.8 KB = **188 MB of mesh traffic for one 16 KB loss** |
| 33 | per block (branch) | path S→V | binomial tail per hop, independent per hop (fresh parity each hop) | P(block pulled by V) = 1 − (1 − p)^7: ρ = 0.5 % → 2.8 %; **1 % → 9.3 %**; 2 % → 28.7 %; **5 % → 35.1 %**; 15 % (spec's cellular example, E = 5) → 45.9 % | PULL demand per depth-7 viewer at ρ = 1 %: 9.3 % × 6 Mbps = 0.56 Mbps against a 1.0 Mbps reserve on a 10 Mbps peer; at ρ = 2 %: 1.7 Mbps > reserve |
| 34 | 4,760 | V | Ch4 §4.3.1 Δ_buffer = 3.0 s behind X_edge | frame 0 of segment 100 renders at 1,760 + 3,000 = **4,760 ms** (enc range 4,660–4,860; with the Ch1 §1.1.3 table's conservative +125 ms pacing term, 4,785–4,985). Spec table: 4,000–4,200 | glass-to-glass measured |
| 35 | 4,760 → 5,760 | V | — | segment 100 plays; every block arrived ≥ 2.8 s before its deadline; no PULL on the main path | end of scenario |

### Per-hop delay, three ways

| Model | τ_prop | τ_trans | τ_queue | per hop | 7 hops | g2g (enc 0.1–0.3 s, + 0.125 pacing) |
| :--- | :-: | :-: | :-: | :-: | :-: | :-: |
| Ch1 §1.1.3 as written (16 KB / 10 Mbps) | 40 | 13 | 20 | 73 | 511 | 3.99–4.19 s |
| Full relay, child-by-child, **mean** child (K_v = 10) | 40 | 82 | 20 | 142 | 70 + 6 × 142 = 922 | 4.42–4.62 s |
| Full relay, **last** child or fair-share interleave (K_v = 10, 0.75 Mbps declared) | 40 | 150 | 20 | 210 | 70 + 6 × 210 = 1,330 | **4.79–4.99 s** |
| Full relay on the nominal 1 Mbps slice (k = 8, u = 10.2 Mbps) | 40 | 118 | 20 | 178 | 1,138 | 4.6–4.8 s |
| Full relay on a 1.5 Mbps L2 stripe (K_v = 5) | 40 | 75 | 20 | 135 | 880 | 4.3–4.5 s |

The transmission term at a full relay is K_v × Size_block × Ω / u_v, and because
K_v = ⌊(1 − r_pull) u_v / (B_m Ω)⌋ it equals ≈ 0.9 × Size_block / B_m — one block
period at the tree's declared bitrate — **independent of u_v**. A 10 Gbps super node
with 1,734 slots per 0.75 Mbps tree pays the same 150 ms per block. Only an
under-filled relay is faster; at σ_target the mean relay is full by construction.

## Requirement checks

| Req | Verdict | Evidence |
| :-- | :-: | :-- |
| R1 glass-to-glass 3–5 s, ≈ 4.0 s at reference | **fail** (on the stated 4.0 s; the 5 s ceiling holds with ≤ 0.2 s margin) | Tick 34: 4.66–4.86 s on the walked path (4.79–4.99 s with the table's pacing term), vs the spec's 4.0–4.2 s. The gap is tick 16: τ_trans = 150 ms at a full relay, not 13 ms (Ch1 §1.1.3, SOLUTION-013). Signing barrier 0.25 s, pacing ≤ 0.125 s and buffer 3.0 s check out. |
| R2 startup ≤ 1.5 s | not_applicable | no join in this walk |
| R3 scale 10^6 at depth ≤ 7 | not_applicable | depth 7 assumed |
| R4 cold start N = 2 | not_applicable | — |
| R5 resilience | not_applicable | no parent loss walked |
| R6 fairness | not_applicable | — |
| R7 no byte forwarded before verification | **pass** | Ticks 12–14 at every hop: MANIFEST signature checked against the pinned key before use (Ch7 §7.1.2 step 2), block reconstructed, Blake3 path verified against H_root(100, 0), forwarded only after (Ch4 §4.1.2, impl `node.rs:2445` `forward_block` after `on_verified`); the pull branch (tick 32) carries the inline proof in BLOCK_TRANSMISSION. No address field in any media frame. Residual: `SenderHopDepth` is session-authenticated only (SOLUTION-021, accepted). |
| R8 broadcaster cost bounded | not_applicable | source egress at K_S = 8 per tree is constant; not stressed here |
| R9 PSR ≤ 0.1 % | **undefined** | Tick 33: at ρ ≥ 2 % on every link the pulled fraction of the stream at depth 7 (28.7 %) exceeds the PULL reserve r_pull = 10 %; the spec gives no rule for pull service once the reserve is exhausted, so the PSR cannot be derived. At ρ = 1 % the demand (9.3 %, 0.56 Mbps per viewer) fits the 1.0 Mbps reserve of a 10 Mbps peer and the zone absorbs it (tick 32). |
| R10 control-to-data ≤ 2 % | **fail** at ≤ 1 Mbps per-tree bitrate, pass at 1.5 Mbps | Per child-second at a full relay on a 0.75 Mbps stripe (5.72 blocks/s): video 99,609 B; BLOCK_PROOF 801; MANIFEST 704; ROSTER 356; heartbeats 0–200; per-relay share (K_v = 10) of receipts 140, bitfields 40, GOSSIP_EXCHANGE 328 (cadence assumed 1 s) → control 2,369–2,569 B → **CDO 2.32–2.52 %**; without the undefined gossip 2.0–2.2 %; the three spec-named per-child frames alone 1.83 %. At 1 Mbps nominal (K_v = 7): 2.10 %. At 1.5 Mbps (K_v = 5): 1.81 %. Counting the 12 B symbol protocol header as control adds 1,167 B → 3.4–3.6 %; full UDP/IP/QUIC framing adds 5,643 B → ≈ 8.5 %. |
| R11 stress scenarios | not_applicable | — |

## Candidate findings

### F1 (High) The depth proof's transmission term is per link, not per child; at a full relay a block takes one block period per hop, and glass-to-glass is 4.7–5.0 s, not 4.0 s

*Spec:* Ch1 §1.1.3: "Chunk size is split into 16 KB blocks sent over a 10 Mbps active
stream link, meaning τ_trans ≈ 13 ms … L ≈ 7 · 73 ms = 511 ms"; SOLUTION-013 table:
"Accumulate ≥ K = 16 symbols ≈ 16 KB / u_link — 13 ms at 10 Mbps. Dominant term."
Ch1 §1.2.1 §1.3 worked value: "a u_v = 10 Mbps single-tree node holds K_v = 10 slots if
assigned a 0.75 Mbps L0 or L1 stripe". Ch4 §4.1.2: a relay "cannot emit a symbol of
block B_i until it holds and has verified all of B_i".

*With inputs* a 10 Mbps relay holding its 10 slots and one verified 16 KB block (18.8 KB
on the wire at E = 1), *the spec produces* 13 ms of transmission per hop, *and that is
wrong because* the relay must put 10 × 18.8 KB = 1.50 Mbit on a 10 Mbps uplink before
the last child holds the block: 150 ms (82 ms for the mean child if served one at a
time). Cut-through is forbidden by verify-before-forward, so the term is paid in full at
every hop. The relation K_v ≈ 0.9 u_v / (B_m Ω) makes it ≈ 0.9 × Size_block / B_m
whatever u_v is. Numbers: per hop 210 ms (worst) / 142 (mean) vs 73; 7 hops 1,330 /
922 ms vs 511; glass-to-glass 4.79–4.99 s (worst path, with the table's pacing term) /
4.4–4.6 s (mean) vs the stated 4.0–4.2 s. On the 0.75 Mbps base-layer stripes the
worst path is within 0.01–0.2 s of the 5 s objective, and the "margin for jitter and
one repair cycle" the §1.1.3 table claims is gone.

*Debate:* charitable reading "active stream link = per-child 10 Mbps" fails against the
spec's own σ_target (10.2 Mbps mean upload *for* fan-out 8) and slot table. The
simulator models uplink serialisation (`bs-sim/src/network.rs:259`) but reports no
latency KPI, so nothing has measured it. Not in any ticket or accepted residual
(SOLUTION-013 and SOLUTION-033 both carry the 13 ms figure as fact; SOLUTION-013's
owed measurement expects it to confirm). ISSUE-072 is about queue drops at 100 %
utilisation, a different consequence of the same uplink sharing.

*Proposed fix:* (1) restate τ_trans = K_v Size_block Ω / u_v ≈ (1 − r_pull) Size_block
/ B_m in Ch1 §1.1.3 and SOLUTION-013, and re-sum the §1.1.3 table (≈ 4.7–5.0 s).
(2) Recover the budget by ordering each block's send by child class: relay-class
children of this tree first (≈ K_v / M ≈ 2 of 10 at the single-tree rule), leaf
children after. The compounding path then sees ≈ 2 × 15 = 30 ms per hop (≈ 0.63 s at
7 hops), leaves see the full 150 ms once. Name it in Ch1 §1.2.2 and Ch4 §4.3.1.
(3) Alternatively halve Size_block (8 KB, K = 8) or state the objective as 4.7–5.0 s.

### F2 (High) The parity rule E = ⌈2ρK⌉ at K = 16 leaves 1.4–9 % of blocks per hop unrecoverable from the push, so 9–50 % of blocks at depth 7 are pulled and the PULL reserve is exceeded from ρ ≈ 2 %

*Spec:* Ch4 §4.2.2 rule 2: "E = max(1, ⌈clamp(2ρ · K, 0.05K, 0.30K)⌉) … (Example:
clean link at ρ = 0.02 → clamped to the floor → E = 1 …)"; rule 3: "If a downstream
child suffers packet loss within the measured ρ budget, it reconstructs the original
payload locally using the parity symbols, completely eliminating retransmission-induced
latency spikes." Ch4 §4.2.1: P_decode(K' = K) ≥ 0.99.

*With inputs* K = 16, E from the rule, independent symbol loss ρ, *the spec produces*
the per-hop block-failure probability P(> E of K + E lost) + decode failure:

| ρ | E | n | P(> E lost) | + decode (0.01 at K' = K) | per hop | pulled at depth 7 | E for < 0.1 % per hop |
| :-: | :-: | :-: | :-: | :-: | :-: | :-: | :-: |
| 0.5 % | 1 | 17 | 0.32 % | 0.09 % | 0.41 % | 2.8 % | 2 |
| 1 % | 1 | 17 | 1.23 % | 0.15 % | **1.38 %** | **9.3 %** | 2 (12 %) |
| 2 % (spec's "clean") | 1 | 17 | 4.46 % | 0.25 % | **4.71 %** | **28.7 %** | 3 |
| 5 % | 2 | 18 | 5.81 % | 0.17 % | **5.99 %** | **35.1 %** | 5 (31 %) |
| 15 % (spec's "cellular") | 5 | 21 | 8.27 % | 0.12 % | **8.39 %** | **45.9 %** | 10 (62 %) |

*and that is wrong because* at K = 16 the binomial spread (σ ≈ 0.4–1.6 symbols) is of
the order of the 2ρK target itself, so "twice the mean" covers one standard deviation
at best; the residual is one order below the raw loss, not zero. Per-hop losses are
independent (every relay re-encodes), so the pulled fraction compounds with depth.
PULL demand per depth-7 viewer: ρ = 1 % → 0.56 Mbps of 6 Mbps; ρ = 2 % → 1.7 Mbps;
the reserve on the reference 10 Mbps peer is 1.0 Mbps, before late-joiner backfill.
Rule 3's "completely eliminating" is false at every ρ the spec names.

*Debate:* not a tuning value owed to Ch8 (the 2ρ shape is normative; the validation
debt names only convergence of the damped loop and "whether K = 16 is right"). The
charitable reading — the PULL zone absorbs the residual — holds for latency at ρ ≤ 1 %
(tick 32) but not for the reserve budget, which is sized by r_pull = 0.10 with no term
for FEC residue. The implementation applies the same formula (`bs-media/src/fec.rs:22`).

*Proposed fix:* size E from the binomial tail at a target per-hop block-failure rate
(≈ 10^-3: E = 2 at ρ ≤ 1 %, 5 at 5 %) and say so in Ch4 §4.2.2 and App B; or raise K
(64-symbol blocks halve the relative spread) at the cost of F1's store-and-forward term;
either way add the FEC residue × depth to the r_pull inequality of Ch1 §1.2.1 *The
Two Budgets*. Touches Ω (1.15 → ≥ 1.22 at the new floor) and every σ threshold.

### F3 (Medium) A relay that cannot reconstruct a block it must forward waits for the block to age into its own PULL zone (≈ 1.5 s) before repairing it, so one push-path FEC failure costs every descendant a pull

*Spec:* Ch4 §4.3.1: "PUSH Zone … No explicit request messages are sent"; "PULL Zone
(the final W_pull before the Playout Deadline): If a child detects missing blocks due
to UDP packet drops, it switches to reactive mesh-pull mode"; "Widening the PULL zone
… starts pulling a missing block earlier". Ch4 §4.1.2 names repair only for a block
that *fails verification* ("re-pulled from an alternate neighbor"), never for one that
cannot be reconstructed. No rule gives a relay with children a forwarding urgency.

*With inputs* R3 at depth 3 holding 15 of 17 symbols of block 0 (tick 30), *the spec
produces* a pull at 2,441 ms — 1.94 s after the loss — and nothing forwarded to R3's
subtree until ≈ 2.6–2.8 s (tick 32), *and that is wrong because* every one of the
≈ 10^4 peers below R3 (k = 10, depths 4–7) reaches its own PULL zone first and pulls
the block from the mesh independently: 10^4 × 18.8 KB ≈ 188 MB of PULL traffic, charged
to PULL reserves swarm-wide, for one 16 KB loss on one link; at depth 1 the subtree is
10^5 peers. Repairing one missing symbol from the parent at once would cost 1 KB and
≈ 1 RTT.

*Debate:* this is a silence, filed because the two reasonable implementations diverge
(repair immediately when children exist vs follow the zone literally) and a consuming
budget — the PULL reserve, and F2's residue — depends on it by a factor of the subtree
size. The reference implementation follows the zone literally
(`bs-core/src/node/pull.rs:189`, `missing_in_window`; a tree with children is
*eligible* at line 211 but still only inside the window). Distinct from ISSUE-066
(whether a choked child may pull from its parent) and from F2 (how often the case
arises).

*Proposed fix:* in Ch4 §4.3.1/§4.3.3, a relay with children in tree m treats an
incomplete block of that tree as urgent on the next τ_sched tick: PULL_REQUEST
(MissingSymbolCount = K − held) to its parent first (it has the block and the symbols
are 1 KB each), bounded by one request per block per τ_sched; charge it to the PULL
reserve inequality.

### F4 (Medium) StripeIndex = j' mod t_l with an odd per-chunk block count puts two blocks of every chunk on stripe 0 and one on stripe 1, against equal declared bitrates; stripe-0 relays run at 120 % of their uplink

*Spec:* Ch1 §1.2.4 §4.2.1: "block j travels on the tree with StripeIndex = j mod t_l.
Each of those trees carries B_m = b_l / t_l"; App D §D.4.8: "BitrateKbps is the tree's
declared bitrate and is the value every slot computation uses". Ch4 §4.1.1: a layer's
bytes are "cut into 16 KB blocks, the last block of the layer zero-padded".

*With inputs* the reference L0 at 1.5 Mbps, 250 ms chunks (46,875 B → 3 blocks per
chunk, tick 4), t_0 = 2, *the spec produces* tree 1 = blocks j' = 0, 2 of every chunk
(8 blocks/s = 1.049 Mbps) and tree 2 = j' = 1 (4 blocks/s = 0.524 Mbps), both declared
750 kbps (tick 7), *and that is wrong because* a tree-1 relay sized at K_v = 10 pushes
10 × 1.049 × 1.1528 = 12.1 Mbps on a 10 Mbps uplink (tick 17: 300 ms of sending per
250 ms chunk) while a tree-2 relay idles at 60 %. With a VBR encoder (2–4 blocks, mode
3) the bias is ≈ +0.3 block per chunk on stripe 0, ≈ +21 % over the declared rate —
still beyond the 5 % headroom the implementation added for ISSUE-072. L1 (also 3
blocks) is identical; L2 (6 blocks) happens to balance.

*Debate:* ISSUE-062 charges padding; it does not address which *stripe* the odd block
lands on, and its fix (publish measured per-tree bitrate) would make tree 1's declared
rate 1,049 and halve its slot count rather than balance the stripes. The implementation
follows the rule (`bs-media/src/mapping.rs:151`) and its synthetic source rounds L0 to
exactly 3 blocks (`source.rs:22`), so the simulator runs the imbalanced forest. Not a
tuning value; the mod rule is normative.

*Proposed fix:* rotate the stripe offset per chunk, StripeIndex = (j' + ChunkIndex) mod
t_l (balances over any 2 chunks for t_l = 2, over t_l chunks in general); state it in
Ch1 §1.2.4 §4.2.1, App D §D.4.8 and `PROOF_OF_UPLOAD` bitmap semantics. Or stripe by
bytes before blocking. Pairs with ISSUE-062.

### F5 (Low) Per-child control traffic is 2.0–2.5 % of the data on the ≤ 1 Mbps trees, so the CDO ≤ 2 % target is not met at the M = 6 base layer; MANIFEST and ROSTER are in no overhead budget

*Spec:* Ch8 §8.2: "CDO = Bytes_control / (Bytes_video + Bytes_control) … Target ≤ 2.0 %".
Ch1 §1.2.1 §1.3: f_frame covers "≈ 70 bytes per 1024-byte symbol … plus one
BLOCK_PROOF per 16 KB block". App D §D.4.15: ROSTER "sent … to every child in tree
TreeID … every τ_roster"; Ch7 §7.1.1: every chunk MANIFEST reaches a peer "from up to
M parents".

*With inputs* a full 10 Mbps relay on a 0.75 Mbps stripe, *the spec produces* per
child-second: BLOCK_PROOF 801 B + MANIFEST 704 B + ROSTER 356 B = 1,861 B (1.83 %),
plus heartbeats and the per-child share of receipts, bitfields and gossip → 2,369–2,569
B against 99,609 B of symbol payload: **2.3–2.5 %** (2.10 % on the nominal 1 Mbps
slice, 1.81 % on a 1.5 Mbps stripe), *and that is wrong because* the KPI is missed on
exactly the trees that are never shed, and the two fixed-per-child terms (MANIFEST
704, ROSTER 356: 1.06 %) appear neither in f_frame nor in the Ch8 definition's list.

*Debate:* the KPI is a Ch8 measurement target, so this is Low; it is filed because the
number is derivable today and the gossip term depends on silence S2. The Ω side
(control plane outside f_frame) is the ISSUE-062 addendum; this finding is the KPI
arithmetic and the two unbudgeted frames, whose fix is different (send ROSTER on
change or only to relay-class children; one MANIFEST per child per chunk, not M).

### F6 (Low) The GOSSIP_EXCHANGE cadence is never stated, and a consuming rule ("a full gossip round") depends on it

*Spec:* Ch3 §3.2.1: "the periodic GOSSIP_EXCHANGE every peer already runs with its
active neighbours"; Ch3 §3.1.1: "A pool that stays below the floor for a full gossip
round triggers a GET_PEERS re-query". App B has τ_gossip only for bitfields.

*With inputs* two implementers, *the spec produces* re-query triggers at 1 s (the
reference implementation, `membership.rs:221`, τ_gossip) or at 30 s, and control
budgets differing by 3.3 KB/s per peer (tick 29), *and that is wrong because* the
re-query rate toward guardians (SOLUTION-037's O(N) hotspot) and R10 both hinge on it.
Fix: one App B row (and a SHUFFLE cadence; the implementation uses 5 × τ_gossip).

### F7 (Low, impl) The implementation accepts manifests up to 3 segments ahead of the live edge; the spec's lookahead is 2

*Spec:* Ch7 §7.1.2 `LOOKAHEAD = 2`; App B "Manifest lookahead 2 segments".
*Impl:* `bs-core/src/params.rs:279` `manifest_window_ahead: 3`, consumed at
`swarm.rs:228`. With X_edge = 100 the spec drops a manifest for segment 103, the
implementation accepts it. No consequence in this walk (tick 12 is inside both
windows); it widens the replay-ahead surface by one segment. Belongs with ISSUE-070's
window widening if that is the intent; otherwise one constant.

## Candidates debated and dropped

| Candidate | Why dropped |
| :-- | :-- |
| Ch3 §3.3.2 "a child at any depth sees a near-continuous trickle … with gaps far shorter than τ_ping": on tree 2 (one block per chunk) a full relay is busy 150 ms then idle 100 ms per chunk, so the heartbeat fires every chunk (4/s/child) and the child's probe PING fires with it | Heartbeat-on-idle is the rule that makes silence harmless (SOLUTION-022); cost ≈ 200 B/s per child, no eviction. Prose overstatement, no behaviour. |
| σ_target: "mean upload ≥ 10.2 Mbps → fan-out 8 at the 1 Mbps slice": ⌊0.9 × 10.2 / 1.1528⌋ = ⌊7.96⌋ = 7 | Rounding drift of 0.05 Mbps; no consequence. |
| Ch5 §5.2.3 "L … typically 8–16 bytes": with bit = ChunkIndex × 128 + j the bitmap is ≥ 3 × 16 + 1 = 49 B for any 4-chunk segment (tick 26), receipt 233 B not 192–200 | Size claim drift; receipts stay ≈ 1.4 KB/s per peer. |
| Every viewer receives each MANIFEST from M = 6 parents: 6 × 704 = 4.2 KB/s ingress, 5 copies redundant | 0.56 % of a 750 KB/s ingress; by design (Ch7 §7.1.1). Folded into F5's fix. |
| `params.rs` comment "App B still says 0.03" | App B now reads 0.085; stale comment, no code effect. |
| MANIFEST and BLOCK_PROOF travel on different QUIC streams (D.2 lists MANIFEST under control, BLOCK_PROOF on "the tree's stream"), so a block can complete before its manifest arrives | Receiver buffers until the manifest lands (176 B, sent first, reliable stream); ≤ 1 segment of state. Silence S1, no divergence. |

## Spec silences assumed through

- **S1** Ordering of MANIFEST relative to BLOCK_PROOF/symbols across QUIC streams is not
  stated; assumed the receiver holds a reconstructed block until the chunk's manifest
  is accepted (bounded by the 2-segment lookahead).
- **S2** GOSSIP_EXCHANGE and SHUFFLE cadence (F6); assumed the implementation's 1 s and
  5 s for the R10 sum.
- **S3** How many segments one BUFFER_STATE_BITFIELD advertises; assumed 4 (the 3 s
  buffer plus one).
- **S4** Whether a relay serialises a block's symbols child-by-child or interleaves
  across children; the worst-child delay is the same (tick 16), the mean differs
  (82 vs 150 ms); F1 reports both.
- **S5** When a relay repairs a block it cannot reconstruct but must forward (F3);
  assumed the PULL-zone rule applies literally, as the implementation does.
- **S6** Heartbeat frame size (QUIC PING frame vs protocol PING on the control stream);
  assumed ≈ 50 B on the wire.
- **S7** Encoder latency within 0.1–0.3 s; walked at 0.2 s, reported as a range.

## Implementation comparison (ticks checked against `src/crates/bs-core`)

| Tick | Spec | Implementation | Result |
| :-: | :-- | :-- | :-- |
| 9 | source paces a chunk over ≤ ½ period | `node.rs:2550` `pacing_slot`: block j at `window × j / n`, source only; relays forward at once | matches |
| 8 | E = max(1, ⌈clamp(2ρK, 0.05K, 0.30K)⌉) | `fec.rs:22` identical | matches (inherits F2) |
| 14 | verify, then forward | `node.rs:2445` `forward_block` called from `on_verified`; `BLOCK_PROOF` then symbols per child with that child's parity | matches |
| 7 | StripeIndex = j' mod t_l | `mapping.rs:151` `j_in_layer % stripes.len()` | matches (inherits F4) |
| 30–31 | pull in the PULL zone | `pull.rs:189` `missing_in_window` | matches literally (F3) |
| 12 | lookahead 2 segments | `params.rs:279` = 3 | diverges (F7) |
| 16 | τ_trans | simulator serialises wire bytes on the sender's uplink (`bs-sim/src/network.rs:259`) but reports no latency KPI | F1 unmeasured |
