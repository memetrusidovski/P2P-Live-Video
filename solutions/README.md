# Solutions

Why the protocol is the way it is.

Each file records a resolved design problem: the decision taken, **the alternatives rejected and why**, the non-obvious consequences, the residual risk, and what still needs validating in simulation. The specification in [`../protocol/`](../protocol/) says *what* the protocol does; these say *why*, and what would break if someone changed it back.

Open defects live in [`../issues/`](../issues/).

---

## Index

### Cold start and small swarms

| # | Solution | Core decision |
|---|---|---|
| [001](SOLUTION-001-adaptive-join-threshold.md) | Adaptive JOINING threshold | $\theta_{\text{join}} = \max(1, \min(4, N-1))$ — a progress floor, not a target |
| [002](SOLUTION-002-dynamic-forest-sizing.md) | Dynamic forest sizing | $M$ scales with swarm size, with hysteresis and a 30 s dwell — *ladder input and minimum rung superseded by 038* |
| [038](SOLUTION-038-forest-ladder-minimum-and-coverage.md) | Ladder minimum and coverage | $M$ starts at 2 and is keyed on $N_{\text{relay}}$; empty trees are covered by a capacity-gated grant to the second-ranked relay |
| [003](SOLUTION-003-burst-join-bootstrap.md) | Burst-join bootstrap | Optimistic slots scale with the local burst, filled FIFO, bounded by $K_v$ |
| [005](SOLUTION-005-relay-warmup-gating.md) | Relay warm-up gating | Advertise $K_{\text{avail}} = 0$ until the first segment verifies — per tree |
| [006](SOLUTION-006-small-swarm-churn-recovery.md) | Small-swarm churn recovery | Tiered candidate sources, unioned by cost, probed concurrently |
| [020](SOLUTION-020-tree-aware-churn-recovery.md) | Tree-aware churn recovery | Passive set indexed by tree; per-tree `ROSTER` lists only relays of that tree |
| [026](SOLUTION-026-ladder-input-and-guardian-counts.md) | Ladder input and guardian counts | Guardians return relay/starved counts on `STORE_RECORD_ACK`; median over guardians; canonical Stream Record |
| [022](SOLUTION-022-rtt-scaled-liveness.md) | RTT-scaled liveness | Heartbeat on idle; $\tau_{\text{evict}} = \max(200, 2\tau_{\text{ping}} + SRTT + 4\,RTTVAR)$; source pacing; receipts on a segment clock |
| [036](SOLUTION-036-per-tree-repair.md) | Per-tree repair | `CHURN_REPAIR` is a sub-state of `ACTIVE` scoped to one tree; forwarding never stops |
| [052](SOLUTION-052-reattach-is-round-trips.md) | Re-attach is round-trips | Recovery is $\tau_{\text{evict}} + \approx 2\,RTT$; the PULL zone widens with the path, $W_{\text{pull}} = \text{clamp}(\tau_{\text{evict}} + 6\,SRTT, 1.5, 2.0)$ s |
| [056](SOLUTION-056-direct-shuffle-request.md) | Direct shuffle request | `SHUFFLE(TTL = 0)` over a session is answered by `GOSSIP_EXCHANGE` within $\tau_{\text{sched}}$ |
| [059](SOLUTION-059-refusal-is-a-frame.md) | Refusal is a frame | `DISCONNECT` carries `TreeID` and `REJECTED_SATURATED` / `NOT_ASSIGNED` / `DEPTH`; every `NEIGHBOR` is answered within $\tau_{\text{sched}}$; only a timeout drops a candidate |

### Capacity and topology

| # | Solution | Core decision |
|---|---|---|
| [007](SOLUTION-007-super-node-multi-tree-assignment.md) | Super-node multi-tree assignment | Trees ∝ full-stream-equivalents, one parent per NodeID, eligibility **earned** |
| [019](SOLUTION-019-layer-tree-allocation.md) | Layer→tree allocation | Source-built minimax mapping with explicit per-tree bitrate; subscribe and shed by **layer** |
| [025](SOLUTION-025-slot-count-with-overhead.md) | Slot count with overhead | $K_v(m) = \lfloor (1-r_{\text{pull}})u_v / (t_v B_m \Omega_v) \rfloor$ — one expression owns parity, framing and the PULL reserve |
| [027](SOLUTION-027-rendezvous-tree-assignment.md) | Rendezvous tree assignment | $\arg\max_m \text{Blake3}(NodeID \| m)$; a resize moves $1/(M{+}1)$ of relays and is switched at `EffectiveSegmentSeq` |
| [008](SOLUTION-008-capacity-score-and-weight-calibration.md) | Capacity score + weight calibration | Saturating $\sqrt{\cdot}$, every score term in milliseconds |
| [009](SOLUTION-009-scaled-active-set-high-fanout.md) | Scaled active set | $c_a^{\text{eff}} = \min(64, \max(c_a, \lfloor K_v/10 \rfloor))$; siblings carry repair, not the parent |
| [021](SOLUTION-021-depth-propagation.md) | Depth propagation | `BLOCK_PROOF.SenderHopDepth`; $h = h_{\text{parent}} + 1$ on every block; HysteresisMargin 30 ms |
| [015](SOLUTION-015-capacity-adaptation.md) | Capacity adaptation | Shed layers, never deepen; source reserve bounds the worst case at $3B_1$ |
| [016](SOLUTION-016-node-classes.md) | Node classes | PoW is universal; leaf-only is priced in QoS, not exempted — *leaf-fraction bound superseded by 048* |
| [040](SOLUTION-040-relays-keep-assigned-trees-and-publisher-fold.md) | Relays keep assigned trees; publisher fold | $\mathcal{T}_{\text{join}} = \mathcal{T}_{\text{sub}} \cup \mathcal{T}_{\text{assigned}}$; the source folds the top layer when a lower layer starves; §5.2 derived per tree |
| [048](SOLUTION-048-leaf-fraction-from-per-tree-condition.md) | Leaf fraction from the per-tree condition | $\ell \le 1 - M B_0 \Omega / ((1-r_{\text{pull}})\bar{u})$: $28\%$ at 8 Mbps, $M = 6$, not $51.6\%$; the lever is the mapping |
| [049](SOLUTION-049-leaf-share-by-displacement.md) | Leaf share by displacement | $\lceil 0.2 K_v(m) \rceil$ of $L_0$ slots claimable by leaves through displacing a pure-subscriber relay child; nothing held idle |
| [058](SOLUTION-058-minimax-kept-fold-tuned-for-base-layer.md) | Minimax kept; fold tuned for $L_0$ | $\tau_{\text{fold}} = 10$ s for base-layer starvation; unfold only on $\ge 50\%$ relay growth, never on a timer |
| [043](SOLUTION-043-pre-emit-new-trees-and-shed-grace.md) | Pre-emit new trees; shed grace | The source emits a new tree's stripe from the announcement; failed rounds against changed trees do not count until `EffectiveSegmentSeq` $+ D_{\max}$ |
| [044](SOLUTION-044-per-tree-probe-state.md) | Per-tree probe state | `PROBE_RESPONSE.TreeState` $\in$ {`SERVING`, `WARMING`, `UNPARENTED`}; segments start at 1 |
| [045](SOLUTION-045-two-budgets-one-expression-each.md) | Two budgets, one expression each | Tree budget owns slots and bridges; PULL reserve owns `PullSlots` and handover overlaps; sequential handover via `ACCEPTED.PENDING` |
| [046](SOLUTION-046-drain-notice.md) | `DRAIN_NOTICE` | The parent-side half of every drain: re-select now, I keep serving until the deadline |
| [047](SOLUTION-047-child-advertisement.md) | Child advertisement | `PROOF_OF_UPLOAD` carries `K_avail`/`AssignedTrees`/`NodeClass`; `NEIGHBOR` carries class and bitmap; the roster has inputs |
| [053](SOLUTION-053-depth-aware-hysteresis.md) | Depth-aware hysteresis | Margin is $\Delta_h(h)/2$ toward a shallower parent, 30 ms otherwise |

### Media plane

| # | Solution | Core decision |
|---|---|---|
| [004](SOLUTION-004-late-joiner-live-edge-sync.md) | Late-joiner live-edge sync | Signed Stream Record as a hint; the push path is the authority |
| [013](SOLUTION-013-block-symbol-verification-boundary.md) | Block/symbol verification boundary | One source block per Merkle block; decode is a no-op when nothing was lost |
| [031](SOLUTION-031-leaves-are-push-children.md) | Leaves are push children | No live-edge offset; leaf price is quality ceiling and preemption rank |
| [033](SOLUTION-033-chunked-manifests-and-latency-budget.md) | Chunked manifests | 250 ms signing chunks, `BlockIndex = Chunk << 12 \| j`, 3.0 s buffer, glass-to-glass ≈ 4.0 s stated in full |
| [060](SOLUTION-060-stream-descriptor.md) | Stream descriptor | Source-signed codec / container / `LayerMode` / initialisation data (`0x1F`), pointed to by the Stream Record, fetched with the segment-$X$ manifests; `LayerByteLength` in every manifest |

### Discovery and reachability

| # | Solution | Core decision |
|---|---|---|
| [028](SOLUTION-028-tree-aware-discovery.md) | Tree-aware discovery | One Peer Record (NodeID, class, tree bitmap, reachability, address) everywhere; `GET_PEERS` samples randomly per wanted tree |
| [032](SOLUTION-032-reachability-and-relay-bridging.md) | Reachability and relay bridging | `RELAY` requires `PUBLIC`/`CONE`; punch via the referrer; relays bridge leaves only, charged to the PULL reserve |
| [037](SOLUTION-037-guardian-load.md) | Guardian load | Registration sampled to ~$10^4$ at any $N$; no periodic `GET_PEERS`; guardians are reachable relays by construction |
| [050](SOLUTION-050-bridge-is-a-tree-edge.md) | Bridge is a tree edge | $R$ is $B$'s child, $A$ is $R$'s; bridges charged to the tree budget (≤ ½), reversing 032; ingress relays are budgeted roots |
| [051](SOLUTION-051-cone-guardian-keepalive.md) | `CONE` guardian keepalive | A `PING` to each guardian every 25 s keeps the punch path alive; the NAT-timeout assumption is stated |
| [054](SOLUTION-054-ch2-counts-and-records.md) | Ch2 counts and records | Only registration counts scale by $2^{s}$; active-only serving; Stream Record carries the pending matrix and `EffectiveSegmentSeq` |

### Security and incentives

| # | Solution | Core decision |
|---|---|---|
| [010](SOLUTION-010-collusion-detection-small-swarm-guard.md) | Collusion small-swarm guard | Degree judged against *opportunity*; guard input must be locally verifiable |
| [011](SOLUTION-011-adaptive-proof-of-work.md) | Adaptive Proof-of-Work | $C_2$ scales with $N$; discounts are alternatives, never cumulative |
| [023](SOLUTION-023-rank-admission-and-pull-scoped-choking.md) | Rank admission, PULL-scoped choking | Tree slots by rank with preemption in enhancement trees; Tit-for-Tat governs the PULL reserve only |
| [024](SOLUTION-024-aggregated-receipts-and-rank-proof.md) | Aggregated receipts and `RANK_PROOF` | One receipt per segment per tree; commit-then-sample proof with a future manifest root as nonce |
| [034](SOLUTION-034-pow-is-a-rate-limit.md) | PoW is a rate limit | ≈10 ms per identity; address-prefix caps are the Sybil bound, stated everywhere they apply |
| [035](SOLUTION-035-evidence-carrying-eviction.md) | Evidence-carrying eviction | Signed accusations with verifiable evidence; 3 ranked accusers from 3 prefixes; local, bounded eviction |
| [039](SOLUTION-039-per-tree-symmetry.md) | Per-tree symmetry | Collusion symmetry judged within one tree; mutual parents in different trees are honest |
| [041](SOLUTION-041-rank-proof-counts-distinct-receipts.md) | `RANK_PROOF` counts distinct receipts | Canonical-key sorted list, adjacent-pair samples, rank from sampled capped bytes; `TotalBytes` removed |
| [042](SOLUTION-042-diversity-relative-prefix-cap.md) | Diversity-relative prefix cap | $\max(1, \lceil K/\min(P_{\text{obs}}, 20) \rceil)$ per prefix; k-buckets hard at 1 of 20 |
| [055](SOLUTION-055-accusations-carry-rank-proof.md) | Accusations carry rank | `REPUTATION_AUDIT_GOSSIP` carries the accuser's `RANK_PROOF`; count only what you can rank, forward only what you counted |
| [057](SOLUTION-057-rank-is-a-cost-not-a-proof.md) | Rank is a cost, not a proof | The subnet penalty applies to resolvable signers only; forged rank via leaf-class Sybils is stated and bounded where rank is spent |
| [017](SOLUTION-017-xdp-ddos-shield.md) | XDP DDoS shield | The kernel enforces the *result* of validation, never the validation |
| [030](SOLUTION-030-sequence-windowed-manifest-acceptance.md) | Sequence-windowed manifests | Signature first, sequence window second, no wall clock; duplicates are never forgeries; bans are local and time-bounded |

### Structure

| # | Solution | Core decision |
|---|---|---|
| [012](SOLUTION-012-canonical-wire-format.md) | Canonical wire format | Binary frames normative, proto non-normative; no self-declared IP |
| [014](SOLUTION-014-unified-churn-recovery.md) | Unified churn recovery | Passive-set promotion is the primitive; election is a layer over it |
| [018](SOLUTION-018-canonical-documentation.md) | One canonical specification | `protocol/` wins; older generations keep their reasoning, lose their authority |
| [029](SOLUTION-029-control-paths-on-the-wire.md) | Control paths on the wire | Eight described-but-unencoded paths, each given its frame inside the cluster that owns its consumer |

---

## Design principles these converged on

Written down because each was learned from a defect, and each generalises past the issue that produced it.

1. **Express counts as shares of capacity, never as absolutes.** $K_v$ moves with $M$, which moves with $N$. Three separately-reasonable constants in the incentive layer were jointly unpayable. (003, 016)

2. **A parameter with multiple modifiers needs one normative expression for its effective value.** Three PoW discounts, each justified, compounded to 512× weaker than intended because no document owned the total. (011, 003, 004)

3. **When you relax a constraint, enumerate everything it was accidentally providing.** The Orthogonal Placement Rule was documented as anti-hotspot; it was *also* silently providing failure independence and a bound on capacity-lying damage. (007)

4. **A rule derived from small-$N$ intuition must be checked at large $N$ before being written unconditionally.** "A node with one peer had no choice" is true at $N=5$ and an immunity hole at $N=10^6$. (010, 001)

5. **Any input that can disable a security control must be locally verifiable.** `swarm_size` is fine for tuning; using it to gate a detector makes it an attack surface. (010, 001)

6. **Sentinel values must distinguish transient from persistent causes.** $K_{\text{avail}} = 0$ meant both "wait a second" and "never", and only one consumer needed the difference. (005)

7. **Fallback chains are unions ordered by cost, not sequences of substitutions.** `if primary inadequate: use secondary` discards partial primary results. (006)

8. **A degraded mode must be traced through every state machine and frame it touches.** Layer shedding was internally complete and left peers stuck in `CONNECTING`. (015)

9. **For a wire format, the byte diagram is the specification and the prose is commentary.** (012)

10. **Weights and the transform they weight are one calibration.** (008)

11. **Verify a fix at the boundaries of its range, not only at the extreme it was designed for.** (009, 014)

12. **Derived text drifts silently** — index entries and summaries restate facts without owning them. Grep the repo, not just the definition. (018)

13. **When two independently-scaled ladders meet, write the map for every rung and check that the failure order it induces matches the failure order the rest of the design assumes.** The first plausible layer→tree rule made $L_1$ starve before $L_2$. (019)

14. **A deterministic assignment function must be stable under every parameter it takes, not merely uniform at fixed parameters.** A modulus is uniform and reassigns 83% of nodes when $M$ changes by one. (027)

15. **Size a ladder against all of its costs at every rung.** "No splitting overhead" at $M = 1$ was true and irrelevant; slot granularity dominated. (038)

16. **A candidate list in a multi-tree protocol must say which tree each candidate serves, or it is a list of things to probe.** The passive set and the roster were both inherited from single-tree designs. (020, 028)

17. **For every value a decision consumes, name the frame that produces it and the node that sends it.** "Reported by the guardians" appeared in three chapters and was implemented by none. (026, 029)

18. **Anything every peer does on a timer toward a fixed set of nodes is an $O(N)$ hotspot; check it against the recipient's own rate limits.** The guardians' DDoS shield was dropping the swarm's registrations. (037)

19. **Assemble a latency budget end to end in one place, with every term owned by a named section.** A 1 s signing barrier lived implicitly in Chapter 4 and never reached the Chapter 1 sum. (033)

20. **Every reject path in a validator must say whether it is evidence of misbehaviour, and by whom.** One-bit validators turn redundancy into bans. (030)

21. **A timeout is a claim about the path; write it as a function of measured RTT, floor it, and quote the result as a range.** "Sub-250 ms" was true of a LAN and quoted for a planet. (022)

22. **A node with $M$ independent roles needs $M$ independent failure states.** One node-level repair state turned a $1/M$ loss into a subtree-wide cascade. (036)

23. **A value a node advertises about itself that depends on its ancestors must be refreshed on a path the ancestors already use.** Depth learned at join was wrong for most of the forest most of the time. (021)

24. **Name the resource each incentive mechanism allocates, and check that the signal it reads is nonzero for the peers competing for that resource.** Tit-for-Tat reads reciprocal flow; tree edges have none. (023)

25. **Anything signed per unit of data must be re-costed at the maximum fan-out the design permits; anything presented to a verifier needs a bounded size and a sampling rule the presenter cannot steer.** (024)

26. **Every "X is expensive" claim needs a number, the hardware it was measured on, and the adversary's hardware.** Ten milliseconds is a price for a phone and free for a data centre. (034)

27. **A distributed punishment must be checkable by every node that applies it, and its cost must fall on the accuser when it is wrong.** An unverifiable accusation is an attack primitive. (035)

28. **A "honest traffic never looks like X" heuristic must be checked against every topology the protocol's own placement rules generate.** Mutual parents are the expected shape of a mid-sized forest, not an edge case. (039)

29. **A per-peer adaptation cannot move capacity that a global assignment has pinned.** Shedding at the peers freed nothing for the base layer while a third of the relays sat on a layer nobody could afford; only the owner of the assignment could re-map, and it needed a rule that fires on the adaptation's failure signal. (040, 048)

30. **A sampling proof establishes only what the sample can be compared against.** Eight genuine leaves said nothing about how many leaves there were; a count needs a structure in which a wrong count is locally visible, and a total must be estimated from the sample, never declared. (041)

31. **A percentage cap on a small integer set is a rounding rule in disguise.** Five percent of six parents is a policy decision; write the rounding, and write the condition under which the cap applies at all. (042, 049)

32. **A migration step phrased as "every peer does X before T" must name the mechanism that makes X possible before T.** If the resource X needs is created at T, the step is a wish. (043)

33. **Every budget needs one inequality listing all of its consumers, and a consumer added in another chapter is added to the inequality or not added.** The PULL reserve was spent three times. (045, 050)

34. **A graceful-release window is worth nothing unless the released party learns the window has started.** Five drain rules, one frame missing. (046)

35. **For every field a frame lists, name the frame that delivered it to the sender.** The roster was an output with no inputs. (047, 055)

36. **When one term of a timeline is made RTT-scaled, re-derive every other term that contains a handshake on the same RTT.** (052)

37. **A defence is only as real as its inputs at the node that applies it.** The subnet penalty needed addresses no verifier had; the fix is to say where it applies, say what it leaves open, and bound the residue where the forged value is spent. (057)

38. **Never probe a state whose failure mode is a freeze.** A timer-driven unfold re-created the base-layer starvation that caused the fold, on schedule, forever. (058)

39. **Every request needs a defined negative answer that tells the requester what to do next.** `ACCEPTED` alone was half a handshake, and the half that was missing decided whether a full relay was forgotten. (059)

40. **A payload-agnostic protocol still has one consumer that is not, and it needs a signed, versioned description on the same path as the first byte it will consume.** (060)

41. **A reversal is cheaper than a workaround.** SOLUTION-032's reserve charge was wrong in kind, not in constant; the fix names it and reverses it rather than raising the constant until it stops hurting. (050)

---

## Validation debt

Every solution ends with a "Validation owed (Chapter 8)" section. These are not optional polish: several fixes introduce constants chosen by argument rather than measurement — the `sqrt` exponent and its weights, the $M$ ladder thresholds and hysteresis, $c_a^{\text{eff}}$ scaling, $K_{\text{ref}}$, $R_{\text{roster}}$, the shed constants, $f_{\text{frame}}$ and $r_{\text{pull}}$ (025), the minimax allocation tie rule (019), $\Delta_{\text{buffer}} = 3.0$ s (033), the $4\,RTTVAR$ jitter multiplier (022), $\beta_{\text{pull}}$ and the $1.25\times$ preemption margin (023), the 8-receipt sample (024) — now 8 adjacent pairs and the $1.5\times$ per-receipt cap (041), the three-accuser/three-prefix eviction threshold (035), the fold trigger $s_{\text{fold}} = 5\%$ over 10/30 s and the growth-gated unfold (040, 058), $\tau_{\text{grace}} = 8$ segments (043), the leaf share $\lceil 0.2 K_v \rceil$ (049), the half-budget bridge cap (050), $\tau_{\text{nat}} = 25$ s (051), the $W_{\text{pull}}$ clamp (052), the $\Delta_h/2$ margin (053), the $P_{\text{obs}}$-relative prefix cap (042), and the 4 KB `InitData` cap and five-segment descriptor lead (060). The simulation plan in [`../protocol/chapter8_simulation/`](../protocol/chapter8_simulation/) is where they get settled.
