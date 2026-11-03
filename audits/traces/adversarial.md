# Trace: adversarial

Walker scenario `adversarial`. Four attacks at N = 1,000 stepped on a virtual clock: (a) 10 %
malicious block injectors, (b) a one-/24 Sybil, (c) eclipse through disjoint lookups, (d) a
colluding pair plus manifest replay. Every number is derived from Appendix B constants and the
spec's own RTT figures (80 ms reference, 250 ms worst end). Where the spec is silent the
assumption is named and listed in section 5.

## 1. Scenario and initial state

| Item | Value | Derivation |
|---|---|---|
| N, N_relay, M | 1,000 peers, 500 relays, M = 6 | N_relay >= 30 puts the ladder at its top rung (App B M_max) |
| Mapping at M = 6 | T1,T2 = L0 @ 0.75 Mbps; T3,T4 = L1 @ 0.75 Mbps; T5,T6 = L2 @ 1.5 Mbps | Ch1 §1.1.5 §5.2 ("the 0.75 Mbps L0 stripes" at M = 6), reference ladder 1.5/1.5/3.0 Mbps |
| Blocks per chunk | L0/L1 trees: 0.75 x 0.25 = 187.5 kbit = 23.4 KB -> 2 blocks (last padded); L2: 46.9 KB -> 3 blocks | Ch4 §4.1.1, 16 KB blocks |
| Omega | 1.15 | E = max(1, ceil(0.05 x 16)) = 1 -> (1 + 1/16)(1.085) = 1.153 |
| Reference relay | u = 10 Mbps, t_v = 1 | K_v(L0 tree) = floor(0.9 x 10 / (0.75 x 1.15)) = 10; K_v(L2 tree) = floor(9 / 1.725) = 5 |
| Relays per tree | ~500 / 6 = 83 | single-tree relays spread by rendezvous (Ch1 §1.2.1) |
| Paths | 80 ms RTT (RTTVAR 10): tau_evict = 320 ms; 250 ms (RTTVAR 10): tau_evict = 490 ms | App B tau_evict row |
| Per-block wire cost | 16 source symbols + 1 parity, 17 x (1024 + ~70) B = 18.6 KB; BLOCK_PROOF = 4 + 4 + 2 + 1 + 1 + 32 x depth B = 48 B at depth 1 | App B f_frame, App D §D.4.9 |
| Pacing | a chunk's blocks leave over <= 125 ms, so a 2-block chunk delivers one block per ~62 ms | Ch3 §3.3.2 |
| Identity cost | static 2^16 + dynamic 2^12 (tier(1000) = 12) = 69,632 Blake3 of ~40 B at 0.15 us = **10.4 ms** on one core; leaf rung (tier 10): 10.0 ms | Ch2 §2.2.1 |
| P_obs at an honest node | >= 20 (a 1,000-peer swarm spans hundreds of /24s) | Ch2 §2.2.2 |
| Prefix cap c_p(K) | ceil(K / 20): K = 8 -> 1; K = 20 -> 1; K = 6 parents -> 1; K = 10 child slots -> 1; K = 5 -> 1 | App B prefix cap row |
| Guardians of K_s | k = 20 closest to Blake3(StreamID); GET_PEERS reaches ~alpha = 3 of them | Ch2 §2.3.2 |
| Fold estimator | s_hat_m = (k/alpha) x median_g StarvedCount[m] / N_hat = 6.67 x S / 1000 at s = 0 (N < N_0 = 10^4) | Ch1 §1.1.5 §5.6, Ch2 §2.3.2 |
| Manifest window | X_edge - 8 <= seq <= X_edge + 2 | Ch7 §7.1.2 |
| Accusation rules | 3 distinct accusers, rank >= 0.25, 3 prefixes, within tau_accuse = 10 min; 4 per accuser per minute; eviction 10 min doubling to 24 h | Ch5 §5.3.3 |

Attack (a) assumes the 100 injectors are prefix-diverse relay-class nodes advertising maximum
K_avail (Ch8 §8.3 B gives no prefix constraint). Victim V is a 10 Mbps relay at depth 3 with 10
children in T1 and 10 in T3; at t = 0 it has just accepted injector A as its parent in T3 after a
parent loss there.

## 2. Tick table

### 2a. Ten percent block injectors (R7, R11), 80 ms path

| t_ms | node | rule (file §) | computation | state after |
|---:|---|---|---|---|
| 0 | A -> V | Ch1 §1.2.2 Rank Admission 1; App D §D.4.4 | ACCEPTED(TreeID = 3, HopDepth = 2); A was the best-scored candidate in P_3 (max K_avail, fresh identity, no history) | V's parent in T3 = A; T3 leaves CHURN_REPAIR |
| 0 | V -> children (T3) | Ch3 §3.3.1 rule 1 | V has sent nothing in T3 for >= 100 ms -> heartbeat every tau_ping = 100 ms | children do not evict V; they hold T3 open with no data |
| 40 | A -> V | App D §D.4.9 | BLOCK_PROOF(seg s, BlockIndex c<<12 \| 0, ProofDepth 1, SenderHopDepth 2, one forged 32 B sibling) = 48 B arrives after 40 ms one-way | V has the proof, no symbols yet; nothing verifiable |
| 40..102 | A -> V | Ch4 §4.2, pacing | 17 RAPTORQ_SYMBOL datagrams, 18.6 KB, over ~62 ms | V holds 16 source symbols: block reconstructed by concatenation, zero decode (Ch4 §4.1.2) |
| 102 | V | Ch4 §4.1.2 verification | Blake3(16 KB) ~5 us + 1 sibling hash; root' != MANIFEST(s, c).MerkleRoot | **FAIL.** Block dropped. Zero bytes of it ever left V: verify-before-forward (R7 holds at this hop) |
| 102 | V | Ch4 §4.1.2 "the delivering parent is flagged (Ch5 §5.3)"; Ch5 §5.3.3 "Poisoning is punished locally (disconnect and local ban, Ch7 §7.1.1)" | two sections give two consequences (F3). Walk assumes the stronger: DISCONNECT(TreeID 3, EVICTION) + ban A for tau_ban = 10 min | A severed from T3; T3 enters CHURN_REPAIR (Ch3 §3.3, per tree; T1,T2,T4..T6 keep forwarding) |
| 102 | V | Ch7 §7.1.1 ban key | "the peer that delivered it" — NodeID or address not stated (S2). Walk keys on NodeID, the reading every other use of "peer" implies | ban table: {A.NodeID: until 600,102 ms, strikes 1} |
| 112 | V | Ch3 §3.3.1 T = tau_evict + 10 | pick standby S from P_3 (>= 3 relays); no 0-RTT ticket -> 2 RTT handshake | ClientHello to S |
| 272 | V -> S | App D §D.4.3 | NEIGHBOR(3, HIGH) after 160 ms handshake | S must answer within tau_sched = 100 ms |
| 352 | S -> V | Rank Admission 1 | ACCEPTED(3, HopDepth 2); V's S-pick is again 10 % likely an injector: expected picks to an honest parent 1/0.9 = 1.11 | V's parent in T3 = S |
| ~400 | S -> V -> children | Ch4 §4.1.2 | first block of the current chunk verifies; forwarded to 10 children with SenderHopDepth 3 | T3 restored ~0.40 s after admission of A |
| 400 | V | gap accounting | T3 blocks missed since the previous parent was lost (~0.3 s before t = 0) to 0.40 s: ~0.7 s x 8 blocks/s = **6 blocks = 96 KB** | PULL zone must repair them (W_pull = 1.5 s) |
| 1500..3000 | neighbours -> V | Ch4 §4.3.3, beta_pull = 0.5 Mbps -> 262 ms per block per slot | 6 blocks over 2 unchoked providers = 0.79 s, done by ~2.3 s < 3.0 s deadline | **no starvation; PSR contribution 0** (R9) |
| 600,102 | V | Ch7 §7.1.1 | ban on A.NodeID expires; A is still live under identity A' minted at t = 112 for 10.4 ms and re-registered (N < N_0: every peer registers) | the doubling "on each repeat" never engages: A' is a new peer |

Cost per injector-victim pairing: victim 18.6 KB + 48 B ingress, ~5 us of hashing, one per-tree
repair (0.4 s), 96 KB of PULL ingress and the same in neighbours' egress; attacker 10.4 ms of
hashing and one REGISTER_PEER. A child of V in T3 received zero bytes of the forged block. On the
250 ms path the same walk gives detection at 125 + 62 = ~190 ms, re-attach by ~900 ms, 11 blocks
to pull, completing by ~2.9 s inside W_pull = 2.0 s before the 3.0 s deadline: no starvation, no
margin to spare.

XDP note: A is allowlisted after the handshake, so its bucket allows 2,000 pps x 1,094 B = 17.5
Mbps of garbage until the user-space disconnect at 102 ms (~0.2 MB). The kernel filter never
sees the forgery; Ch7 §7.2.1 says so.

Implementation comparison (`src/crates/bs-core/src/node.rs` `try_verify`, lines 2374-2415): a
rejected block increments `rejected_blocks` on the parent and evicts only at **3**, then inserts
the NodeID into `bad_peers`, a `HashSet` that is never aged. Detection at 102 ms becomes eviction
at ~226 ms after three forged blocks (55.8 KB), the ban is permanent instead of 10 min doubling,
and `ban_peer`/tau_ban are not used on this path (F7). A forged `MANIFEST` (`ManifestVerdict::
BadSignature`, `node.rs` 2236) only emits `ManifestRejected`: no disconnect, no ban.

### 2b. One-/24 Sybil (R6, R7, R11)

| t_ms | node | rule (file §) | computation | state after |
|---:|---|---|---|---|
| 0 | Sybil host | Ch2 §2.2.1 | one core: 1 / 10.4 ms = **96 identities/s**; GPU 10^4-10^5/s; all solved against the 256 observed addresses of its /24 | identities are bound to the attacker's own /24 only |
| 0 | any honest node | App B prefix cap; Ch2 §2.2.2 | P_obs >= 20 -> c_p(8) = ceil(8/20) = 1 Active Set slot; c_p(6) = 1 parent; c_p(10) = 1 child slot per L0 tree; c_p(5) = 1 per L2 tree; k-bucket 1 of | 0 | honest relay | Ch2 §2.2.2; App C §C.2.1 | k-bucket cap: 1 of k = 20 per /24, unconditional | one /24 holds at most 1 of 20 bucket entries |
| 0 | victim V | diversity cap | with K = 8/6/10/5, every cap floors to 1, so the Sybil takes exactly 1 Active Set slot, 1 parent slot, 1 child slot per tree, 1 bucket entry at V | Sybil footprint on any one honest node = 1 of each set |
| 0 | swarm aggregate | Ch2 §2.2.2 | per tree, sum_v c_p(K_v(m)) >= N_relay/M = 83 children placeable from one /24; the /24 can hold ~8 % of N as children per tree | a single-prefix flood above that share is unplaceable capacity, falls to source reserve / bridges |
| 0 | collusion detector | Ch5 §5.3; App B N_collusion = 20 | N_guard = max(N_record, locally observed) >= 1,000 > 20 -> detector active | no small-swarm suppression at N = 1,000 |

What the Sybil can take, by the diversity-relative cap `max(1, ceil(K / min(P_obs, 20)))`:

| K (slot set) | K = 8 Active Set | K = 20 bucket | K = 10 L0 child | K = 6 parents | K = 5 L2 child |
|---|---|---|---|---|---|
| min(P_obs,20) = 20 | ceil(8/20) = 1 | 1 (uncond.) | ceil(10/20) = 1 | ceil(6/20) = 1 | ceil(5/20) = 1 |
| worst case P_obs = 1 (dorm) | 8 | 1 (uncond.) | 10 | 6 | 5 |

At K = 8 the cap is 1 whether P_obs is 20 or 8 (ceil(8/8) = 1). At K = 20 the cap is 1 at
P_obs = 20 and rises to ceil(20/8) = 3 if the victim has seen only 8 prefixes. The k-bucket cap
stays 1 unconditionally, which is what keeps P_obs trustworthy (eclipse defence, 2c). So the
Sybil, however many identities/s it mints, takes **one slot per set per honest node** once that
node has seen 20 prefixes. Rank forged by leaf-class Sybil downloaders (SOLUTION-057) buys
preemption of lower-ranked children, but the same per-prefix cap bounds that to 1 child slot per
honest parent per tree, a slot a genuine contributor would also have taken. R6's floor holds: a
non-uploading device still receives L0 as a tree child. R7 holds: no identity is usable from an
address the attacker does not control.

Implementation gap: grep of `src/crates/bs-core/src/dht.rs` (`insert`, lines 113-132) and
`node/membership.rs` shows the k-bucket uses the classic capacity rule (`b.len() >= self.k`) with
**no per-/24 cap**, and no `P_obs`-relative cap is applied at the Active Set or parent set. The
prefix key is used only for eviction cooldowns and accuser diversity (`incentive.rs`
`prefix_key`, `audit.rs`), not for the admission caps Ch2 §2.2.2 makes load-bearing (F6). At
N = 1,000 from one /24 this lets the Sybil fill a victim's bucket and Active Set, the exact
eclipse the spec's cap exists to stop.

### 2c. Eclipse via disjoint lookups (R7)

| t_ms | node | rule (file §) | computation | state after |
|---:|---|---|---|---|
| 0 | joiner V | Ch2 §2.1.2 | FIND_VALUE over alpha = 3 strictly disjoint paths; attacker controls prefix fraction f | P_hijack <= f^alpha |
| 0 | V | arithmetic at f = 0.2 | 0.2^3 = 0.008 = 0.8 % | one lookup compromised < 1 % |
| 0 | V | at f = 0.1 (this scenario's 10 %) | 0.1^3 = 0.001 = 0.1 % | eclipse of one lookup 1 in 1,000 |
| 0 | V | Ch2 §2.2.2 note | f is the **prefix** fraction, not the node fraction; the /24 Sybil of 2b has f ~ 1/(hundreds) << 0.1 | a one-/24 attacker cannot reach f = 0.1 in prefixes |
| 0 | V | Ch1 §1.3 eclipse-foothold rule | swarm_size is signed by SK_Publisher; a hostile guardian cannot forge N; V keeps >= 1 independent DHT re-query in flight until a 2nd distinct parent is set | single-peer view diluted within seconds |

The bound holds as stated. The load-bearing subtlety the spec gets right: f is measured in
prefixes (a scarce, purchasable resource), so the cheap-identity Sybil of 2b does not raise f.
The disjointness assumption (a path-1 referral never overlaps path 2) is asserted, not
mechanized, in §2.1.2; that is a known simulation debt, not a defect. No finding.

### 2d. Collusion evidence and manifest replay (R7, R11)

| t_ms | node | rule (file §) | computation | state after |
|---:|---|---|---|---|
| 0 | colluders M1,M2 | Ch5 §5.3.1 | M1 relays T1 to M2 and M2 relays T1 to M1 in the same 60 s window, bytes within 10 % | one symmetric same-tree edge |
| 0 | auditor | Ch5 §5.3.1 | W(M1->M2)/W(M2->M1) ~ 1.0 in T1; honest same-tree flow is one-directional (one parent per tree) | symmetry_score = 1.0 |
| 0 | auditor | Ch5 §5.3.1 confidence | opportunity = min(N_guard - 1, c_a + c_p) = min(999, 40) = 40; degree(M1) = 1 -> confidence = 1 x (1 - 1/40) = 0.975 > 0.5 | SUSPICIOUS; accusation formed |
| 0 | accuser a1 | Ch5 §5.3.3; App D §D.4.18 | REPUTATION_AUDIT_GOSSIP(SYMMETRIC_TREE_PAIR: two PoU bodies, same TreeID, 60 s, 10 %) + a1's own RANK_PROOF (rank >= r_accuser = 0.25) | a1 is one countable accuser |
| ~200 | receiver | Ch5 §5.3.3 | re-verifies both receipt signatures, same tree, window, ratio; ranks a1 via attached proof whose nonce manifest it still holds (< tau_retain = 8 s) | count[M1] = {a1} |
| +X | a2, a3 | Ch5 §5.3.3 | 2 more accusers from 2 more /24 prefixes within tau_accuse = 10 min | count[M1] = {a1,a2,a3}, 3 prefixes |
| evict | receiver | Ch5 §5.3.3 | 3 distinct accusers, rank >= 0.25, >= 3 prefixes -> evict M1 for tau_accuse = 10 min, doubling, capped 24 h | M1 severed locally and bounded |
| rate | receiver | Ch5 §5.3.3 | at most 4 accusations/accuser/minute counted; older than 10 min not counted | flood and hoard-release bounded |

ISSUE-068 (open, High): the same SYMMETRIC_TREE_PAIR evidence is produced by **honest** parent-
child role swaps under migration and preemption; one simulated run evicted 104 honest nodes. The
spec text (Ch5 §5.3.1) still defines the evidence as "same TreeID, 60 s, bytes within 10 %". The
walk reaches the same evidence predicate, so the honest-swap false positive is on the clock here.
**already_known = ISSUE-068.** The implementation (`audit.rs` `symmetric_pair`, lines 165-180)
adds `a.segment == b.segment && bitmaps_intersect(...)` -- the "same segment and one common
block" tightening ISSUE-068 proposes -- so the impl is ahead of the spec text on this point.

Manifest replay:

| t_ms | node | rule (file §) | computation | state after |
|---:|---|---|---|---|
| 0 | replayer -> V | Ch7 §7.1.2 | resends MANIFEST(seg, chunk) the source signed earlier; signature valid against pinned PK_Source | step 2 passes (not FORGED) |
| 0 | V | Ch7 §7.1.2 step 3 | (seg, chunk) already in `verified` with the same root -> DUPLICATE: ignored silently, never penalised | replayer not banned (correct: every peer hears each manifest from up to M parents) |
| 0 | V (stale replay) | Ch7 §7.1.2 step 4 | if X_edge >= 0 and not (X_edge - tau_retain <= seg <= X_edge + 2): OUT_OF_WINDOW; tau_retain = 8 s = 8 segments, lookahead 2 | an old segment (> 8 behind) or a future one (> 2 ahead) dropped, not penalised |
| 0 | V (future forge) | Ch7 §7.1.2 | replayer cannot sign a *new* (seg, chunk); a fabricated future manifest fails step 2 -> FORGED -> disconnect + ban | forgery bounded to the 2-segment lookahead and only with SK_Source |

Replay is bounded by sequence, not wall clock. The acceptance window tau_retain = 8 s and
lookahead 2 (App B) are what bound it; the signed timestamp is not an acceptance criterion
(Ch7 §7.1.2 "Why There Is No Wall-Clock Check"). This is the correct resolution of the old
+/-2 s rule (ISSUE closed; SOLUTION recorded). No finding on replay.

## 3. Requirement checks

| Req | Verdict | Evidence |
|---|---|---|
| R7 (no byte forwarded before verification; no self-declared address; identity bound to address) | pass | 2a tick 102: forged block dropped before any symbol leaves V; verify-before-forward holds per hop (Ch4 §4.1.2). 2b: identities bound to observed IP, prefix-capped. Spec-level pass; impl weakens it (F6, F7) |
| R11 malicious injectors blocked at one hop | pass (spec) | 2a: corrupted block never propagates past V; child of V in T3 gets zero bytes. Impl allows 3 forged blocks before eviction (F7) |
| R11 10 % malicious injectors | pass | 2a: cost per forgery is ~5 us hashing + one per-tree repair at V; no cascade (per-tree CHURN_REPAIR, other 5 trees keep flowing) |
| R11 75 % symmetric-NAT served without TURN | not_applicable | not exercised by this scenario |
| R6 device that cannot upload still gets base layer | pass | 2b: the per-prefix cap bounds theft to 1 L0 child slot per honest parent per tree; L0 is never shed and never taken from a sitting child (App B leaf share) |
| R9 PSR <= 0.1 % | pass | 2a: both the 80 ms and 250 ms repairs complete inside W_pull before the 3.0 s deadline; PSR contribution 0 |

## 4. Candidate findings

**F3 (filed below, Low/Medium):** Ch4 §4.1.2 says a block that fails verification has its
delivering parent "flagged (Ch5 §5.3)"; Ch5 §5.3.3 says poisoning "is punished locally
(disconnect and local ban, Ch7 §7.1.1)". The two consuming sections name different consequences
(flag-and-score vs immediate disconnect-and-ban) and neither states a threshold. Two reasonable
implementers diverge on how many forged blocks evict a parent -- the reference impl chose 3,
Ch5's text reads as 1. Load-bearing: it sets the free poisoning budget per prefix per cooldown.

**F6 (impl):** The diversity-relative admission cap `max(1, ceil(K/min(P_obs,20)))` that Ch2
§2.2.2 and Appendix C make the operative Sybil/eclipse defence is not applied at the k-bucket,
Active Set or parent set in `src/crates/bs-core/src/dht.rs` / `node/membership.rs`. Prefix keys
exist only for eviction cooldowns and accuser diversity. At N = 1,000 from one /24 a victim's
bucket and Active Set can be filled by Sybils, the exact topology the cap forbids. Divergence
from spec, tagged impl.

**F7 (impl):** `node.rs try_verify` evicts a block-forging parent only after **3** rejected
blocks and records it in an un-aged `bad_peers` set (permanent), where Ch7 §7.1.1 / Ch5 §5.3.3
specify local ban tau_ban = 10 min doubling to 24 h. A forged MANIFEST (`BadSignature`) is only
logged, with no disconnect or ban, where Ch7 §7.1.2 requires "disconnect + local ban of the
delivering peer". Divergence from spec, tagged impl.

Dropped (debated and not filed): the eclipse bound 2c (holds, f is prefix-measured); manifest
replay 2d (correctly bounded by sequence window); the ISSUE-068 symmetric-pair false positive
(open ticket, already filed); SOLUTION-057 leaf-Sybil rank inflation (accepted residual risk,
bounded where rank is spent). The one new spec-level candidate is F3; F6 and F7 are impl
divergences surfaced by the cheap cross-check the task asked for.

## 5. Spec silences assumed through

- **S2:** Ch7 §7.1.1 bans "the peer that delivered it" without saying whether the ban keys on
  NodeID or on address prefix. Walked as NodeID (matching every other use of "peer"). This makes
  the 10 ms identity rotation of 2a defeat the doubling escalation entirely -- a prefix-keyed ban
  (as the eviction cooldowns in §5.1.3 and §5.2.1 are) would not. Candidate-adjacent but the
  §5.3.3 eviction path and the free-rider cooldown already key on prefix, so the omission is local
  to the poisoning ban; noted, not filed as its own ticket (one gap, F3's cluster).
- **S3:** the exact detection threshold (how many forged blocks before a parent is dropped) is
  undefined in the spec -- the core of F3.
- **S4:** the fold estimator s_hat_m reads guardian StarvedCount, which counts distinct querying
  NodeIDs; the spec does not cap StarvedCount per prefix. A Sybil minting identities (2b) and
  querying with a starved bit could inflate s_hat_m and force a spurious layer fold. Debated:
  s_hat_m is explicitly "a trigger, never a measurement" (Ch1 §1.1.5 §5.6) with a 10/30 s mean
  and a 5 % threshold, and the fold only ever removes the top layer (resolution loss, never a
  freeze), and unfold is relay-growth-gated, so the blast radius is bounded and the attacker pays
  one identity + one query per count. Accepted as within the coarse-trigger design; not filed.
- **S5:** disjointness of the alpha = 3 lookup paths (2c) is asserted in §2.1.2, not mechanized;
  simulation debt, not a defect.
