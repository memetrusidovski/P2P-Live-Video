# Tick-by-tick walk of the BitStream protocol, 2026-11-02

Twelve scenarios were stepped on a virtual clock against `protocol/` and, where cheap, against `src/crates/bs-core`. Each walk records every tick (time, node, rule, computation, state) in its trace file under [`traces/`](traces/) and ends with a verdict on each requirement it touches. Sixteen findings were verified against the cited text and filed as ISSUE-073 to ISSUE-088; the rest are listed below with their status.

## Requirements against scenarios

The requirements are the README design goals and the Ch8 §8.2 targets: R1 glass-to-glass 3 to 5 s; R2 first frame within 1.5 s; R3 10^6 viewers at depth ≤ 7; R4 works at N = 2; R5 sub-second one-tree repair; shed resolution, never playback; R6 contribution buys placement; base layer for all; R7 verify before forward; bound identities; R8 broadcaster cost constant; R9 PSR ≤ 0.1 %; R10 CDO ≤ 2 %; R11 churn storm, injection, 75 % symmetric NAT.

| Scenario | R1 | R2 | R3 | R4 | R5 | R6 | R7 | R8 | R9 | R10 | R11 |
|---|---|---|---|---|---|---|---|---|---|---|---|
| Cold start, N = 2 | pass | undefined | n/a | **fail** | n/a | n/a | pass | pass | n/a | n/a | n/a |
| Growth from 2 to 30 relays | n/a | **fail** | pass | pass | pass | n/a | n/a | pass | pass | n/a | n/a |
| Flash crowd at N = 10,000 | n/a | **fail** | pass | n/a | n/a | **fail** | n/a | n/a | pass | pass | n/a |
| Parent loss and repair | pass | n/a | n/a | n/a | **fail** | n/a | pass | pass | undefined | pass | undefined |
| Capacity shortfall, shed and fold | n/a | n/a | n/a | n/a | **fail** | pass | n/a | pass | **fail** | n/a | n/a |
| Media pipeline and latency budget | **fail** | n/a | n/a | n/a | n/a | n/a | pass | n/a | undefined | **fail** | n/a |
| Receipts, rank and admission | n/a | n/a | n/a | n/a | pass | **fail** | pass | n/a | n/a | pass | n/a |
| NAT traversal and emergent relays | n/a | **fail** | n/a | **fail** | **fail** | pass | pass | pass | undefined | pass | **fail** |
| Adversaries | n/a | n/a | n/a | n/a | n/a | pass | pass | n/a | pass | n/a | n/a |
| Super node at K_v = 10,000 | pass | n/a | pass | n/a | n/a | pass | pass | pass | n/a | pass | **fail** |
| Discovery at N = 1,000,000 | n/a | **fail** | **fail** | n/a | n/a | n/a | pass | pass | n/a | pass | n/a |
| Implementation versus spec | pass | **fail** | n/a | n/a | **fail** | n/a | n/a | n/a | pass | **fail** | n/a |

A **fail** cell means the walk produced a number that violates the target under the spec's own rules. An undefined cell means the spec does not determine the outcome. The evidence for every cell is in the scenario sections and the trace files.

## Scenarios

### Cold start, N = 2

Trace: [`traces/cold-start-n2.md`](traces/cold-start-n2.md), 33 ticks.

- **R1 pass.** Glass-to-glass 3.49 s under strict chunk deadlines (chunk (11,1) captured 10,250 ms renders 13,740 ms) or 3.93 s under a lenient start; the spec's own one-hop sum is 0.2 + 0.25 + 0.125 + 0.07 + 3.0 = 3.65 s (Ch1 §1.1.3, Ch4 §4.3.1). Inside 3 to 5 s.
- **R2 undefined.** SJL = 930 ms if the player renders the first complete backfilled GOP within the one-segment drift tolerance; 1,740 ms if it honours the chunk deadlines the anchor sets (deadline(8,0) = 10,490 is already past at 10,930, so the first renderable GOP is segment 9 at 11,740); 4,490 ms or never if the source serves no PULL (Ch1 §1.1.5 §5.4 + Ch5 §5.1.3 PullSlots = 0). The spec fixes neither the start rule (Ch4 §4.3.1) nor the backfill provider at N = 2.
- **R4 fail.** The M = 2 slicing matrix (N_relay < 12, every stream's first rung) has no wire encoding for the {L0+L1} bundle on Tree 1: App D §D.4.8 and Ch2 §2.3.3 carry one Layer byte per tree and no LayerCount in the Stream Record, while Ch1 §1.2.4 forbids inference. The backfill path has no provider with a PULL budget at N = 2. After the only parent dies, recovery falls to DISCOVERY with an unspecified retry cadence and, under a source sequence reset, never recovers (window [52, 62] vs seq 1).
- **R7 pass.** Every block is verified against the chunk MANIFEST root signed by PK_source pinned from the Stream Record (Blake3(PublisherPubKey) = K_s) before any forwarding (tick 10,760); registrations and Peer Records carry only the guardian-observed address (ticks 170, 10,250); the dynamic puzzle binds NodeID to the observed IP. Caveat: a NAT'd node cannot produce a valid dynamic nonce for its first PING (F2), a reachability gap, not a trust gap.
- **R8 pass.** Source egress at N = 2: 6.92 Mbps push (2 × 3.0 × 1.1528) + at most 7.9 Mbps backfill transient for 2.3 s + 5.19 Mbps reserve = 20.0 Mbps ≤ 20 Mbps. K_S(m) = ⌊(20 − 5.19)/(2 × 3.0 × 1.1528)⌋ = 2 bounds direct service at two viewers; growth beyond that is ISSUE-061 (known).

Findings:

- High: The slicing-matrix entry cannot encode a multi-layer bundle, so the M = 2 rung every stream starts on is undefined on the wire. filed as [ISSUE-073](../issues/ISSUE-073-slicing-matrix-entry-cannot-encode-a-layer-bundle.md).
- Medium: The first PING must carry a dynamic puzzle bound to an address the node learns only from the PONG, so a NAT'd node never bootstraps. not verified.
- Medium: At N = 2 the only backfill provider is the source, which the spec gives no PULL budget and no unchoke slot. not verified.
- Low: The startup render rule is undefined, and the strict reading of the chunk deadlines costs 0.8 s of SJL at N = 2. not verified.
- Medium: Segment numbering after a source restart is unspecified; a reset strands every surviving viewer and every guardian holding the old record. not verified.
- Low: The implementation computes the source's per-tree slots with the relay formula instead of u_S − R_src. not verified.

### Growth from 2 to 30 relays

Trace: [`traces/growth-2-to-30.md`](traces/growth-2-to-30.md), 32 ticks.

- **R2 fail.** T5: at N = 2 the joiner anchors 3.0 s behind the live edge (Ch4 §4.3.1) and its only neighbour S 'serves no PULL_REQUESTs from the live edge' (Ch1 §1.1.5 §5.4), so no backfill; first frame at t ≈ 0.49 + 3.0 ≈ 3.5 s. Already owed to Ch8 by SOLUTION-001 (SJL at N = 2..10) and SOLUTION-023 (beta_pull backfill).
- **R3 pass.** Max depth 4 at M = 2 (T11), 3-4 at M = 3 (T20), 3 at M = 5/6 (T23, T28R). D_max = 8 never reached. The 10^6 claim is not exercised at N = 31. Impl caveat: ISSUE-072 headroom makes 8 Mbps relays hold 1 slot at B_m = 3.0, so M = 2 trees are chains (depth ≈ 6 at 11 viewers).
- **R4 pass.** theta_join = 1, GET_PEERS returns exactly S, both trees from S (K_S = 2 at 20 Mbps), warm-up 1.79 s (spec says ~1 s; 1-2 s by attach phase), coverage grant passes at 8 Mbps by 0.28 Mbps (7.2 >= 6.917). Impl: no grant (finding E), no R_src reserve (finding G).
- **R5 pass.** Every release is a warm drain except orphans whose target slots open only at the switch (T16: 3-4, T22: ≈ 8): cold re-attach ≈ 2 RTT = 0.16-0.5 s inside W_pull = 1.5-2.0 s, scoped to one tree. Shortages at M = 3 (L2), M = 5 (L1+L2 for 13 of 26), M = 6 (L2) are resolution loss; L0 deliverable at every rung (U_L0 <= 5.76 <= 8 Mbps).
- **R8 pass.** Source egress 6.92 (N = 2), 13.83 (M = 2 full), 10.38 Mbps (M = 3, 5, 6) + R_src 5.19 <= u_S = 20 at every tick, independent of N. The literal per-tree pre-emit formula at T21 would ask 22.5 Mbps (finding C) but §4.5 also says the source 'is never asked to exceed u_S'.
- **R9 pass.** No playout interval without L0 in the spec walk; worst gap is 8 cold base-layer re-attaches of 0.16-0.5 s at seg 66 (T22/T23) inside the PULL zone, repair volume 3 Mb against ≈ 20 Mbps of PULL reserve. PSR = 0 by arithmetic; the 0.1 % figure is a Ch8 measurement. Impl: the 12.5 % draw of T31 leaves one viewer >= 4.5 s without L0.

Findings:

- Medium: A fold and a resize due at the same dwell expiry are unordered; fold-first parks the swarm at 720p until N_relay grows 50 %. holds (Medium), not filed.
- Low: Pre-emit slot formula is stated per new tree and does not share the headroom across several new trees. refuted: Fixer lens: the quoted formula sentence in 4_stream_slicing_architecture.md §4.5 step 4 does read 'in each new tree', but the same section states two explicit b.
- Low: Equal-split K_S makes source slots fall at the 5 → 6 growth step, draining base-layer children with 30 % of the budget idle. holds (Low), not filed.
- Medium: Coverage grant is not implemented; an empty base-layer tree at M = 2 leaves a joiner without L0 until a relay of that tree arrives and warms. holds (Medium), not filed.
- Medium: Source recomputes slots over old and new trees at the announcement and drains base-layer children during the migration window. holds (Medium), not filed.
- Low: Source holds no base-layer reserve R_src. holds (Low), not filed.
- Low: ISSUE-072 headroom costs an 8 Mbps relay half its slots at M = 2. already known (ISSUE-072).
- High: A tree whose interior slots are held by pure subscribers cannot admit the relays assigned to it. already known (ISSUE-061).
- Medium: Fold/unfold timing under fill-in and count ramp. already known (ISSUE-071).
- Low: SJL at N = 2 is ≈ 3.5 s because the 3.0 s anchor cannot be backfilled from a source that serves no PULLs. already known (Validation owed by SOLUTION-001 (SJL at N = 2…10) and SOLUTI).

### Flash crowd at N = 10,000

Trace: [`traces/flash-crowd.md`](traces/flash-crowd.md), 34 ticks.

- **R2 fail.** Pre-media path 1.14 s (discovery 0.58 + JOIN 0.32 + probe/handshake 0.24); ACTIVE gated until L2 shed at t = 2.3 s; backfill at beta_pull 0.5 Mbps per slot with ~4 of 8 optimistic slots open within 1 s misses segment X-2; first frame at 3.3 to 4.3 s strict, 2.2 s lenient best case. Implementation gates are sjl_mean_max_s 3.6 and sjl_p95_max_s 4.5.
- **R3 pass.** After the crowd: 18 / 180 / 1,800 / 9,690 / 311 at depths 1-5, mean 3.84, max 5. Holds only for the relay-shallow converged shape (silence S7; ISSUE-061 means no rule produces that order in L0 trees).
- **R6 fail.** J at rank 1.25 (10 L1 children, 7.5 Mbps) never sends NEIGHBOR to a full L2 parent because §2.2 step 2 drops every K_avail = 0 candidate (tick T = 12,300). Leaf L is seated only via the ~3 % probe/request race; without it case 3 is unreachable (tick T = 5,900).
- **R9 pass.** No rule takes a slot from a sitting child except warm displacement (DRAIN_NOTICE, parent keeps serving to deadline, no gap). Depth-3 relays go from 8.3/10 to 10/10 children, so ISSUE-072 queueing applies; swarm-wide number owed to simulation.
- **R10 pass.** Crowd control ~2,000 x 65 KB = 130 MB over 10 s = 104 Mbps against 60-72 Gbps video = 0.17 % swarm-wide. Guardians locally 0.55 Mbps control vs 8.6 Mbps video = 6 % (see F5).

Findings:

- High: Sampling exponent s ratchets upward once N_est exceeds N_0 because 2^s is applied to registrations made under earlier s and no frame tells registrants s changed. filed as [ISSUE-080](../issues/ISSUE-080-registration-sample-exponent-ratchets-without-bound.md).
- High: Join sequence cannot render inside the 1.5 s SJL target: pre-media path alone is 1.14 s and beta_pull backfill cannot fill the first renderable segment before its 3.0 s deadline. filed as [ISSUE-082](../issues/ISSUE-082-join-sequence-cannot-meet-the-startup-target.md).
- Medium: ACTIVE is gated on all subscribed trees including the top layer, so a full top layer delays every joiner's playback by two failed rounds plus backoff. not verified.
- High: Rank Admission cases 2 (rank preemption) and 3 (leaf displacement) are unreachable: the join algorithm's step 2 filter drops every K_avail = 0 candidate before a NEIGHBOR is sent. filed as [ISSUE-083](../issues/ISSUE-083-preemption-and-displacement-are-unreachable-from-the-join-algorithm.md).
- Medium: Guardian GET_PEERS egress is charged to neither upload budget, so a 10 Mbps guardian's uplink exceeds 100 % during the crowd. not verified.
- Low: Implementation join retry backoff is 250 ms where the spec requires at least one segment period (1.0 s). not verified.
- Low: REGISTER_RESPONSE appears in the Ch2 §2.3.2 diagram but has no frame in Appendix D, so a registrant cannot distinguish a dropped from an accepted registration and nothing carries the current s. not verified.

### Parent loss and repair

Trace: [`traces/parent-loss-repair.md`](traces/parent-loss-repair.md), 47 ticks.

- **R1 pass.** Playout stays 3.0 s behind a live edge that trees 1,3,4 keep advancing during the tree-2 repair; the repaired branch is one hop deeper (orphans at depth 5 <= 7 at N = 1,000). Ch1 §1.3 core loop line 36, Ch4 §4.3.1.
- **R5 fail.** (a) Repair complete 0.32 / 0.61 / 1.17 s after the last packet at 20 / 80 / 250 ms RTT (tau_evict 240/320/490 + 10 ms select + 2 RTT + next paced block); spec's 0.28/0.48/0.99 omit the one-way arrival offset; with RFC 9002 RTTVAR seeding (SRTT/2) tau_evict at 250 ms is 950 ms and repair ~1.6 s: sub-second holds only to ~150 ms RTT. (b) Scope to one tree holds at every tick (Ch1 §1.3 Repairing is per tree). (c) 'resolution lost, never playback' is violated for >= 15 s after a 30 % storm at the u_relay = 10 Mbps margin: 54 of 760 peers per tree unparented, L0 on one 1.5 Mbps tree at M = 4 freezes until the fold (tau_fold 10 s + 5-segment window): accepted transient per Ch1 §1.2.4 §4.2.1 / SOLUTION-058.
- **R7 pass.** Every pulled gap block is verified against the chunk MANIFEST before use (BLOCK_TRANSMISSION carries the inline Merkle proof, Ch4 §4.3.1); ROSTER addresses are the parent's observed values (App D §D.4.15); NEIGHBOR carries no address (App D §D.4.3).
- **R8 pass.** The source takes no part in the repair; R_src = 3 x 1.5 x 1.15 = 5.2 Mbps and K_S = floor(44.8/6.9) = 6 are unchanged by the loss or the storm (Ch1 §1.1.5 §5.4).
- **R9 undefined.** Single loss: 0 if the orphan holds >= 2 unchoked PULL slots; with one 0.5 Mbps slot 2 of 8 gap blocks (80 ms) and 7 of 14 (250 ms) miss their deadline, a freeze of 1-3 chunks per event on an L0 tree. Storm: 0.002 % with a 600 ms dead-candidate handshake timeout, 0.15 % with a 10 s one; the timeout is not in the spec (Ch3 §3.3.2 line 17).
- **R10 pass.** During repair: ROSTER 98 B/s/child (6.3 kbps at 8 children = 0.04 % of 16 Mbps), heartbeats once per chunk per child (pacing <= 125 ms leaves >= 125 ms idle > tau_ping): 15 kbps = 0.1 %, bitfields 8 x 40 B/s, PULL_REQUEST 15 B each; total < 0.5 % << 2 % (Ch8 §8.2).
- **R11 undefined.** Scenario A is absorbed without a slot shortage only if pre-storm slots/children >= 1/0.7 x (1 - 0.3 N_relay/N) = 1.43, i.e. u_relay >= 11.5 Mbps at ell = 0.2, M = 4; at 10 Mbps 7 % lose L0 for >= 15 s. Starvation from the repair path depends on the unstated handshake timeout (0.002 % vs 0.15 %) and on nothing stopping two unparented relays from adopting each other. Ch8 §8.3 A states neither precondition.

Findings:

- High: tau_deputy = 45 ms expires before a sibling can answer on any path over ~22 ms RTT, so the Deputy election never succeeds at the spec's reference RTT. filed as [ISSUE-074](../issues/ISSUE-074-deputy-timer-expires-before-a-sibling-can-answer.md).
- Medium: The PULL zone is sized for repair latency but one beta_pull slot cannot move the bytes a 250 ms repair leaves behind before their deadlines. not verified.
- High: A parent that heartbeats but never delivers is never evicted, and an unparented relay may still accept children, so orphaned relays can adopt each other and starve a subtree indefinitely. filed as [ISSUE-075](../issues/ISSUE-075-heartbeating-parent-that-never-delivers-is-never-evicted.md).
- Medium: No timeout is defined for the QUIC handshake to a dead passive-set candidate, and the fallback loop is sequential. not verified.
- Low: The tau_evict table rests on an RTTVAR the spec never states, and at 250 ms the W_pull formula exceeds its own 2.0 s clamp. not verified.
- Low: The multi-deputy partition and surplus rules do not exempt the deputies themselves, so a literal reading lets two deputies attach under each other. not verified.
- Medium: The reference implementation has no ROSTER or Deputy election and repairs through a full probe round instead of the fast path. not verified.
- Medium: Base-layer freeze for the starved fraction after a 30 % storm at the M = 4 minimax rung until the fold fires. already known (Ch1 §1.2.4 §4.2.1 accepted transient; SOLUTION-058 (minimax ).
- Medium: Gap-repair PULLs after a parent loss are governed by the Tit-for-Tat choke and the 30 s optimistic cooldown. already known (ISSUE-066).

### Capacity shortfall, shed and fold

Trace: [`traces/capacity-shortfall.md`](traces/capacity-shortfall.md), 33 ticks.

- **R5 fail.** Base layer frozen for 83 of 500 peers (16.6 %) from t=0.2 s to 15.24 s before the first fold (per-tree supply 2+83*5=417 of 500 at 750 kbps stripes, 5 Mbps < 5.75 Mbps threshold; Ch1 §1.1.5 §5.1, §5.6; accepted transient in SOLUTION-058). Frozen again for 123 of 750 peers (16.4 %) for 30.3 s at every growth-gated unfold (2+125*5=627 of 750; dwell blocks the re-fold until t=150 s; finding F1). The shed itself is clean: V lost only L2 at 1.32 s while T1..T4 kept flowing.
- **R6 pass.** V (rank 0) loses L2 slots to ranked joiners under Rank Admission case 2 and keeps L0/L1 (Ch1 §1.2.2). Leaf share ceil(0.2*5)=1 per L0 relay = 83 displacement slots per L0 tree, so a 0-upload device obtains L0 by displacing a pure subscriber (case 3). Leaf bound (Ch1 §1.2.5): pre-fold 1-5.175/4.5 < 0, post-fold 1-3.45/4.5 = 23 %.
- **R8 pass.** Source egress 4*2*0.8625+2*1*1.725 = 10.35 + R_src 5.175 = 15.5 Mbps before the fold; 6*4*0.575 = 13.8 + 5.175 = 18.98 Mbps after; both <= u_S = 20 Mbps and independent of N (Ch1 §1.1.5 §5.4 K_S formula). The reserve covers 6 stripe-slots (3 per L0 tree) against 83 starved per tree: bounded cost, marginal coverage, as §5.4 states.
- **R9 fail.** First fold: 83 peers starved 15 s in a 600 s stream = 83*15/(500*600) = 0.42 % > 0.1 %. Each unfold cycle: 0.164 * 30 s per 1.5x growth step; at one step per 300 s = 1.6 % (Ch8 §8.2). Caveat: pre-fold starved peers never reached ACTIVE, so a literal reading of PSR (playout intervals of a playing client) makes them an SJL failure; the unfold freeze hits playing peers and counts under any reading.

Findings:

- High: Growth-gated unfold re-creates the base-layer starvation that caused the fold whenever relays and viewers grow together. filed as [ISSUE-076](../issues/ISSUE-076-unfold-trigger-re-creates-the-base-layer-freeze.md).
- Medium: The median over guardians of StarvedCount is zero when GET_PEERS lands on the closest alpha guardians, so the fold never fires. not verified.
- Medium: The starved re-query cadence is defined three ways and both the fold estimate and guardian load depend on it. not verified.
- Low: The source reserve has no trigger threshold, no release rule and no steering, and the reference implementation withholds no reserve at all. not verified.
- Low: Implementation retry backoff base is 250 ms, below the 1.0 s floor the spec requires. not verified.
- Medium: Relays assigned to a saturated tree are UNPARENTED and not warm at the fold, so the one-round join claim fails. already known (ISSUE-061 (and the fill-in length in ISSUE-071 item 3)).
- Low: Fold dwell from stream start and measurement from Effective + D_max shift the first fold and the re-fold. already known (ISSUE-071).
- Low: Slot headroom of 5 % doubles the pre-fold base-layer starvation in the implementation. already known (ISSUE-072).

### Media pipeline and latency budget

Trace: [`traces/media-pipeline-latency.md`](traces/media-pipeline-latency.md), 35 ticks.

- **R1 fail.** Glass-to-glass at depth 7 on the walked 0.75 Mbps base-layer stripe is 4.66-4.86 s (4.79-4.99 s with the Ch1 §1.1.3 table's +125 ms pacing term) against the stated 4.0-4.2 s. Cause: per-hop transmission at a full relay is K_v x 18.8 KB / u_v = 150 ms (10 slots on 10 Mbps), not the 13 ms of Ch1 §1.1.3 / SOLUTION-013; 7 hops = 1,330 ms vs 511 ms. Signing barrier 0.25 s, pacing <= 0.125 s and buffer 3.0 s check out. The 5 s ceiling holds with <= 0.2 s margin; the claimed margin for jitter and one repair is gone.
- **R7 pass.** At every hop (ticks 12-14) the MANIFEST signature is checked against the pinned key before use (Ch7 §7.1.2), the block is reconstructed from >= 16 symbols, Blake3 + 4 sibling hashes are checked against the chunk root, and only then re-encoded and forwarded (Ch4 §4.1.2; impl node.rs:2445 forward_block from on_verified). Pull branch carries the inline proof in BLOCK_TRANSMISSION. No media frame carries a self-declared address. Residual: SenderHopDepth is session-authenticated only (SOLUTION-021, accepted).
- **R9 undefined.** Per-hop FEC residue at K=16: 1.38 % (rho 1 %), 4.71 % (rho 2 %), 5.99 % (rho 5 %); pulled fraction at depth 7 = 9.3 / 28.7 / 35.1 %. At rho >= 2 % on every link PULL demand (1.7 Mbps per viewer) exceeds the 1.0 Mbps reserve of a 10 Mbps peer, and the spec gives no rule for pull service once the reserve is exhausted, so PSR cannot be derived. At rho = 1 % the demand (0.56 Mbps) fits the reserve and the PULL zone absorbs it.
- **R10 fail.** Per child-second at a full relay on a 0.75 Mbps stripe (5.72 blocks/s): video 99,609 B; BLOCK_PROOF 801 + MANIFEST 704 + ROSTER 356 = 1,861 B (1.83 %); plus heartbeats 0-200 and the K_v=10 share of receipts 140, bitfields 40, GOSSIP_EXCHANGE 328 (cadence undefined, impl 1 s) = 2,369-2,569 B -> CDO 2.32-2.52 %. Nominal 1 Mbps slice: 2.10 %. 1.5 Mbps L2 stripe: 1.81 % (pass). Counting the 12 B symbol protocol header adds 1,167 B -> 3.4-3.6 %; full UDP/IP/QUIC framing -> ~8.5 %.

Findings:

- High: The depth proof's transmission term is per link, not per child share; a full relay spends one block period per hop and glass-to-glass is 4.7-5.0 s, not 4.0 s. filed as [ISSUE-077](../issues/ISSUE-077-propagation-term-is-per-link-not-per-child-share.md).
- High: E = ceil(2 rho K) at K = 16 leaves 1.4-9 % of blocks per hop unrecoverable from the push, so 9-50 % of blocks at depth 7 are pulled and the PULL reserve is exceeded from rho ≈ 2 %. filed as [ISSUE-078](../issues/ISSUE-078-parity-rule-leaves-a-percent-of-blocks-per-hop-unrecoverable.md).
- Medium: A relay that cannot reconstruct a block it must forward waits for the block to reach its own PULL zone (~1.5 s) before repairing it, so one push-path FEC failure costs every descendant a pull. not verified.
- Medium: StripeIndex = j' mod t_l with an odd per-chunk block count puts two blocks of every chunk on stripe 0 and one on stripe 1 against equal declared bitrates; stripe-0 relays run at 120 % of their uplink. not verified.
- Low: Per-child control traffic is 2.0-2.5 % of the data on the <= 1 Mbps trees, so the CDO <= 2 % target is not met at the M = 6 base layer; MANIFEST and ROSTER are in no overhead budget. not verified.
- Low: The GOSSIP_EXCHANGE cadence is never stated, and a consuming rule (a full gossip round) depends on it. not verified.
- Low: The implementation accepts manifests up to 3 segments ahead of the live edge where the spec's lookahead is 2. not verified.

### Receipts, rank and admission

Trace: [`traces/incentive-timeline.md`](traces/incentive-timeline.md), 34 ticks.

- **R5 pass.** Preempted child C9 re-attached warm in ~600 ms (probe 200 + 2 RTT 160 + first block) with 0 starved intervals and its T1 edge untouched; displaced C15 the same. The CAPACITY drain victim is spec-undefined (F2), so the released leaf's playback outcome there is undefined.
- **R6 fail.** Ch1 §1.2.2 case 2 reads r_min from C9's admission proof (0.42) while C9's actual rank is 1.25; J at 0.5504 > 1.25 x 0.42 = 0.525 preempts the higher contributor (would be refused at 0.5504 < 1.56). Leaf L1 keeps its L0 slot through preemption and displacement, but the spec names no victim for a CAPACITY drain and the implementation releases the leaf L2 (K(T1) 10 -> 9 at Omega 1.2252) and displaces it for an arriving T1 relay.
- **R7 pass.** Every credited byte was Merkle-verified by the child and attested under the child's Ed25519 signature before credit (Ch5 §5.2.1, §5.2.3); receipts carry no address; third-party verification of sampled receipts rests on ISSUE-065 (open).
- **R10 pass.** Control at R over 60 s ~465 KB (799 receipts x 248 B = 198 KB, heartbeats 144 KB, rosters 75 KB, bitfields 48 KB) vs ~105 MB data = 0.44 %; one RANK_PROOF of 5,792 B per admission.

Findings:

- High: Rank Admission reads a child's rank frozen at admission, so r_min is stale or zero and a lower contributor preempts a higher one. filed as [ISSUE-079](../issues/ISSUE-079-rank-admission-reads-a-rank-frozen-at-admission.md).
- Medium: A CAPACITY drain names no victim and no overlap charge; the implementation releases the newest child, which here is a leaf's base-layer slot and the just-admitted contributor. not verified.
- Medium: The ISSUE-061 displacement as implemented (and as its proposed text reads) can drain a leaf-class child from an L0 slot. not verified.
- Medium: Threshold_min is consumed by the unchoker and the optimistic-slot promotion but has no value anywhere in the spec. not verified.
- Low: PROOF_OF_UPLOAD bitmap prose ('typically 8-16 bytes') contradicts its own bit layout, which needs at least 49 bytes for a four-chunk receipt. not verified.
- Low: The tree receipt cap's '50 % headroom' is 7 % on a 0.75 Mbps stripe once zero-padded blocks are counted. not verified.
- Medium: A leaf-class child gets PULL repair from its parent only 2 s in every 32 s under the optimistic cooldown. already known (ISSUE-066).
- Medium: RANK_PROOF window and nonce anchoring assumed on the commitment, not the verifier's live edge. already known (ISSUE-069).
- Low: RANK_PROOF ordering relative to NEIGHBOR assumed proof-first. already known (ISSUE-067).
- Medium: At the parity ceiling the same preemption becomes a sequential handover and the admitted contributor idles up to five segments. already known (ISSUE-061 (addendum)).

### NAT traversal and emergent relays

Trace: [`traces/nat-and-relays.md`](traces/nat-and-relays.md), 44 ticks.

- **R2 fail.** Cone viewer V_a: first verified GOP ≈ 1.1–1.3 s (pass, Reading B only). Bridged symmetric viewer V_b: ≈ 4.7 s after play (b17) vs 1.5 s; impl's own gate admits sjl_mean_max_s = 8.0 for this population. Under literal pseudocode (Reading A) with CONE-only samples V_a never joins T₁.
- **R4 fail.** c3: RELAY_CAPABLE 30-min uptime has no defined clock; on stream-membership reading a NAT-blocked source at N = 2 never binds. c5: at the 20 Mbps floor with n_ingress = 1, K_R = ⌊(18 − 5.175)/6.9⌋ = 1 < 2 → rejected; sole-relay floor is 21.1 Mbps (M = 2), 28.75 Mbps (M = 6). c1: SYMMETRIC source must register RELAY-class and may not.
- **R5 fail.** b20: repair of a lost bridge = eviction 320 ms + GET_PEERS 80 + session 160 + probe 200 + NEIGHBOR 160 + BIND 40 + NEIGHBOR 80 = 1,040 ms, plus relay warm-up ≤ 1,000 ms → 1.3–2.4 s after loss vs W_pull = 1.5 s at 80 ms RTT; L₀ freezes at the worst end. Non-bridged repair τ_evict + 2 RTT = 480 ms passes.
- **R6 pass.** V_b (cannot accept inbound) receives L₀ via the bridge (b16–b17) and may earn PULL standing outbound (Ch1 §1.2.5).
- **R7 pass.** Forwarded punch carries referrer-observed address (a9/b6); PoW bound to observed source (a4); bridge relay decodes/verifies/re-encodes before pushing (b14/b17). Caveat filed as F5/F10: the publisher identity behind SOURCE_INGRESS is not verifiable from any frame, but content remains manifest-signed so no unverified byte is forwarded.
- **R8 pass.** c7, c10, c11: source egress = n_ingress × ΩB = 6.9 or 13.8 Mbps, independent of N.
- **R9 undefined.** Bridged-peer PSR owed to Ch8 (SOLUTION-050 validation owed); b20 worst end implies PSR > 0 for bridged L₀ on each relay loss.
- **R10 pass.** τ_nat keepalive 20 × 188 B / 25 s = 1.2 kbps per CONE relay; punch ≈ 640 B per CONE candidate per round; RELAY_PROPOSAL + RELAY_BIND = 112 B per bridge: all ≪ 2 % of a 0.75 Mbps slot.
- **R11 fail.** Scenario C (75 % symmetric) per 133 peers: 13 PUBLIC nodes give 67 PUBLIC slots per tree vs 100 symmetric per tree; 178–396 bridge edges per 100 private leaves; 31 bridges per 20 Mbps relay → 6–13 RELAY_CAPABLE nodes = 46–100 % of all PUBLIC nodes at a 10 Mbps reference mean upload. Also the spec's own punch ordering makes SYMMETRIC → CONE 0 % (b9), not the matrix's 95 %/10 %. Scenarios A, B not applicable.

Findings:

- High: Punch procedure sends the PROBE before the target's PING can open the mapping; a SYMMETRIC requester can never learn when to re-send. filed as [ISSUE-084](../issues/ISSUE-084-punch-requester-probes-before-the-mapping-opens.md).
- Medium: No rule says when a NAT-blocked peer asks for a bridge, and a mixed round (saturated + timeouts) has no classification. not verified.
- Medium: Ingress-relay cold start at N = 2 depends on the first viewer meeting the 30-minute uptime criterion, whose clock is undefined. not verified.
- Medium: The ingress-relay floor '20 Mbps suffices at M = 2' is computed with two relays already bound; the first relay is bound alone and fails K_R(m) ≥ 2. not verified.
- High: RELAY_PROPOSAL(SOURCE_INGRESS) has no signature field and no rule binds the session identity to PK_Publisher, so a relay cannot verify the proposer is the broadcaster. filed as [ISSUE-085](../issues/ISSUE-085-source-ingress-proposal-carries-no-publisher-signature.md).
- Medium: The delegated base-layer pool has no trigger: the starvation count reaches the source, the capacity sits at the ingress relay, and no frame connects them. not verified.
- Medium: RxFlags.RELAYED is asserted by the downloader and checked by nobody, so the ×3 weight and the 'never more than 2×' bound are unenforceable at a verifier. not verified.
- Medium: A bridged peer's repair is a full bridge cycle, which at the worst end exceeds the PULL zone. not verified.
- Low: Scenario C arithmetic: bridge supply at the 20 Mbps floor cannot meet 75 % symmetric demand at the reference upload. already known (SOLUTION-032 and SOLUTION-050 'Validation owed (Chapter 8): ).
- High: Implementation authenticates the broadcaster behind SOURCE_INGRESS from a self-declared, unsigned Peer Record flag. not verified.
- Medium: A saturated answer resets silent_rounds, so a NAT-blocked peer never asks for a bridge while any PUBLIC relay in its sample is full. not verified.
- Low: Implementation's ingress-relay admission omits the R_src/n_ingress term the spec subtracts. not verified.
- Low: A NAT-blocked source must register as RELAY-class and may not. not verified.

### Adversaries

Trace: [`traces/adversarial.md`](traces/adversarial.md), 30 ticks.

- **R7 pass.** Attack (a) tick 102 ms: V reconstructs the injected block, Blake3-verifies against the signed MANIFEST root, fails, and drops it before any RAPTORQ_SYMBOL leaves V; verify-before-forward (Ch4 §4.1.2) holds per hop, so a child of V in T3 receives zero bytes of the forged block. Attack (b): identities are bound to the observed IP and capped per /24. Spec-level pass; impl weakens it (F6, F7).
- **R11 pass.** Attack (a): a corrupted block never propagates past the first honest relay. Cost per forgery at the receiver is ~5 us hashing plus one per-tree CHURN_REPAIR (~0.4 s on the 80 ms path), the other 5 trees keep forwarding (no cascade). Spec pass; the reference impl admits 3 forged blocks before eviction (F7).
- **R6 pass.** Attack (b): the diversity-relative cap max(1,ceil(K/min(P_obs,20))) floors a one-/24 Sybil to 1 L0 child slot per honest parent per tree; L0 is never shed and never taken from a sitting child (App B leaf share), so a non-uploading viewer keeps its base-layer tree edge.
- **R9 pass.** Attack (a): the per-tree repair completes inside W_pull before the 3.0 s deadline on both paths - ~2.3 s vs 3.0 s at 80 ms (W_pull 1.5 s), ~2.9 s vs 3.0 s at 250 ms (W_pull 2.0 s); PSR contribution 0.

Findings:

- Low: Verification-failure consequence is specified twice, differently, with no threshold. holds (Low), not filed.
- High: Diversity-relative admission cap (the operative Sybil/eclipse defence) is not applied at k-buckets, Active Set or parent set in the implementation. filed as [ISSUE-086](../issues/ISSUE-086-prefix-caps-not-applied-in-the-implementation.md).
- Medium: Block-forging parent evicted only after 3 blocks and banned permanently; forged MANIFEST only logged - both diverge from the spec ban rule. holds (Medium), not filed.

### Super node at K_v = 10,000

Trace: [`traces/super-node-obligations.md`](traces/super-node-obligations.md), 29 ticks.

- **R1 pass.** One super-node hop at 94 % utilization adds 13 us to its first child and 78 ms to its last (3 blocks/chunk, 5,217 children): ~4.08 s at depth 1. The per-child serialization term is ISSUE-072 / media-pipeline-latency F1.
- **R3 pass.** 10 Gbps node holds 8,694 children over six trees (5,217 in one 1.5 Mbps tree). Every spec obligation is O(k): 10,000 Ed25519 verifies/s = 0.25-1.0 core; ROSTER 41 Mbps = 0.23 %; allowlist 10,100/65,536 = 15 %; ledger 140 MB; c_a_eff = 64; PullSlots 2,000. Nothing O(k^2) in the spec; one O(k x ledger) path in the implementation (F4).
- **R6 pass.** Leaf share ceil(0.2 x 5,217) = 1,044 L0 slots; prefix cap 261 per /24 per tree; rank 1,335-2,560 earns all six trees.
- **R7 pass.** decode -> Blake3 (5 us) -> re-encode -> push on each of 72 inbound blocks/s; roster addresses are the parent's observed values.
- **R8 pass.** Nothing at the super node is charged to the source; the source's per-child work is at K_S(m) = 4 per tree at u_S = 50 Mbps, M = 4 (16 receipts/s, 16 rosters/s).
- **R10 pass.** 1.85 % at k = 10,000 counting BLOCK_PROOF (24.5 MB/s), MANIFEST (9.7), ROSTER (5.1), bitfields (0.02) against 16.7 Gbps of symbols; 2.09 % if heartbeats fire every chunk; 0.7 % if Merkle proofs count as framing. KPI does not say which.
- **R11 fail.** F1: the XDP allowlist bucket is per source IPv4 address; ~18-90 children behind one CGNAT address saturate 2,000 pps with ACKs/receipts/probes, and the first disconnect among them deletes the shared entry so the rest fall to the 50 pps unknown tier.

Findings:

- High: XDP allowlist keyed by IPv4 address: CGNAT children share one 2,000 pps bucket and lose the entry when any one of them disconnects. filed as [ISSUE-087](../issues/ISSUE-087-xdp-allowlist-keyed-by-address-shares-one-bucket-across-a-cgnat.md).
- Medium: PULL unchoker electorate is the Active Set (<= 64) while PullSlots at 10 Gbps is 2,000; choke state of children outside the Active Set is undefined. holds (Medium), not filed.
- Low: Half-period source pacing leaves >= 125 ms idle per chunk on under-filled links, so heartbeats fire every chunk and the child probe races them, against the stated zero cost. holds (Low), not filed.
- High: Implementation: on_receipt scans the whole receipt ledger three times per receipt, O(k x ledger) at a super node. filed as [ISSUE-088](../issues/ISSUE-088-receipt-ledger-scanned-three-times-per-receipt.md).
- Medium: Implementation: heartbeats and probes are signed plain-UDP PINGs with a 152 B validation block, verified and answered with a signed PONG. holds (Medium), not filed.
- Low: Implementation: bitfield gossip goes to every tree child instead of the scaled Active Set. holds (Low), not filed.

### Discovery at N = 1,000,000

Trace: [`traces/dht-at-scale.md`](traces/dht-at-scale.md), 34 ticks.

- **R2 fail.** Usable parent list at 1,960 ms at 80 ms RTT (>= 4,000 ms at 250 ms): lookup 400 + GET_PEERS 80 + JOIN 160 + round 1 160 + >= 1,000 ms backoff forced by the saturated registration sample (F1) + round 2 160. Without F1 the list is usable at 800 ms (pass at reference, fail at worst end per flash-crowd F2).
- **R3 fail.** Guardian load at 1 %/min churn, uniform spread: 239 pps in, 1.09 Mbps out, ~300 Ed25519 verifies/s (bounded). At the spec's own 5-minute-session figure with queries landing on the closest alpha guardians: ~13,500 pps > GLOBAL_NEW_PPS 5,000 and 13.7 Mbps egress on a 10 Mbps node. Discovery yield of the DHT entry point at s = 8 is ~1 % of first rounds (F1).
- **R7 pass.** REGISTER_PEER bound to observed UDP source, dynamic PoW tier 14 verified against it, no self-declared IP in any frame or the Peer Record. Spec prefix cap on k-buckets holds; the implementation lacks it (F4).
- **R8 pass.** STORE_RECORD 20 x 422 B/s = 67.5 kbps (74 kbps with a pending matrix) + ACKs 1.6 kbps + one guardian lookup per 30 s, independent of N. Source appears in ~6 % of GET_PEERS samples: 10 PROBEs/s at 1 %/min, 210/s at 5-min sessions (~0.13 Mbps out): proportional to join rate, small.
- **R10 pass.** Swarm-wide DHT control ~30 Mbps against 6 Tbps of video = 0.001 %. Locally a guardian spends 1.09 Mbps (uniform) to 6.2 Mbps (closest-alpha) of its uplink on DHT control, unbudgeted (flash-crowd F5, known).

Findings:

- High: Registration sample is a fixed identity-keyed 1/2^s of relays and absorbs every DHT-directed join; above N0 the DHT entry point yields only saturated candidates, every first round fails, and the starved re-query feeds the fold trigger. filed as [ISSUE-081](../issues/ISSUE-081-dht-sample-is-the-only-entry-point-and-saturates-above-n0.md).
- Medium: RELAY_CAPABLE registrations are sampled at 2^-(s-3) but counted and scaled at 2^s, biasing N_est by (1 + 7 f_rc) and N_relay more, and firing the unfold when a launch cohort crosses the 30-minute uptime criterion. holds (Medium), not filed.
- Medium: The number of GET_PEERS a join issues is defined two ways (one per tree vs one per join), 7x apart in guardian load and 5x in candidates per tree. holds (Medium), not filed.
- Medium: Implementation k-buckets have no per-prefix cap. holds (Medium), not filed.
- Low: Implementation applies neither the RELAY_CAPABLE 2^-(s-3) sample nor the SOURCE_INGRESS exemption when registering. holds (Low), not filed.
- Medium: Queries land on the closest alpha guardians: median StarvedCount is 0 and the load concentrates on G1-G3 (new numbers: 13,500 pps > GLOBAL_NEW_PPS at the spec's 5-minute sessions). already known (capacity-shortfall trace F2 (walker finding, not yet a ticke).
- Medium: Guardian discovery egress is unbudgeted: 1.09 Mbps uniform at 1 %/min, 6.2 Mbps closest-alpha, 13.7 Mbps at 5-minute sessions on a 10 Mbps relay. already known (flash-crowd trace F5 (walker finding); SOLUTION-037 residual).
- High: Join sequence cannot render inside 1.5 s at the worst-end RTT even with a usable sample. already known (flash-crowd trace F2 (walker finding)).

### Implementation versus spec

Trace: [`traces/impl-core-loop.md`](traces/impl-core-loop.md), 34 ticks.

- **R1 pass.** Steady chunks in the churn-storm run play 3.007-3.157 s after source emission (median 3.064 s, depth <= 4); adding the unmodelled encoder 0.1-0.3 s and signing barrier 0.25 s gives 3.4-3.7 s, ~4.0-4.2 s at depth 7. The impl anchor (newest manifest before first play + 3.0 s on a 250 ms grid) reproduces Ch4 §4.3.1.
- **R2 fail.** Node 1 (N = 2): start 1000 ms -> FirstChunk 3019 ms = 2.019 s. small_swarm SJL min 1.398, mean 2.468, p95 3.408 s; churn_storm mean 3.197, p95 4.156 s. The first renderable chunk is the oldest parent-pushed manifest (live_edge - 1), deadline anchor + 3.0 - k x 0.25 s, so SJL >= join + 1.25 s. Spec-level cause already known (flash-crowd F2, cold-start-n2 F4); the impl gate is relaxed to 3.6 s.
- **R5 fail.** Scope holds: per-tree `repairing`, trees 2/3 verified and forwarded during the tree-1 repair, children held by heartbeats. Time fails at the margin: hand walk at 40 ms RTT 710 ms from the last packet; churn median = detection 476 ms + eviction->first block 573 ms = 1.05 s (p95 ~2.1 s) against the spec's 280/480/990 ms table. Contributors: RTTVAR frozen at RTT/2 (F1), probe round + 1.5-RTT handshake with no 0-RTT ticket (F3). Shedding works but fires on rounds 0.34-0.7 s apart (F4).
- **R9 pass.** small_swarm steady PSR 0/925. Churn storm (a §8.3 stress case) 0.39 % by the impl definition but 1.14 % by the spec's denominator because 152 starved intervals after the first frame are excluded as 'backfill' (F2); the impl's PSR is not comparable to the spec's until that exclusion is narrowed.
- **R10 fail.** Measured CDO 2.25 % (small_swarm) and 2.52 % (churn_storm) with BLOCK_PROOF and MANIFEST counted as control; the simulator gate is cdo_max = 0.05 (F8). Spec-level overrun already known (media-pipeline-latency F5).

Findings:

- Medium: Parent RTT estimator is seeded once from the PROBE RTT with RTTVAR = RTT/2 and never updated while media flows, so tau_evict = 200 + 3 RTT. not verified.
- Medium: Simulator PSR excludes starved intervals after the first frame and every interval of a relay whose assigned tree has not pushed yet. not verified.
- Medium: Repair has no fast path: a full PROBE round, a 1.5-RTT session open and a NEIGHBOR round trip precede data. already known (audits/traces/parent-loss-repair.md F7 (no ROSTER / Deputy e).
- Low: Join retry backoff is 250/500/1000 ms; shed decisions are taken on two rounds 0.34-0.7 s apart. already known (audits/traces/flash-crowd.md F6; audits/traces/capacity-shor).
- Low: NEIGHBOR deadline is a constant 600 ms counted from before the handshake, where the spec's is tau_sched + RTT after it. not verified.
- Low: An unparented assigned tree is retried at the join backoff (<= 1 s) instead of the 5 s migration cadence. not verified.
- Low: A shed layer is re-subscribed at the hysteresis expiry before any of its trees has a parent. not verified.
- Low: Simulator KPI gates are relaxed against Ch8 §8.2 without a recorded decision: cdo_max 0.05 vs 2 %, sjl_mean 3.6 s vs 1.5 s with a stale rationale. not verified.

## Tickets filed

- [ISSUE-073](../issues/ISSUE-073-slicing-matrix-entry-cannot-encode-a-layer-bundle.md) (High, App D §D.4.8, Ch2 §2.3.3, Ch1 §1.2.4 §4.2.1): The 7-byte matrix entry has one Layer field and no bundle encoding, so the M = 2 rung every stream starts on is undefined on the wire.
- [ISSUE-074](../issues/ISSUE-074-deputy-timer-expires-before-a-sibling-can-answer.md) (Medium, Ch1 §1.2.3, App B τ_deputy, Ch3 §3.3.1): The fixed 45 ms Deputy timer is shorter than a sibling's 1 to 2 RTT reply on every path over about 22 ms, so the election never succeeds at the reference RTT.
- [ISSUE-075](../issues/ISSUE-075-heartbeating-parent-that-never-delivers-is-never-evicted.md) (High, Ch3 §3.3.1–§3.3.2, Ch1 §1.2.2 Rank Admission): Eviction counts any packet and an unparented relay accepts children, so orphaned relays adopt each other and a subtree starves without bound.
- [ISSUE-076](../issues/ISSUE-076-unfold-trigger-re-creates-the-base-layer-freeze.md) (High, Ch1 §1.1.5 §5.6, App B): The growth-gated unfold fires when relays and viewers grow together, raises the base stripe bitrate and drains 16 % of base-layer children for 30 s per cycle.
- [ISSUE-077](../issues/ISSUE-077-propagation-term-is-per-link-not-per-child-share.md) (Medium, Ch1 §1.1.3, Ch4 §4.1.2, SOLUTION-013/033): The 13 ms transmission term is per link; a relay with K_v children spends about 150 ms per hop, so glass-to-glass is 4.7 to 5.0 s, not 4.0.
- [ISSUE-078](../issues/ISSUE-078-parity-rule-leaves-a-percent-of-blocks-per-hop-unrecoverable.md) (High, Ch4 §4.2.2, Ch1 §1.2.1 §1.3, App B): With K = 16 the parity rule leaves 1.4 to 9 % of blocks per hop unrecoverable; at depth 7 a quarter to a half of the stream is pulled and the PULL reserve is exceeded from 2 % loss.
- [ISSUE-079](../issues/ISSUE-079-rank-admission-reads-a-rank-frozen-at-admission.md) (Medium, Ch1 §1.2.2 case 2, Ch5 §5.2.2, App D §D.4.17): A child's rank is read from the proof it presented at admission and never refreshed, so a lower contributor preempts a higher one.
- [ISSUE-080](../issues/ISSUE-080-registration-sample-exponent-ratchets-without-bound.md) (High, Ch2 §2.3.2, §2.3.3, App B): Registrants never learn that s rose, so ActiveCount never halves, N_est doubles per evaluation and s ratchets to its cap within seconds.
- [ISSUE-081](../issues/ISSUE-081-dht-sample-is-the-only-entry-point-and-saturates-above-n0.md) (High, Ch2 §2.3.2, Ch1 §1.2.2 §2.2, Ch1 §1.1.5 §5.6): The fixed registration sample absorbs every DHT-directed join; above N_0 round 1 yields only saturated candidates and the starved re-query feeds the fold trigger.
- [ISSUE-082](../issues/ISSUE-082-join-sequence-cannot-meet-the-startup-target.md) (High, Ch4 §4.3.1, Ch5 §5.1.3, Ch8 §8.2): The specified join sequence renders its first frame at 3.3 to 4.3 s against the 1.5 s SJL target.
- [ISSUE-083](../issues/ISSUE-083-preemption-and-displacement-are-unreachable-from-the-join-algorithm.md) (High, Ch1 §1.2.2 §2.2 step 2, Rank Admission cases 2–3): The probe filter drops every K_avail = 0 candidate, so no NEIGHBOR ever reaches a full parent and preemption and leaf displacement never fire.
- [ISSUE-084](../issues/ISSUE-084-punch-requester-probes-before-the-mapping-opens.md) (Medium, App D §D.4.14, Ch1 §1.2.2 §2.2, Ch6 §6.1.1): The requester's PROBE leaves before the target's PING opens the mapping, and a SYMMETRIC requester never receives that PING.
- [ISSUE-085](../issues/ISSUE-085-source-ingress-proposal-carries-no-publisher-signature.md) (High, Ch6 §6.3.3, App D 0x30, Ch2 §2.3.1): RELAY_PROPOSAL(SOURCE_INGRESS) has no signature field and no rule binds the session to PK_Publisher, so one identity can capture every ingress relay; the implementation trusts a self-declared flag.
- [ISSUE-086](../issues/ISSUE-086-prefix-caps-not-applied-in-the-implementation.md) (High, Implementation of Ch2 §2.2.2 and App C): The per-prefix caps on k-buckets, Active Set and parent set are not implemented, so a one-/24 Sybil fills all of them.
- [ISSUE-087](../issues/ISSUE-087-xdp-allowlist-keyed-by-address-shares-one-bucket-across-a-cgnat.md) (High, Ch7 §7.2.1–§7.2.2, App B B.1.4): The XDP allowlist is keyed by address, so children behind one CGNAT share one 2,000 pps bucket and lose the entry when any of them disconnects.
- [ISSUE-088](../issues/ISSUE-088-receipt-ledger-scanned-three-times-per-receipt.md) (High, Implementation of Ch5 §5.2.1; node/incentive.rs): on_receipt scans the whole receipt ledger three times per receipt, 1.8 × 10^10 visits per second at a 10,000-child super node.

## Verification

Every candidate rated High by its walker was checked against the quoted spec text, the design record in `solutions/` and the open tickets before filing. Four scenarios (growth, adversarial, super node, DHT at scale) also had every candidate checked by an independent verifier agent; its verdicts appear in the scenario lists. Candidates rated Medium or Low in the other eight scenarios are marked "not verified": the text they quote was not re-opened, and they are leads for the next audit, not findings.

## Spec silences the walkers assumed through

**Cold start, N = 2**

- Whether bootstrap seeds are DHT storage nodes (Ch2 §2.1.3 inserts them into k-buckets, which with k = 20 makes them guardians of every key at small N); walked with the publisher as the only guardian as the scenario states.
- Which dynamic-puzzle tier a node solves before it has read SwarmSize; walked at tier 8 (0.04 ms), tier 14 (2.5 ms desktop, ≈160 ms on Cortex-A53) as the safe choice.
- Whether a viewer with the same node as parent in both trees opens one or two QUIC connections (edge-disjointness is 'ideal'); walked with one connection and one eviction timer.
- Timeout of a QUIC handshake to a dead tier-2 candidate; Ch3 §3.3.1 gives τ_sched + RTT only for NEIGHBOR over an open session. Walked with 1 s.
- Timeout of a plain-UDP GET_PEERS with no reply and the DISCOVERY retry cadence after a complete disconnect; walked with 400 ms (implementation) and 1 s.
- Whether 'no DHT re-query returns records' (Ch1 §1.3.1, the 180 s TERMINATED bound) includes a re-query that gets no reply; walked as 'stay in DISCOVERY'.
- Whether a block obtained by PULL counts toward 'received and Merkle-verified its first complete segment' for warm-up; walked as push-only, so warm-up ended at segment 12 not 11.
- Whether the source pushes the remaining blocks of the current chunk to a child admitted mid-chunk or starts at the next chunk; walked as next chunk (≤ 250 ms of extra backfill).
- What a joiner does with a Stream Record whose LiveEdgeSegmentId = 0 after the restart; walked as 'wait for the first pushed manifest'.
- The viewer's registration vanishes with the restarted guardian's store and nothing tells it; SwarmSize reads 1 for 35 s until the 90 s refresh. Harmless at N = 2 (θ_join = 1 either way).

**Growth from 2 to 30 relays**

- Whether the source's own RELAY-class registration counts toward N_relay (assumed yes; impl agrees; each ladder step is one viewer earlier than '30 relays' suggests)
- Which of a due resize and a due fold fires first at a dwell expiry (finding A; both paths walked)
- Whether the per-tree pre-emit count shares one headroom across several new trees (finding C; shared reading walked)
- Retry cadence for a tree that is both assigned and subscribed while unparented during a window (assumed >= 1 s under grace, 5 s migration cadence after)
- How orphans of a moving relay find a slot during the window when target slot counts rise only at the switch (assumed pre-resize free slots, else cold re-attach at the switch)
- Who wins a slot when several equal-rank NEIGHBORs arrive in one tau_sched (assumed first arrival)
- Whether 'the source serves no PULL_REQUESTs from the live edge' also excludes retained-segment backfill for a late joiner (assumed it does; R2)
- When a relay re-registers its new AssignedTrees at a resize, announcement or switch (assumed announcement)

**Flash crowd at N = 10,000**

- S1: PoW tier for the first GET_PEERS before any Stream Record is read; assumed tier 12, guardian one-tier grace accepts it against tier 14 (~60 ms of hashing at stake)
- S2: GET_PEERS stratification ceil(20 / popcount(WantedTrees)) = 4 per tree gives 24 > 20 at six wanted trees; assumed round-robin 4,4,3,3,3,3
- S3: whether a joiner may render a segment before its 3.0 s deadline; strict vs lenient readings differ by ~1 s in SJL, both miss 1.5 s
- S4: whether a failed re-join of a shed layer after hysteresis sets StarvedTrees on the re-query; assumed yes, affects only top-layer StarvedCount
- S5: source upload at N = 10^4; assumed 100 Mbps, changes K_S (18/9 slots) and depth-1 count only
- S6: whether tree joins may start during JOINING; assumed no per Ch1 §1.3.1, lenient reading saves ~0.3 s of SJL
- S7: order in which relays and subscribers occupy depth 1-3 in L0 trees after churn; assumed relay-shallow, the rule that would produce it is the subject of ISSUE-061

**Parent loss and repair**

- S1: No timeout for the QUIC handshake to a dead passive-set candidate (Ch3 §3.3.2 line 17); assumed 600 ms as the impl does; a 10 s QUIC default changes R9/R11 (F4).
- S2: What an orphan does with a Deputy ACCEPTED that arrives after tau_deputy fired and it attached elsewhere; assumed DISCONNECT(TreeID, CHOKE) to the Deputy (F1).
- S3: Whether deputies run the partition/surplus hash of Ch1 §1.2.3 §3.2; assumed exempt (F6).
- S4: A Deputy's ACCEPTED.HopDepth while it has no parent; assumed its stale pre-loss depth, corrected by BLOCK_PROOF after re-attach.
- S5: Whether a relay UNPARENTED in tree m may ACCEPT a NEIGHBOR(m); the Rank Admission Rule says yes; assumed yes (F3).
- S6: Unchoker tie-break when every neighbour's PullRate is 0: the §5.1.2 pseudocode unchokes the top RegularSlots unconditionally, the rule box requires >= Threshold_min; assumed the rule box (choked). Decides how many slots an orphan has for F2.
- S7: Which tier-1 pool candidates are dialed concurrently: §3.3.1 says tiers overlap, §3.3.2 is a sequential loop; assumed sequential within tier 1.
- S8: The RTTVAR behind the tau_evict table; assumed 5 / 10 / 10 ms to reproduce App B (F5).
- S9: The §3.3 fast path picks by R and K_avail with no depth term; assumed the pick may land at any depth <= D_max - 2 and REJECTED_DEPTH bounds the rest.
- S10: Steady-state heartbeat cadence: source pacing over <= 125 ms leaves >= 125 ms idle per chunk > tau_ping = 100 ms, so a heartbeat fires every chunk on every edge; assumed harmless (~0.1 % of uplink).
- S11: 'Advertised K_avail' and R of passive-set entries: Peer Records (Ch2 §2.3.3) carry neither; assumed the values come from the most recent PROBE_RESPONSE or gossip the peer holds, else unknown.
- S12: Whether 'repair complete' in the Ch3 §3.3.1 table counts from the parent's death or from the child's last received packet; walked from the last received packet (one-way offset of RTT/2 added).

**Capacity shortfall, shed and fold**

- u_S is not given by the scenario; assumed 20 Mbps (Ch6 §6.3.1 relay-capable floor), which only moves K_S by ±1 per tree.
- 'the mean of s_hat_m over the last tau_fold of acknowledgements' when fewer than tau_fold ACKs carry starvation: assumed a full 10 s window is required (first fold at t = 10 s); averaging what exists folds at t ≈ 2 s.
- When a peer first sets a StarvedTrees bit: assumed after its first failed round, on the next re-query.
- Whether the 10 s hysteresis on a shed layer survives that layer leaving the mapping at a fold: assumed the timer is per layer and the fold ends it, so trees 5 and 6 are joined from the announcement.
- Which alpha of the k guardians a GET_PEERS reaches: walked the spec's uniform model; alternatives are finding F2.
- Whether an unfold's K_v fall drains children: §5.6 is silent; applied §4.5 last paragraph (DRAIN_NOTICE, CAPACITY).
- Whether the source opens reserve slots before or after the fold: walked as not opened, since the fold cleared the starvation (finding F4).
- Whether a fold that follows an unfold is a matrix change bound by the dwell: assumed yes per 'fold, unfold or resize alike', which makes the F1 freeze 30 s instead of 15 s.
- Mean upload walked as every relay at 5 Mbps; a real distribution with floor effects lowers mean K_v below floor(mean) and raises the starved fraction without changing any rule.

**Media pipeline and latency budget**

- S1: Ordering of MANIFEST relative to BLOCK_PROOF/symbols across QUIC streams is not stated (D.2 lists MANIFEST under control, BLOCK_PROOF on the tree's stream); assumed the receiver holds a reconstructed block until the chunk's manifest is accepted, bounded by the 2-segment lookahead.
- S2: GOSSIP_EXCHANGE and SHUFFLE cadence are never given (finding F6); assumed the implementation's 1 s and 5 s for the R10 sum.
- S3: How many segments one BUFFER_STATE_BITFIELD advertises is not stated; assumed 4 (the 3 s buffer plus one).
- S4: Whether a relay serialises a block's symbols child-by-child or interleaves across children is not stated; the worst-child delay is identical (150 ms), the mean differs (82 vs 150 ms); F1 reports both.
- S5: When a relay repairs a block it cannot reconstruct but must forward (finding F3); assumed the PULL-zone rule applies literally, as the implementation does.
- S6: Heartbeat frame size (QUIC PING frame vs protocol PING on the control stream) is not stated; assumed ~50 B on the wire.
- S7: Encoder latency is given as a 0.1-0.3 s range; walked at 0.2 s and reported as a range.

**Receipts, rank and admission**

- S1: a child's K_avail for the first ROSTER before its first PROOF_OF_UPLOAD (NEIGHBOR carries none); assumed 0 until the first receipt.
- S2: receipt deadline for the partial segment in which a child was admitted; assumed the first full segment after admission is the first due (implementation).
- S3: which child is released when K_v(m) falls, and whether the K_old - K_new overlap is charged to the PULL reserve; walked both the spec (undefined) and the implementation (newest child; charged) -> finding F2.
- S4: the decayed Theta_A (lambda = 0.005) is defined and consumed by no rule; ContributionScore_2s in Ch5 §5.1.3 is neither Theta_A nor Theta_rate; assumed PULL ingress over 2 s (implementation). SOLUTION-023's residual-risk argument rests on the decayed score -> feeds F1.
- S5: whether a sitting child ever re-presents RANK_PROOF; assumed never (only the two App D §D.4.17 triggers) -> finding F1.
- S6: the value of Threshold_min; assumed 64 kbps (implementation) -> finding F3.
- S7: whether the parent's own PULLs from a child (a child in its Active Set serving it repair) count as that child's reciprocity; assumed counted like any mesh counterparty; none occurred.
- S8: the recorded rank of a child admitted into a free slot without a proof; assumed 0 per 'A request with no proof ... has r_new = 0' -> zero-signal case of F1.

**NAT traversal and emergent relays**

- Second PROBE after a punch (a11): no rule; both readings walked (F1).
- 'One registration period' for the unpunchable mark (App D §D.4.14): 90 s or 180 s; assumed 90 s, no consuming rule depends on it.
- Punch validity 1 s (App B): no node enforces it; assumed decorative.
- Mixed-round classification, saturated + timeouts (b9): assumed failed for shed, silent for bridge (F2).
- Bridge trigger (b10): assumed 2 rounds, the implementation constant (F2).
- JOIN to a CONE membership peer during JOINING (a6): §D.4.14 is written for the tree-join PROBE; assumed the same punch applies and membership targets were PUBLIC.
- RELAY_BIND before the relay's first verified segment (b15): text gates BIND on the slot; assumed BIND on ACCEPTED and child waits through warm-up (F8).
- Source's re-query interval for RELAY_CAPABLE records (c2): none stated; assumed 2 s (impl nat.rs:181).
- Uptime clock for RELAY_CAPABLE (c3): assumed node-process uptime (F3).
- n_ingress used in K_R(m) at the first bind (c5): both walked (F4).
- R_src at M = 2 ('three copies of L₀ however striped'): assumed 5.175; striped reading gives 10.35 (F4).
- How the ingress relay learns starvation and n_ingress changes (c7, c11): no frame (F6).
- Ingress unbind on source loss (c12): assumed none (F5).
- Class of a SYMMETRIC peer: spec says LEAF, scenario said LEAF_PRIVATE; walked as LEAF.
- Whether non-symmetric peers avoid PUBLIC parents (d2): no steering rule for them (Ch6 §6.1.1 steers only SYMMETRIC children toward PUBLIC); both bounds walked.
- Whether a bridged child issues receipts during the relay's WARMING (b17): assumed no blocks → no receipts.

**Adversaries**

- Ch7 §7.1.1 bans 'the peer that delivered it' without stating whether the ban keys on NodeID or on /24/48 prefix. Walked as NodeID, which lets 10 ms identity rotation defeat the doubling escalation; the §5.3.3 eviction and §5.1.3/§5.2.1 cooldowns already key on prefix, so the gap is local to the poisoning ban (part of F3's cluster).
- The exact number of forged blocks before a parent is dropped is undefined in the spec (core of F3); impl chose 3, Ch5 text reads as 1.
- The fold estimator s_hat_m reads guardian StarvedCount (distinct querying NodeIDs) with no per-prefix cap, so a Sybil querying with a starved bit could inflate it. Accepted: s_hat_m is an explicit coarse trigger (10/30 s mean, 5% threshold), the fold only removes the top layer (resolution loss, never a freeze), and unfold is relay-growth-gated, so blast radius is bounded. Not filed.
- Disjointness of the alpha=3 S/Kademlia lookup paths is asserted in Ch2 §2.1.2, not mechanized; simulation debt, not a defect.
- The P_hijack <= f^alpha eclipse bound holds because f is the prefix fraction, not the node fraction, so the cheap-identity Sybil of attack (b) does not raise f; no finding.

**Super node at K_v = 10,000**

- S1 RANK_PROOF commit cadence (per segment vs on demand); a per-segment rebuild of a 2^20-leaf tree is ~0.4 core-s at a super node
- S2 How many chunks a BUFFER_STATE_BITFIELD round covers; the BlockIndex layout forces one frame per chunk
- S3 Default choke state of a peer with an open session that is not in the node's Active Set (F2)
- S4 Which children a fallen K_v(m) drains (incentive-timeline S3; impl drains the newest). Dropped candidate: dK_v/dE_bar ~ -300 at K_v = 5,217, so E_bar noise drains ~0.5 clean-link children/s and a 500-child ISP loss event drains 117 unrelated children; accepted trade-off of SOLUTION-025
- S5 Whether 'parity per block, per downstream link' means a per-link count or a per-link encode; assumed count since RaptorQ symbols are deterministic in (block, ESI)
- S6 Whether BLOCK_PROOF, MANIFEST and per-packet headers are Bytes_control in the CDO KPI (1.85 % vs 0.7 % at the super node)
- S7 16-bit K_avail and ChildCount ceiling 65,535 per tree (~63 Gbps single-tree on a 0.75 Mbps stripe); fine at 10 Gbps
- S8 Heartbeat frame form and size (QUIC PING frame vs control-stream PING); assumed ~65 B
- S9 Serialization order of a block across k children (which child is 'last'; ISSUE-072)
- S10 How a super node resolves 10,000 downloader keys/s for receipt verification (ISSUE-065, open); assumed cached from the session handshake
- Dropped drift: Ch1 §1.2.1 table 1734/867/~8,700 vs formula 1,739/869/8,694; Ch5 §5.2.3 'BitmapLen typically 8-16 bytes' vs normative bit = ChunkIndex x 128 + j giving L = 49 B (receipt 233 B, 23 Mbps inbound at 10,000/s)
- Scenario note: K_v = 10,000 in one 1.5 Mbps tree needs 19.2 Gbps; at 10 Gbps the slot formula permits 5,217 (10,434 on a 0.75 Mbps tree); counts given at both, timing walked at 5,217

**Discovery at N = 1,000,000**

- S1: how many GET_PEERS a join issues: assumed one tree query (0x3F) plus one membership query (filed as F3)
- S2: how the publisher and registrants learn that the 20 closest nodes to K_s changed; no republish rule and no lookup cadence (30 s assumed, as the implementation)
- S3: whether CONE leaf registrants send the tau_nat keepalive and how a membership JOIN reaches a CONE record (punch is written for PROBE only); assumed PUBLIC JOIN targets
- S4: whether a new Active Set connection triggers an immediate GOSSIP_EXCHANGE; assumed no, first records arrive within one tau_gossip
- S5: Kademlia hop count at 7 x 10^5 DHT nodes; 5 hops assumed, matching the spec's 0.3-0.5 s
- S6: whether a guardian rejects a REGISTER_PEER from a NodeID outside the current sample; assumed not
- S7: RELAY_CAPABLE share of the swarm; 5.2 % of N assumed from Ch6 §6.3.1 criteria and 1 %/min ages
- S8: how a registrant learns a changed s at its refresh; assumed it does (flash-crowd F1 says no mechanism exists)

**Implementation versus spec**

- S1: the spec does not fix when a joiner's playout anchor is taken relative to the manifests a parent pushes at admission; the impl anchors at the newest manifest seen before the first play (recorded in ARCHITECTURE §6), assumed equivalent to '3.0 s behind the received live edge'.
- S2: Ch4 §4.3.1 're-anchor on entry to ACTIVE' and 'correct drift continuously' have no impl counterpart; on one virtual clock no tick depended on them.
- S3: the spec gives JOINING no time bound; the impl's 1 s fall-through to CONNECTING was walked as given and judged an improvement, not a defect.
- S4: the spec's initial RTTVAR for a fresh parent is unstated (impl uses RFC 6298 R/2; the spec's own tau_evict table implies RTT/4..RTT/25).
- S5: heartbeat form on a bursty tree is unstated (QUIC PING vs protocol PING); the impl sends a protocol PING per chunk gap (~every 250 ms per child), cost inside the measured CDO.
- S6: the 2 s data-stall and stalled-relay drain rules are implementation-only (ISSUE-066 addendum, recorded); neither fired in the runs.
- S7: the sim models a 1.5-RTT handshake and no node holds a resumption ticket; the spec's 1-RTT re-attach depends on the ticket rule the impl lacks (covered by F3).
- S8: which frames count as 'video' for CDO (BLOCK_PROOF, MANIFEST) is unstated; the impl counts them as control, the conservative reading.
- Dropped after debate: backfill_from is dead on the first attach (live_edge is NONE when ACCEPTED is handled) but the parent already pushes the same live_edge-1 manifests, so no behavioural consequence; ACTIVE -> DISCOVERY -> CONNECTING skipping JOINING and never TERMINATING on an empty re-query is cosmetic; source slots via the relay formula (cold-start-n2 F6), manifest window 3 vs 2 (media-pipeline F7) and the spec table's RTTVAR inconsistency (parent-loss-repair F5) were not re-filed.

## Not covered

- Ch7 §7.3 onion routing and Ch6 §6.2 QUIC migration were not walked.
- The implementation walk covers bootstrap, join, first playout, one repair and one shed in `bs-core`. The DHT, incentive and relay code paths were read only where a spec walk pointed at them.
- No walk ran the simulator; every number is computed from Appendix B constants by hand.
- Medium and Low candidates in eight scenarios were not independently verified (see Verification).
