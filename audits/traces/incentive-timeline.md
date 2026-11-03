# Trace: incentive-timeline — receipts, rank and admission over 60 segments

Walker: incentive-timeline. Spec under `protocol/` is canonical; constants from `protocol/appendix_b_parameters.md`; wire format from `protocol/appendix_d_frame_registry.md`. Implementation compared where cheap: `src/crates/bs-core/src/node.rs`, `node/incentive.rs`, `node/rank.rs`, `params.rs`.

## 1. Scenario and initial state

**Forest.** $M = 6$, reference ladder $L_0 = 1.5$, $L_1 = 1.5$, $L_2 = 3.0$ Mbps, $B = 6$ Mbps, minimax mapping $(2,2,2)$ (Ch1 §1.2.4 §4.2.1): $T_1, T_2$ carry $L_0$ at $0.75$ Mbps; $T_3, T_4$ carry $L_1$ at $0.75$; $T_5, T_6$ carry $L_2$ at $1.5$ Mbps.

**Relay R.** $u_R = 20$ Mbps, `RELAY`, `PUBLIC`, depth $h = 2$ in both assigned trees. $\text{max\_trees} = \min(6, \max(1, \lfloor 0.9 \cdot 20 / (1.1528 \cdot 6) \rfloor)) = \lfloor 2.60 \rfloor = 2$ (Ch1 §1.2.1 §1.3). Its two highest rendezvous trees are $T_1$ ($L_0$) and $T_5$ ($L_2$). The scenario's "tree 1" is $T_1$ and "tree 2" is $T_5$.

**Constants used** (App B): $r_{\text{pull}} = 0.10$, $f_{\text{frame}} = 0.085$, $\bar{E} = 1$ until a loss report arrives, so $\Omega = (1 + 1/16)(1.085) = 1.1528$ (the spec rounds to $1.15$; both are carried below). $\beta_{\text{pull}} = 0.5$ Mbps, $\tau_{\text{tft}} = 500$ ms, $\tau_{\text{drain}} = 5$ segments, preemption margin $1.25$, leaf share $\lceil 0.2 K_v \rceil$, PULL credit cap $62{,}500$ B per counterparty per segment, tree receipt cap $1.5 \times \text{BitrateKbps}_m \times 1$ s, $\lambda = 0.005$ s$^{-1}$, rank window 60 segments, rank sample 8 pairs, RTT 80 ms reference, 250 ms worst end.

**Slots.** $K_R(T_1) = \lfloor 18 / (2 \cdot 0.75 \cdot 1.1528) \rfloor = \lfloor 10.41 \rfloor = 10$; $K_R(T_5) = \lfloor 18 / (2 \cdot 1.5 \cdot 1.1528) \rfloor = \lfloor 5.20 \rfloor = 5$. Leaf share $R_{\text{leaf}}(T_1) = \lceil 0.2 \cdot 10 \rceil = 2$. PullSlots $= \lfloor 2.0 / 0.5 \rfloor = 4$ with no handover in progress. (Implementation, with the 5 % headroom of ISSUE-072: $\lfloor 17 / 1.729 \rfloor = 9$ and $\lfloor 17 / 3.458 \rfloor = 4$.)

**Children at t = 0.**

| Tree | Child | Class | Relays this tree? | Admission `RANK_PROOF` → recorded rank |
|---|---|---|---|---|
| $T_1$ | C1, C2 | RELAY | yes (bit 0 of `AssignedTrees` set) | none (free $L_0$ slot) → 0 |
| $T_1$ | C3, C4, C5 | RELAY | no (pure subscribers, assigned to $T_3$, $T_4$, $T_6$) | none → 0 |
| $T_1$ | L1 | LEAF (0x01) | no | leaf → 0 |
| $T_5$ | C7 | RELAY | no | 0.80 |
| $T_5$ | C8 | RELAY | no | 0.60 |
| $T_5$ | C9 | RELAY (10 Mbps, relays $T_3$) | no | 0.42 (144 receipts in window: 3 children × 48 segments) |

Mesh (Active Set) PULL counterparties N1–N4 reciprocate at 120 / 110 / 95 / 80 kbps. L1 never uploads anything.

**Per-segment receipt values.** $T_1$ stripe: $0.75$ Mbps × 250 ms = 23,437.5 B per chunk → 2 blocks of 16 KB (second zero-padded, Ch4 §4.1.1) → 8 blocks per segment → $\text{Bytes} = 8 \times 16{,}384 = 131{,}072$ B; cap $= 1.5 \times 750 \times 1000 / 8 = 140{,}625$ B. $T_5$ stripe: 46,875 B per chunk → 3 blocks → 12 per segment → 196,608 B; cap 281,250 B. Both under cap, so credited in full.

**Timing model.** Source emits chunk $c$ of segment $k$ at $1000(k-1) + 250c$ ms, paced over ≤ 125 ms. Per hop $d_{\text{hop}} = RTT/2 + 150$ ms block forward: 190 ms at 80 ms RTT, 275 ms at 250 ms. R (depth 2) receives chunk $c$ of segment $k$ at $T_R(k, c) = 1000(k-1) + 250c + 380$ ms (550 ms at 250 ms RTT). Children at $+190$ / $+275$ ms.

**Schedule of arrivals.** C10 → $T_5$ at segment 5; C11 → $T_5$ at 10; C12–C15 → $T_1$ at 15–18; leaf L2 → $T_1$ at 25; joiner J → $T_5$ at 30 (relay of $T_3$ with 7 children since segment 1); relay C16 assigned to $T_1$ → $T_1$ at 40; loss event in $T_5$ at segment 44; end at segment 60.

## 2. Tick table

| # | t_ms | node | rule (file §) | computation | state after |
|---|---|---|---|---|---|
| 1 | 0 | R | Ch1 §1.2.1 §1.3 *Capacity-Proportional Multi-Tree Assignment* | $\lfloor 0.9 \cdot 20 / (1.1528 \cdot 6) \rfloor = 2$ | $t_R = 2$, assigned $\{T_1, T_5\}$ |
| 2 | 0 | R | Ch1 §1.2.1 §1.3 *Slot Count* | $K(T_1) = \lfloor 18/1.729 \rfloor = 10$; $K(T_5) = \lfloor 18/3.458 \rfloor = 5$ | $K_{\text{avail}}(T_1) = 4$, $K_{\text{avail}}(T_5) = 2$, both `SERVING` |
| 3 | 0 | R | Ch5 §5.1.2 PullSlots; Ch1 §1.2.5 §5.5 leaf share; Ch1 §1.2.1 *Two Budgets* | PullSlots $= \lfloor 2.0/0.5 \rfloor = 4$; $R_{\text{leaf}}(T_1) = 2$, leaves hold 1; tree egress $6 \cdot 0.8646 + 3 \cdot 1.729 = 10.37$ Mbps of 18 | budgets: tree 10.37/18, reserve 0/2.0 |
| 4 | 380 | R | Ch1 §1.2.2 *Depth Propagation*; Ch4 §4.3.1 PUSH zone | `BLOCK_PROOF.SenderHopDepth = 1` → $h_R = 2$; chunk 0 of segment 1: 2 blocks to each $T_1$ child, 3 to each $T_5$ child | 9 children pushed; advertised depth 2 |
| 5 | 500 | R | Ch5 §5.1.2 algorithm lines 5–25; §5.1.3 slot count | ingress N1..N4 = 120/110/95/80 kbps; 9 tree children = 0 (tree edge, one direction); queue = {L1} (`ContributionScore_2s` = 0, asked for PULL); optimistic $= \min(\lceil 1/3 \rceil, 4, \lfloor 4/2 \rfloor) = 1$; regular $= 3$ | N1, N2, N3 `UNCHOKE`; N4 `CHOKE`; L1 `UNCHOKE_OPTIMISTIC` until 2,500 |
| 6 | 1,000 | R | App D §D.4.15 ROSTER | $T_1$ entries = children that relay $T_1$: C1, C2; `ChildCount = 6`; their $K_{\text{avail}}$ not yet advertised (first `PROOF_OF_UPLOAD` lands at 1,485; `NEIGHBOR` carries no $K_{\text{avail}}$) — silence S1 | roster sent with $K_{\text{avail}}$ = 0 or omitted (implementer's choice) |
| 7 | 1,485 | C1..C5, L1, C7..C9 → R | Ch5 §5.2.1 *Deadline and Consequence* | child verifies chunk 3 of segment 1 at $1000 \cdot 0 + 750 + 125 + 570 = 1{,}445$; receipt arrives $+40$ = 1,485. Deadline = first block of segment 3 at R: $T_R(3,0) = 2{,}380$. Margin 895 ms. At 250 ms RTT: receipt 1,825, deadline 2,550, margin 725 ms | 9 receipts for segment 1 held |
| 8 | 1,485 | R | Ch5 §5.2.2 §2.1 score; §5.2.3 `LossRate`; Ch4 §4.2.2 rule 2 | 6 × min(131,072, 140,625) + 3 × min(196,608, 281,250) = 1,376,256 B credited for segment 1; `LossRate` = 0 → $E = \max(1, \lceil 0.8 \rceil) = 1$; 9 Ed25519 verifies ≈ 0.45 ms | ledger: 9 receipts; $\bar{E} = 1$ |
| 9 | 2,380 | R | Ch5 §5.2.1 deadline | before pushing segment 3 chunk 0: receipt(segment 1) present for all 9 children | missed = 0 everywhere |
| 10 | 2,500 | R | Ch5 §5.1.3 rules 1–2 (end of 2 s window) | `EgressRate(L1)` = 0 < $\text{Threshold}_{\min}$ (no value in the spec; implementation 64 kbps — finding F3) → tail, 30 s cooldown keyed on NodeID and /24 | L1 `CHOKE` until 32,500; L1's $L_0$ slot untouched (choking never touches the tree) |
| 11 | 4,500 | C10 → R | Ch1 §1.2.2 Rank Admission case 1 | `NEIGHBOR(T5)` + `RANK_PROOF` rank 0.55; free slot (4 of 5) | $T_5$: {0.80, 0.60, 0.42, 0.55}; egress 10.37 + 1.73 = 12.10 |
| 12 | 9,500 | C11 → R | case 1 | rank 0.47; last free slot | $T_5$ full, $K_{\text{avail}}(T_5) = 0$, `SERVING`; egress 13.83 |
| 13 | 12,485 | N1 → R | Ch5 §5.2.2 PULL credit cap | N1 pulled 5 blocks (81,920 B) in segment 12; receipt `TreeID 0, RxFlags.PULL`; credit = min(81,920, 62,500) = 62,500 B | one PULL receipt per counterparty per segment; cap binds |
| 14 | 14,500–17,500 | C12, C13, C14, C15 → R | case 1 | relay-class pure subscribers of $T_1$, no proof → recorded rank 0; slots 7..10 | $T_1$ full at 17,500; egress $10 \cdot 0.8646 + 5 \cdot 1.729 = 17.29$ Mbps = 96 % of the 18 Mbps tree budget (implementation: 9 slots, C15 refused — ISSUE-072) |
| 15 | 24,500 | L2 (LEAF) → R | Ch1 §1.2.2 case 3 | no free slot, $T_1$ carries $L_0$, requester leaf, leaves hold 1 < 2; victims = relay-class pure subscribers {C3, C4, C5, C12..C15}, all rank 0; tie → most recent admission = C15 | `DRAIN_NOTICE(T1, DISPLACED, 0x00, Deadline 30)` → C15; `ACCEPTED(T1, HopDepth 2, PENDING 0)` → L2 (20 ≥ 10 × 0.8646 = 8.65 → overlapped) |
| 16 | 24,500 | R | Ch1 §1.2.1 *Two Budgets*; Ch5 §5.1.2 PullSlots | $T_1$ serves 11: tree egress 9.51 + 8.65 = 18.16 > 18; the 0.8646 Mbps overlap charged to the reserve; PullSlots $= \lfloor (2.0 - 0.8646)/0.5 \rfloor = 2$; queue empty (L1 in cooldown) → optimistic 0, regular 2 | N1, N2 unchoked; N3 `CHOKE` |
| 17 | 25,100 | C15 | Ch1 §1.2.2 *Drain Path* steps 2–3; Ch3 §3.3.1 | warm repair: probe ≤ 200 ms + 2 RTT (160) + first block ≈ 600 ms; `DISCONNECT_CHOKE(T1)` → R | $T_1$ = 10 (L2 in, leaves 2 of 2); C15 PSR 0 |
| 18 | 25,500 | R | TFT | no handover → PullSlots 4 | N3 re-`UNCHOKE` |
| 19 | 29,500 | J → R | App D §D.4.17; Ch5 §5.2.2 steps 1–4; Ch1 §1.2.2 case 2 | J: 7 children in $T_3$ since segment 1. Proof sent before `NEIGHBOR(T5)` (implementation; spec says after — ISSUE-067). Commit 27, nonce = `MANIFEST(28, chunk 3).MerkleRoot` (commitment-anchored — ISSUE-069). `ReceiptCount` = 7 × 27 = 189; pairs $\lceil 189/2 \rceil = 95$; $p_i = \text{Blake3}(\text{root} \| \text{nonce} \| i) \bmod 95$; 16 receipts, path $\lceil \log_2 189 \rceil - 1 = 7$ hashes; proof 8 × (2 × 250 + 224) = 5,792 B; 17 signature checks ≈ 1 ms. All sampled receipts 131,072 B (homogeneous stripe → no sampling error). $\Theta^{\text{rate}}_J = 189/60 \times 131{,}072 \times 8 = 3{,}302{,}195$ bit/s → $r_J = 0.5504$. $r_{\min}$ = 0.42 (C9). $0.5504 > 1.25 \times 0.42 = 0.525$ | case 2 fires: preempt C9 (ratio 1.31) |
| 20 | 29,500 | R → C9, J | Ch1 §1.2.2 *Drain Path* §1, *Handover budget*; App D §D.4.19, §D.4.4 | `DRAIN_NOTICE(T5, PREEMPTED, 0x00, Deadline 35)`; `ACCEPTED(T5, HopDepth 2, PENDING 0)`: $u_R = 20 \ge 10 B_m \Omega = 17.29$ → overlapped. Worst end $\bar{E} = 4.8$, $\Omega = 1.4105$: $10 B_m \Omega = 21.2 > 20$ → `PENDING` (ISSUE-061 addendum) | $T_5$ serves 6; tree egress $8.65 + 6 \times 1.729 = 19.02$ Mbps |
| 21 | 29,500 | R | Ch5 §5.1.2 PullSlots | $\lfloor (2.0 - 1.729)/0.5 \rfloor = \lfloor 0.54 \rfloor = 0$ | N1..N4 `CHOKE`; no optimistic slot |
| 22 | 29,500 | R (observation) | Ch1 §1.2.2 case 2 reads $r_{\min}$; Ch5 §5.3.3 "a current tree child (which presented a `RANK_PROOF` at admission)" | C9's actual rank now: 8 children × 50 segments + 3 × 10 = 430 receipts in its window → $430/60 \times 131{,}072 \times 8 / 6 \times 10^6 = 1.25$. R holds 0.42. Had R known 1.25: $0.5504 < 1.25 \times 1.25 = 1.56$ → `REJECTED_SATURATED`. Any child admitted without a proof (C1..C5, C12..C15 in $T_1$; every cold-start child) is held at 0 and is preempted by any $r > 0$ in an enhancement tree | finding F1 |
| 23 | 29,700 | J | Ch5 §5.2.1 deadline for a partial first segment | J receives the remaining blocks of segment 30 (chunks 2–3). First receipt due: implementation treats segment 31 as the first full segment; spec silent on the partial admission segment — silence S2 | J pushed; `child_receipts.last_receipted = 30` |
| 24 | 30,100 | C9 | *Drain Path* steps 2–3 | warm repair ≈ 600 ms; new parent delivering; `DISCONNECT_CHOKE(T5)` → R | $T_5$ = 5; C9 PSR 0; C9's $T_1$ (other tree) never affected |
| 25 | 30,500 | R | TFT | handover over → PullSlots 4 | N1..N4 `UNCHOKE`; PULL service at R lost for ≤ 1.0 s (PULL zone 1.5–2.0 s → at most one pull round delayed for neighbours) |
| 26 | 32,500 | R | Ch5 §5.1.3 | L1 cooldown ends → queue head → `UNCHOKE_OPTIMISTIC` 32,500–34,500 → fails again → cooldown to 64,500 | leaf gets repair from R 2 s in every 32 s (6 %) — ISSUE-066; implementation serves a child's repair of its own stripe regardless |
| 27 | 34,380 | R | App D §D.4.19 deadline | segment 35 reached; C9 already left → no `DISCONNECT` | — |
| 28 | 39,500 | C16 (RELAY, rendezvous-assigned to $T_1$) → R | Ch1 §1.2.2 case 4 | no free slot, $T_1$ carries $L_0$, requester relay-class → `DISCONNECT(T1, REJECTED_SATURATED)` (ISSUE-061). Implementation (`node.rs` ~1385): displaces the most recently admitted child that does not relay $T_1$ **or is not relay-class** → L2 (leaf, admitted at 24,500) → `DRAIN_NOTICE(T1, DISPLACED)` to a leaf in an $L_0$ tree | spec: C16 unparented (ISSUE-061); impl: L2 loses its base-layer slot — finding F6 |
| 29 | 44,485 | C7, C8, C10, C11 → R | Ch4 §4.2.2 rules 2 and 5; Ch1 §1.2.1 §1.3 $\Omega$ | segment-44 receipts carry `LossRate` 38 → $\rho = 38/255 = 0.149$ → $E = \max(1, \lceil \text{clamp}(4.77, 0.8, 4.8) \rceil) = 5$; $\bar{E} = (11 \cdot 1 + 4 \cdot 5)/15 = 2.067$; $\Omega = (1 + 2.067/16)(1.085) = 1.2252$; $K(T_1) = \lfloor 18/1.838 \rfloor = 9$; $K(T_5) = \lfloor 18/3.676 \rfloor = 4$ | one child over in each tree |
| 30 | 44,500 | R | Ch1 §1.2.1 §1.3 "a $K_v(m)$ that falls is honoured by releasing children through the drain path"; App D §D.4.19 `CAPACITY` | which child? no rule (silence S3). Implementation `shed_capacity`: newest admitted, non-draining, non-bridge → $T_1$: **L2 (leaf)**; $T_5$: **J** (the contributor admitted by preemption 15 s earlier). `DRAIN_NOTICE(CAPACITY, Deadline 50)` ×2. Egress during the drain $10 \cdot 0.919 + 5 \cdot 1.838 = 18.38 > 18$; overlap of CAPACITY drains is not in the reserve expression (spec silent); implementation charges $0.919 + 1.838 = 2.76 > 2.0$ → PullSlots 0 | finding F2 |
| 31 | 45,100 | J; L2 | *Drain Path* step 2 | J re-selects $T_5$ (rank 0.55 preempts any parent whose recorded $r_{\min} < 0.44$); L2 re-selects $T_1$ by case 3 elsewhere; where every $L_0$ parent's leaf share is full → `REJECTED_SATURATED`; failed rounds counted, $L_0$ never shed (Ch1 §1.1.5 §5.3) | J gone from R at ≈ 45,100; L2 still on R |
| 32 | 49,380 | R | App D §D.4.19 deadline | segment 50: L2 still attached → `DISCONNECT(T1, CAPACITY)` → L2 has no $L_0$ parent until a slot opens or the source reserve (Ch1 §1.1.5 §5.4) serves it: a freeze, not a resolution loss | R6 fails under the implementation's choice; undefined under the spec |
| 33 | 50,000 | R | TFT | both drains closed → PullSlots 4 | N1..N4 `UNCHOKE` after 5.5 s choked (CAPACITY drain + overlap charge) |
| 34 | 59,500 (segment 60) | R | Ch5 §5.2.2 $\Theta^{\text{rate}}$, $\Theta_A$ | receipts in window $[0, 60]$: $T_1$: 14·6 + 7 + 8 + 9 + 32·10 + 11·9 = 527; $T_5$: 4·3 + 5·4 + 36·5 + 15·4 = 272. Bytes $527 \times 131{,}072 + 272 \times 196{,}608 = 122{,}552{,}320$. $\Theta^{\text{rate}}_R = 122{,}552{,}320 \times 8 / 60 = 16.34$ Mbps → rank $2.72$ (media delivered ≈ 12.75 Mbps → 2.13; padded blocks are credited, ISSUE-062 covers the budget side). $\Theta_A$ (decayed, $\varphi(59\text{ s}) = 0.745$) ≈ 105 MB — read by no rule (silence S4). CPU: 799 receipt verifies ≈ 40 ms per 60 s. Control bytes at R ≈ 465 KB (receipts 198 KB, heartbeats 144 KB, rosters 75 KB, bitfields 48 KB) vs ≈ 105 MB data → 0.44 % | end |

## 3. Requirement checks

| Req | Verdict | Evidence |
|---|---|---|
| R1 glass-to-glass | not_applicable | The walk fixes the hop model (190 ms per hop at 80 ms RTT) and does not sum the end-to-end budget. |
| R2 SJL ≤ 1.5 s | not_applicable | No cold join walked. Note: J's first block arrives ≈ 200 ms after `ACCEPTED` on the overlapped handover; on a `PENDING` handover it waits up to $\tau_{\text{drain}} = 5$ segments (ISSUE-061 addendum). |
| R3 scale | not_applicable | One relay. |
| R4 N = 2 | not_applicable | |
| R5 resilience | pass | Preemption at tick 19–24: C9 re-attached warm in ≈ 600 ms, PSR 0, its $T_1$ edge untouched. Displacement at tick 15–17: same. Under the CAPACITY drain (tick 30–32) the released leaf loses playback, not resolution, when no $L_0$ parent has leaf-share room — the spec does not determine which child is released, so this case is **undefined**. |
| R6 fairness | **fail** | Tick 22: with $r_{\min}$ read from C9's admission proof (0.42) while C9's actual rank is 1.25, J at 0.55 preempts the higher contributor: contribution did not buy placement. Every child admitted into a free slot is recorded at 0 and is preempted by any $r > 0$. Leaf floor: L1 keeps its $L_0$ slot through preemption (case 4) and displacement (case 3 victims are relay-class) — pass for those two triggers; **undefined** for the CAPACITY drain (tick 30), and the implementation releases the leaf there and at tick 28. |
| R7 trust | pass | Every byte credited at R was first Merkle-verified by the child and attested under the child's Ed25519 signature; no receipt carries an address; R rejects `wrong_uploader`/`bad_signature` before credit. Third-party verification of the sampled receipts depends on NodeID → key resolution, which is ISSUE-065 (open). |
| R8 broadcaster cost | not_applicable | |
| R9 PSR ≤ 0.1 % | not_applicable | Swarm-wide metric; in this walk C9 and C15 starve 0 intervals; L2 starves from tick 32 until re-served (spec-undefined duration). |
| R10 control ≤ 2 % | pass | 0.44 % at R over 60 s (tick 34), this relay only; `RANK_PROOF` 5,792 B once per admission. |
| R11 stress | not_applicable | |

## 4. Candidate findings

Each was debated against the fixer's objections (AUDITOR-AGENT.md Step 2) before being kept.

### F1 (High, new) — Rank Admission reads a child's rank frozen at admission, so $r_{\min}$ is stale or zero and a lower contributor preempts a higher one

- **Spec:** Ch1 §1.2.2 *Rank Admission Rule* case 2: "$r_{\min}$ is the rank of the parent's lowest-ranked current child in $T_m$". Ch5 §5.2.2 last paragraph: "A parent that has held $A$ as a child for a while needs no proof to rank $A$ as a downloader; but it learns nothing about $A$'s contribution to others except through `RANK_PROOF`." Ch5 §5.3.3: "A receiver knows it for a current tree child (which presented a `RANK_PROOF` at admission)". App D §D.4.17 defines exactly two triggers: after a `NEIGHBOR(TreeID > 0)` and attached to an accusation. No rule refreshes a sitting child's rank.
- **Inputs → output:** C9 admitted at segment 0 with 144 receipts (rank 0.42); by segment 30 it holds 430 receipts in its window (rank 1.25). J presents 189 receipts (rank 0.5504). R computes $0.5504 > 1.25 \times 0.42 = 0.525$ and preempts C9. With C9's actual rank the same rule gives $0.5504 < 1.56$ → refuse. The spec produces "preempt the 1.25 contributor for the 0.55 contributor", and that is wrong because the rule exists so that "contributors drift toward the source and free-riders toward the leaves" (same section); here the reverse happens.
- **Zero-signal case:** every child admitted into a free slot without a proof (the normal cold-start path and the $L_0$ bootstrapping path the spec describes) is recorded at 0; "Leaves and newcomers rank 0, so any contributor preempts them" then applies to any long-serving child, however much it uploads now. In a steady-state swarm all recorded ranks age at once, so a forged rank of 0.01 (SOLUTION-057: identities, not bandwidth) preempts the swarm's early, shallow relays one per tree per drain window.
- **Why not already decided:** SOLUTION-023 residual risk says "a child's rank is not re-evaluated while it holds a slot … Acceptable — decay in $\Theta$ makes it preemptible within minutes." Two things are wrong with that acceptance: rank is $\Theta^{\text{rate}}/B$, which is explicitly "the undecayed 60-second sum"; and no rule anywhere recomputes a sitting child's rank, decayed or not (silence S4: the decayed $\Theta_A$ has no consumer). The acceptance rests on a mechanism that does not exist, so per AUDITOR-AGENT.md Step 2 it is re-filed.
- **Charitable reading:** "lowest-ranked current child" could mean a freshly measured rank — but the spec gives the parent no way to measure it and no trigger to ask. Two implementers diverge: hold the admission value (the implementation: `ChildLink.rank` set once at `src/crates/bs-core/src/node.rs:1494`, read by `weakest_child` in `node/rank.rs`), or demand a fresh proof before every preemption (one extra round trip inside a `NEIGHBOR` that must be answered within $\tau_{\text{sched}}$).
- **Proposed fix:** a child re-presents `RANK_PROOF` on the tree's QUIC stream on a cadence (e.g. every 30 segments) or whenever its rank changes by more than the preemption margin; the parent replaces the recorded rank on verification and treats a child whose proof is older than the rank window as rank 0 only for *upward* comparison. Touches Ch1 §1.2.2 case 2, Ch5 §5.2.2 "sent once per admission attempt", App D §D.4.17 triggers, SOLUTION-023 residual risk.

### F2 (Medium, new, impl) — A `CAPACITY` drain names no victim; the implementation releases the newest child, which here is a leaf's base-layer slot

- **Spec:** Ch1 §1.2.1 §1.3: "a $K_v(m)$ that **falls** is honoured by releasing children through the drain path (§2.2 *The Drain Path*, `DRAIN_NOTICE` reason `CAPACITY`), never by dropping them." Ch1 §1.2.2 gives a victim rule for `PREEMPTED` (lowest rank) and `DISPLACED` (lowest-ranked pure subscriber, ties by most recent admission) and none for `CAPACITY`; the other three triggers are scope *all children*.
- **Inputs → output:** tick 29–30: four $T_5$ children report $\rho = 0.149$ → $E = 5$ → $\bar{E} = 2.067$ → $\Omega = 1.2252$ → $K(T_1)$ 10 → 9, $K(T_5)$ 5 → 4. One child per tree must go. The implementation (`src/crates/bs-core/src/node/incentive.rs`, `shed_capacity`) takes the newest admitted: L2 in $T_1$ and J in $T_5$. Y is wrong because (a) L2's slot is the $L_0$ floor the spec says is "never taken from a sitting child" outside the leaf-share exception, and (b) J is the contributor that case 2 placed 15 s earlier — the release order undoes the admission order. Note also that loss in $T_5$ alone lowers $K(T_1)$, because $\bar{E}$ is the mean over all children.
- **Also silent:** whether the $K_{\text{old}} - K_{\text{new}}$ overlap during a `CAPACITY` drain is charged to the PULL reserve. The *Two Budgets* expression names "preemption / displacement overlap" only. The implementation charges it (PullSlots → 0 for the drain); a different implementer runs the link at 18.38/18 for five segments.
- **Proposed fix:** one victim rule for every partial-scope release: lowest recorded rank among children that do not relay $m$, never a leaf-class child while leaves hold ≤ $R_{\text{leaf}}(m)$ in an $L_0$ tree, ties by most recent admission; and add `CAPACITY` overlaps to the reserve expression. Touches Ch1 §1.2.1 §1.3, §1.2.2 *Drain Path*, App D §D.4.19.

### F6 (Medium, impl; cluster ISSUE-061) — The ISSUE-061 displacement as implemented can drain a leaf from an $L_0$ slot

- **Spec:** the spec has no such case (case 4 rejects C16 — that is ISSUE-061). ISSUE-061's proposed text reads "the parent has at least one child in $T_m$ that does *not* relay $T_m$ … displaces its lowest-ranked such child", which includes leaf-class children. The implementation (`src/crates/bs-core/src/node.rs` ~1385–1410) filters victims by `!assigned_trees.contains(tree) || node_class != Relay` and takes the most recently admitted.
- **Inputs → output:** tick 28: C16 (relay of $T_1$) requests full $T_1$; newest non-relay-of-$T_1$ child is L2 (leaf) → `DRAIN_NOTICE(T1, DISPLACED)` to a leaf in a base-layer tree. Wrong because Ch1 §1.2.5 §5.5 scopes displacement to relay-class pure subscribers and R6 guarantees the leaf its base layer.
- **Proposed fix:** add "relay-class" to the ISSUE-061 victim set, and order by recorded rank before recency (consistent with case 3). Belongs in the ISSUE-061 cluster.

### F3 (Medium, new) — $\text{Threshold}_{\min}$ has no value anywhere

- **Spec:** Ch5 §5.1.2: "Unchoked (PULL served) if $\text{PullRate}(B \to A) \ge \text{Threshold}_{\min}$"; Ch5 §5.1.3 rules 1 and the pseudocode "If EgressRate(p) >= Threshold_min". App B lists no such constant (grep: no `Threshold` row). AUDITOR-AGENT.md: "A constant used in a chapter but missing here is a finding."
- **Inputs → output:** tick 10: R must decide whether L1 (0 kbps) and, generally, a 40 kbps reciprocator earn a regular slot or a 30 s cooldown keyed on the /24. One implementer picks 64 kbps (the implementation, `tft_threshold_min_kbps`), another one block per window (65 kbps), another any nonzero rate; the cooldown is the sharp edge, and it is prefix-keyed, so the choice decides whether a household's second viewer is locked out of repair for 30 s. Local behaviour only — no wire effect — hence Medium, not High.
- **Proposed fix:** one App B row ($\text{Threshold}_{\min}$, with a derivation from $\beta_{\text{pull}}$ or block size) and a cross-reference from both §5.1 files.

### F4 (Low, new) — `PROOF_OF_UPLOAD` bitmap prose ("typically 8–16 bytes") contradicts its own bit layout

- **Spec:** Ch5 §5.2.3: "$L$ is $\lceil (\text{blocks in the tree's stripe of this segment}) / 8 \rceil$ rounded to the chunk layout — typically 8–16 bytes, at most 64"; same file and App D §D.4.12: "bit $= \text{ChunkIndex} \cdot 128 + j$".
- **Numbers:** a $T_1$ receipt covering chunks 0–3 sets bits {0, 1, 128, 129, 256, 257, 384, 385}; the highest set bit is 385 → $L \ge 49$. The implementation sends $L = 4 \times 16 = 64$ (`bs-wire/src/frames/incentive.rs` `BLOCKS_PER_CHUNK = 128`, `MAX_BITMAP = 64`). Frame size 248 B, not 192–200. No interoperability consequence (`BitmapLen` is explicit; the App D diagram wins), and the §D.4.17 size estimate already uses 250 B per receipt. Derived-text drift only.

### F5 (Low, new; cluster ISSUE-062) — The receipt cap's "50 % headroom" is 7 % on a 0.75 Mbps stripe once block padding is counted

- **Spec:** Ch5 §5.2.2: "$\text{Cap} = 1.5 \times \text{BitrateKbps}_m \times 1$ s for a tree receipt (… with 50 % headroom for encoder variance — ≈ 140 KB on a 0.75 Mbps stripe)". Ch5 §5.2.1: "The receipt's byte value is 16 KB × popcount". Ch4 §4.1.1: the last block of a layer in a chunk is zero-padded.
- **Numbers:** honest nominal $T_1$ receipt = 8 × 16,384 = 131,072 B against a cap of 140,625 B: 7.3 % headroom. A chunk over 32,768 B (stripe > 1.049 Mbps, +40 %) needs 3 blocks → 12 per segment → 196,608 B → clipped to 140,625: 28 % of honest bytes uncredited, where the derivation promised credit up to +50 %. At 1.5 Mbps the padded nominal is 196,608 B against 281,250 (43 % headroom), so the clipping is stripe-size dependent. The $1.5\times$ value is Chapter 8 debt (SOLUTION-041), but the *shape* mismatch — a cap in media bytes applied to a count in padded blocks — is what this notes. Fix: cap on $\lceil \text{chunk bytes} \cdot 1.5 / 16\text{ KB} \rceil \times 4$ blocks, or state the cap in blocks.

### Known, hit in this walk (already filed)

- **ISSUE-066** (tick 10, 26): the leaf L1 gets PULL repair from R for 2 s in every 32 s under the spec's cooldown; the implementation serves a child's repair of its own stripe.
- **ISSUE-067** (tick 19): proof-before-`NEIGHBOR` ordering assumed (implementation).
- **ISSUE-069** (tick 19): window and nonce anchored on the commitment (segment 27/28) assumed; the spec's verifier-edge window would reject nothing here but differs for a verifier ahead of J.
- **ISSUE-061 addendum** (tick 20): at the parity ceiling $\Omega = 1.4105$, $10 B_m \Omega = 21.2 > 20$ Mbps, so the same preemption becomes sequential (`PENDING`) and J idles up to five segments.
- **ISSUE-072** (tick 14): with the 5 % headroom the implementation holds 9 and 4 slots, not 10 and 5.
- **ISSUE-065** (tick 19): R verifies J's 16 sampled receipts only because the implementation carries signer keys in each pair.

### Dropped after debate

- Padded blocks inflate rank by 1.40× on 0.75 Mbps stripes and 1.05× on 1.5 Mbps stripes (tick 34). Dropped: the relay transmits the padding, so rank still measures bytes uploaded; the budget side is ISSUE-062.
- The receipt-deadline "choke at the next TFT cycle" reads a signal (PULL ingress) that is already zero for a tree child. Dropped: vacuous but harmless; the eviction after three segments is the real consequence.
- The ±15 % sampling error at a 1.31× ratio. Dropped: J's receipts are homogeneous here; the general case is accepted residual risk in SOLUTION-041.
- ROSTER $K_{\text{avail}}$ before a child's first receipt (S1). Dropped: one-segment staleness with no consumer that acts within a segment.

## 5. Spec silences assumed through

| # | Silence | Assumption taken | Consequence |
|---|---|---|---|
| S1 | A child's $K_{\text{avail}}$ for the first `ROSTER` before its first `PROOF_OF_UPLOAD`; `NEIGHBOR` carries none | listed with $K_{\text{avail}} = 0$ until the first receipt | one-segment staleness; not a finding |
| S2 | Receipt deadline for the partial segment in which a child was admitted | first full segment after admission is the first due (implementation) | none observed |
| S3 | Which child is released when $K_v(m)$ falls, and whether the overlap is charged to the reserve | walked both the spec (undefined) and the implementation (newest; charged) | finding F2 |
| S4 | The decayed $\Theta_A$ ($\lambda = 0.005$) is defined and consumed by no rule; `ContributionScore_2s` in §5.1.3 is neither $\Theta_A$ nor $\Theta^{\text{rate}}$ | `ContributionScore_2s` = PULL ingress over 2 s (implementation); $\Theta_A$ computed for the record only | SOLUTION-023's residual-risk argument rests on the decayed score; feeds F1 |
| S5 | Whether a sitting child ever re-presents `RANK_PROOF` | never (only the two triggers in App D §D.4.17) | finding F1 |
| S6 | Value of $\text{Threshold}_{\min}$ | 64 kbps (implementation) | finding F3 |
| S7 | Whether R's own PULLs *from* a child (a child in R's Active Set serving R repair) count as that child's ingress | counted like any mesh counterparty; none occurred in the walk | none |
| S8 | Rank of a child admitted without a proof into a free slot | 0, per "A request with no proof … has $r_{\text{new}} = 0$" | zero-signal case of F1 |
