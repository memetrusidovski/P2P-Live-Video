# 2. The Parent Selection & Scoring Algorithm

## 2.1 The Multivariate Scoring Function

When a peer $i$ wishes to join tree $T_m$ as a child, it must select the best possible parent from a pool of candidates discovered via the S/Kademlia DHT (`GET_PEERS` with `WantedTrees` naming $m$, Ch2 §2.3.2) and its own per-tree passive pool $\mathcal{P}_m$ (Ch3 §3.1.1). Both sources deliver Peer Records, so every candidate is known to relay $T_m$ and to be reachable before it is probed. Selecting a random parent leads to high latency and geographic inefficiency. Selecting only based on bandwidth creates hotspot bottlenecks.

To solve this, peer $i$ evaluates every candidate parent $p$ using a continuous **Multivariate Scoring Function**:
$$\text{Score}(p, i) = w_c \cdot \text{CapacityCredit}(p) \;-\; \text{RTT}(p, i) \;-\; w_h \cdot \text{HopPenalty}(p)$$

All three terms are expressed in **milliseconds of effective penalty**, so they are directly comparable and the score reads as "how much better than a 0 ms, hop-0, zero-capacity parent is this candidate." Keeping the terms commensurable is not cosmetic: see [§2.1.1 Weight Calibration](#211-weight-calibration) for what goes wrong when they are not.

### Component 1: Capacity Score
The parent $p$ gossips its available remaining upload slots ($K_{\text{avail}}$) and historical reliability ($R \in [0, 1]$, representing the percentage of chunks delivered on time).
$$\text{CapacityCredit}(p) = \min\left(\sqrt{K_{\text{avail}}},\ \sqrt{K_{\text{ref}}}\right) \cdot R, \qquad K_{\text{ref}} = 256$$
*We use square-root compression so that a peer with more slots is scored higher, but not proportionally higher, encouraging load distribution. An earlier draft used $\ln(1 + K_{\text{avail}})$, but the log is too flat at the top end: it gave a 10 Gbps server (10,000 slots) only a 3.8× advantage over a 10 Mbps node — so nearby mid-tier peers routinely outscored available backbone capacity. The square root yields a 31.6× advantage in that comparison: strong enough to route toward super nodes, still far from the raw 1000× that would cause monopolisation. The exponent is a tuning parameter to be validated in simulation (Chapter 8) before deployment.*

**Warm-Up Gating:** A freshly joined relay has no data to forward until it has received and Merkle-verified its first complete segment (~1 second of buffering). During this window it MUST advertise $K_{\text{avail}} = 0$, which removes it from parent selection entirely (probes are skipped by the `available_slots > 0` check below). Without this rule, a new relay's high capacity score attracts children who then receive nothing for a full second — the relay answers keepalives (so it is never evicted as dead) while its empty bitfield forces every child into mesh-PULL fallback, compounding into latency spikes during rapid-growth phases when many relays are warming up at once.

**Warm-up is per tree, not per node.** A multi-tree super node (§1.3) may hold verified segments for $T_1$ while still warming up in $T_4$. It advertises its real slot count in the trees it can serve and zero in the trees it cannot, so one warming assignment never hides its whole capacity from the forest.

```python
# Capacity gossip advertisement, evaluated per assigned tree:
def advertise_K_avail(tree_id):
    if len(verified_segment_buffer[tree_id]) == 0:
        return 0                                    # warm-up: hidden from selection
    B_m = tree_mapping[tree_id].bitrate             # the tree's OWN declared bitrate (§4.2.1)
    K_v = floor((1 - R_PULL) * u_v / (t_v * B_m * omega_v))   # per-tree slot count (§1.3)
    return K_v - current_children[tree_id]
```

`K_avail` is always computed against the tree's declared bitrate from the slicing matrix, never against the nominal $B/M$: on the reference $M = 6$ mapping a $1.5$ Mbps $L_2$ stripe holds half the slots of a $0.75$ Mbps $L_0$ stripe on the same node.

**Warming and saturated must be distinguishable.** Both states advertise $K_{\text{avail}} = 0$, but they mean opposite things to a joiner: a warming relay will have capacity in under a second, while a saturated one will not. Conflating them makes a growth transient look identical to a capacity shortage, and the shed rule of [§1.1.5](../1.1_scale_latency/5_capacity_adaptation.md) would then drop a quality layer for a condition that resolves on its own.

`PROBE_RESPONSE` (0x0F) therefore carries a **per-tree** `TreeState` for the probed tree in `Flags` bits 5–6 (Appendix D §D.4.7), and its `LiveEdgeSegmentSeq` is the highest segment the responder has verified **in the probed tree** — not the node-level live edge of Ch4 §4.3.1. An earlier draft keyed the distinction on a node-level `LiveEdgeSegmentSeq = 0`, which classified every relay that was warming in one tree while serving another — a super node earning a new tree, a coverage-grant acceptor, every relay that moves at a resize — as *saturated*, and so fired the shed rule on precisely the growth transients the exclusion was written for.

| `TreeState` | Meaning | $K_{\text{avail}}$ | Joiner's response |
| :--- | :--- | :---: | :--- |
| `SERVING` | Has a parent and $\ge 1$ verified segment in this tree | $\ge 0$ | $> 0$: score and join. $= 0$: **saturated** — counts toward the shed threshold |
| `WARMING` | Has a parent in this tree, no verified segment yet | $0$ | Retry — capacity is imminent; the round does **not** count |
| `UNPARENTED` | Assigned to this tree but currently without a parent in it (§1.1.5 §5.3 item 4) | $0$ | Cannot serve; counts as a failed candidate — it is a victim of the same shortage |

Segment sequence numbers start at $1$ (Appendix D §D.4.8), so `LiveEdgeSegmentSeq = 0` unambiguously means "nothing verified in this tree".

### Component 2: Latency Penalty (RTT)
Peer $i$ pings candidate $p$ over UDP to establish the current Round Trip Time (RTT) in milliseconds. It enters the score directly, at unit weight — it *is* the millisecond scale the other two terms are calibrated against.
$$\text{LatencyPenalty}(p, i) = \text{RTT}(p, i)$$

### Component 3: Hop Penalty (Layer Depth)
To prevent the tree from becoming too deep, parent $p$ advertises its hop-distance $h_p$ from the source in tree $T_m$.
$$\text{HopPenalty}(p) = e^{\lambda \cdot h_p} - 1, \qquad \lambda = 0.5$$
*The exponential penalty ensures that joining deeply nested nodes (e.g., Hop 10) is heavily penalized compared to joining a node at Hop 2, aggressively keeping the tree shallow. The $-1$ makes the penalty zero at $h_p = 0$ (the source itself), so the term measures added depth rather than carrying a constant offset.*

### Depth Propagation

$h_p$ must be the parent's **current** depth, not the depth it had when it joined. Three mechanisms move whole subtrees after join — greedy upward migration every 5 s (§2.3), Deputy re-attachment after a parent dies (§3.3), and forest resize (§4.5) — and at $N = 10^6$ with every node running the migration loop, depth change is the steady state. A node that learned its depth once, as `ACCEPTED.HopDepth + 1`, and never again would advertise a stale value to every joiner that probes it: stale-*high* after an ancestor migrated up (honest parents look saturated, joiners shed a layer for no reason), stale-*low* after a Deputy re-attached deeper (the forest silently exceeds $D_{\max}$).

Depth therefore rides on the frame every parent already sends every child several times per second. `BLOCK_PROOF` (Appendix D §D.4.9) carries `SenderHopDepth`; a node sets

$$h_{\text{self}}(m) = \text{SenderHopDepth}_{\text{latest}}(m) + 1$$

on every block it receives in tree $m$, and advertises that value in its own `BLOCK_PROOF`s, `PROBE_RESPONSE`s and `ACCEPTED`s from then on. A depth change at depth $d$ reaches depth $D_{\max}$ within one block interval per level — under $1.5$ s on the slowest reference tree ($0.75$ Mbps, one block per $\approx 170$ ms). `ACCEPTED.HopDepth` remains the *initial* value; the block stream is the authority.

Two rules follow for a node whose depth **rises**:

*   At $h \ge D_{\max}$ it **stops accepting children** in that tree and drains the ones it has through the drain path below (`DRAIN_NOTICE`, reason `DEPTH`, scope *all children*) — they re-select, and their own scores now correctly penalise the deep branch. It also runs the upward-migration step (§2.3) immediately rather than waiting for the 5 s tick.
*   It is *not* an error: a Deputy that re-attaches under a passive-set candidate at $h = 6$ has done the right thing for the next 250 ms; the depth rule then unwinds the branch over the following seconds instead of leaving it there forever.

## 2.1.1 Weight Calibration

| Weight | Value | Meaning |
| :--- | :---: | :--- |
| $w_c$ | $12$ | ms of credit per unit of $\sqrt{K_{\text{avail}}}$ |
| $w_h$ | $20$ | ms per unit of exponential depth penalty |
| $K_{\text{ref}}$ | $256$ | slot count at which capacity credit saturates ($192$ ms max) |

**The weights are a function of the compression curve and must be re-derived whenever it changes.** The earlier draft paired the weights $(1000, 1, 50)$ with $\ln(1 + K_{\text{avail}})$, whose output spans only $2.4 \to 9.2$ across the entire plausible range of $K_{\text{avail}}$ — a $3.8\times$ spread, comparable to the hop term, which is what made those weights balanced. Substituting $\sqrt{\cdot}$ widened that span to $3.2 \to 100$, a $31.6\times$ spread, while leaving the weights untouched.

The consequence is not subtle. Under $(1000, 1, 50)$ with $\sqrt{\cdot}$, a 10 Gbps server on another continent scores $\sqrt{10^4} \cdot 1000 - 300 - 50e^{0.5} \approx 99{,}618$, against $\approx 3{,}070$ for an excellent peer in the same city. Capacity outweighs latency by more than thirty to one, so **every** peer in the world routes to the super node until it saturates — and it has 10,000 slots, so it takes a long time to saturate. The forest becomes globally scattered, and the $RTT_{\text{avg}} = 80\text{ ms}$ assumption underpinning the latency proof of §1.1.3 no longer holds. Fixing the flatness of the log by widening the range, without rebalancing, replaced under-use of backbone capacity with latency-blind monopolisation of it.

Two changes restore balance:

*   **Capacity saturates.** Beyond $K_{\text{ref}} = 256$ free slots, more slots do not make a parent better *for this child* — they make it better for the swarm, which is realised by it accepting many children, not by it outscoring every alternative for each one. Credit is capped at $12 \cdot 16 = 192\text{ ms}$.
*   **Everything is in milliseconds.** Each term's contribution is legible and boundable rather than an arbitrary product of dimensionless weights.

Worked comparisons at $R = 1$:

| Candidate | RTT | $h_p$ | $K_{\text{avail}}$ | Score | Outcome |
| :--- | :---: | :---: | :---: | :---: | :--- |
| Local mid-tier peer | 20 | 3 | 10 | $38 - 20 - 70 = -52$ | preferred |
| Distant super node | 250 | 1 | 10,000 | $192 - 250 - 13 = -71$ | not worth the RTT |
| **Local** super node | 20 | 1 | 10,000 | $192 - 20 - 13 = +159$ | strongly preferred |
| Nearly-saturated local peer | 20 | 3 | 1 | $12 - 20 - 70 = -78$ | avoided |
| Deep local peer | 20 | 7 | 10 | $38 - 20 - 643 = -625$ | strongly avoided |

Backbone capacity now wins when it is *reachable*, loses to a good local peer when it is not, and the depth term still dominates everything — which is what keeps the tree shallow. All three weights remain tuning parameters to be validated in simulation (Chapter 8) before deployment.

---

## 2.2 The Tree Join Algorithm (Pseudocode)

The following sequence dictates exactly how a node connects to the network upon completing its DHT bootstrap phase.

```python
import time

def execute_tree_join(node_i, target_tree_m, dht_interface, passive_pool):
    # Step 1: Discover Candidates — Peer Records known to relay tree m (Ch2 §2.3.3)
    records = passive_pool[target_tree_m] \
            + dht_interface.get_peers(stream_id, wanted_trees=bit(target_tree_m))
    candidate_list = [r for r in records
                      if r.node_class == RELAY
                      and r.assigned_trees.has(target_tree_m)
                      and r.reachability != SYMMETRIC
                      and r.node_id not in node_i.current_parents]      # distinct-parent rule (§1.3)

    if not candidate_list:
        # Coverage grant (§1.4): no relay is assigned to m — ask a relay whose
        # SECOND-ranked rendezvous tree is m, before falling back to the source.
        candidate_list = [r for r in records if rendezvous_rank(r.node_id, M)[1] == target_tree_m]
        candidate_list.append(source_record)

    scored_candidates = []

    # Step 2: Parallel Probing
    for p in candidate_list:
        if p.reachability == CONE:
            send_punch_request(referrer_of(p), target=p.node_id)      # App D §D.4.14, +1 RTT
        # Send lightweight UDP probe to measure RTT and fetch current Hop Count/Capacity
        probe_response = send_udp_probe(target=p.ip, tree=target_tree_m, timeout_ms=200)
        
        if probe_response.success and probe_response.available_slots > 0:
            # Step 3: Execute Scoring Function
            W_C, W_H, K_REF = 12.0, 20.0, 256   # ms/unit, ms/unit, slot cap (§2.1.1)

            cap_credit  = min(math.sqrt(probe_response.available_slots),
                              math.sqrt(K_REF)) * probe_response.reliability
            lat_penalty = probe_response.rtt_ms
            hop_penalty = math.exp(0.5 * probe_response.hop_count) - 1.0

            # All three terms are in milliseconds and directly comparable
            final_score = (W_C * cap_credit) - lat_penalty - (W_H * hop_penalty)
            scored_candidates.append( {"peer": p, "score": final_score} )

    # Step 4: Sort and Dispatch
    # Sort descending so the highest score is index 0
    scored_candidates.sort(key=lambda x: x["score"], reverse=True)
    
    for best_candidate in scored_candidates:
        # Send formal join request via QUIC: NEIGHBOR(TreeID = m). Answered within tau_sched by
        # ACCEPTED or by DISCONNECT(TreeID = m, Reason in REJECTED_*) — App D §D.4.3b. Never silence.
        join_ack = send_quic_join_request(best_candidate["peer"], target_tree_m)

        if join_ack == ACCEPTED:
            register_active_parent(best_candidate["peer"], target_tree_m)
            return SUCCESS
        if join_ack == REJECTED_NOT_ASSIGNED:
            passive_pool.clear_tree_bit(best_candidate["peer"], target_tree_m)   # stale record; keep the peer
        # REJECTED_SATURATED / REJECTED_DEPTH: the candidate is full, not bad — keep its record,
        # count it toward this round's failure. TIMEOUT: unreachable — drop the record.

    # If all candidates rejected (e.g., they filled their slots during probing).
    # The retry re-issues get_peers(): every GET_PEERS draws a fresh random sample.
    return FAILURE_RETRY_BACKOFF
```

### Depth Admission Rule

A parent **must reject** any join request that would place the child deeper than $D_{\text{max}} = 8$ hops from the source — that is, a candidate advertising $h_p \ge D_{\text{max}}$ is not a legal parent. A saturated forest may never resolve pressure by growing deeper than its latency budget allows. The refusal is `DISCONNECT(TreeID = m, REJECTED_DEPTH)` (Appendix D §D.4.3b).

**A refusal is a frame, not a silence.** Every outcome below that reads `REJECTED` is sent as `DISCONNECT` carrying the request's `TreeID` and a refusal reason: `REJECTED_SATURATED` for cases 3–5 of the Rank Admission Rule, `REJECTED_NOT_ASSIGNED` when the responder does not relay $T_m$ (a stale `AssignedTrees` bit somewhere — the requester corrects its record and keeps the peer), `REJECTED_DEPTH` for the rule above. The joiner acts on the reason: saturated and depth refusals count toward the shed threshold and keep the candidate's record, since the peer is full rather than bad; only a timeout drops it. An earlier draft returned the values `REJECTED` and `REJECTED_NOT_ASSIGNED` from this algorithm and defined no frame that produced them, so a refusal was indistinguishable from a dead peer and cost a full probe timeout per candidate.

### Rank Admission Rule

The protocol's core axiom is that contribution buys placement and quality. Tree slots are where placement and quality are decided, so this is the rule that implements the axiom; without it a 10 Gbps super node's slots go to whoever probes first and nothing ever moves a free-rider out of the way of a contributor.

A `NEIGHBOR(TreeID = m)` join request carries the joiner's `NodeClass` and `AssignedTrees` (Appendix D §D.4.3) and may be followed on the same QUIC stream by a `RANK_PROOF` (Ch5 §5.2.2) establishing the joiner's rank $r_{\text{new}} = \Theta^{\text{rate}} / B$ — full-stream-equivalents the joiner is currently delivering to others. A request with no proof, or from a leaf-class node, has $r_{\text{new}} = 0$. The parent decides:

1.  **Free slot in $T_m$** (respecting the depth rule above): `ACCEPTED`.
2.  **No free slot, $T_m$ carries an enhancement layer, and $r_{\text{new}} > 1.25 \cdot r_{\min}$** where $r_{\min}$ is the rank of the parent's lowest-ranked current child in $T_m$: `ACCEPTED`, and the parent **preempts** that child through the drain path below (reason `PREEMPTED`). At most one preemption may be in progress per tree. Leaves and newcomers rank $0$, so any contributor preempts them; a $1.25\times$ margin stops two near-equal contributors from swapping a slot back and forth.
3.  **No free slot, $T_m$ carries $L_0$, the requester is leaf-class, and leaf-class children hold fewer than the leaf share $R_{\text{leaf}}(m) = \lceil 0.2\,K_v(m) \rceil$ of the parent's slots in $T_m$**: `ACCEPTED`, and the parent **displaces** its lowest-ranked relay-class child in $T_m$ *that does not relay $T_m$* (bit $m-1$ of its `AssignedTrees` clear — a pure subscriber with no children in this tree, so nobody is orphaned), through the drain path (reason `DISPLACED`). Ties by most recent admission. A child that relays $T_m$ is never displaced. This is the universal service floor of Ch1 §1.1.5 §5.5 and §1.2.5 §5.5 — the one bounded exception to the rule that base-layer slots are never taken from a sitting child, scoped to leaf requesters and to the leaf share. If every relay-class child relays $T_m$, or the leaf share is already held by leaves: `REJECTED`.
4.  **No free slot, $T_m$ carries $L_0$, otherwise**: `REJECTED` — base-layer slots are **never** rank-preempted. The joiner's next step is the retry path below and, at the margin, the source's base-layer reserve.
5.  **Otherwise**: `REJECTED`.

Two properties follow. **Bootstrapping is through the base layer**: a new relay ranks $0$, obtains an $L_0$ slot wherever one is free (the ladder and the source reserve make that the common case), relays $L_0$ to children, earns receipts, and within a minute has the rank to preempt into enhancement trees. **Shallower placement emerges** from the migration loop (§2.3) plus preemption: a high-rank node's scoring prefers shallow parents, it requests them, and it displaces a lower-ranked child there — so contributors drift toward the source and free-riders toward the leaves, which is what "contribution buys placement" means concretely.

### The Drain Path

Five rules release a child without dropping it: preemption and displacement (above), a relay moving at a forest resize (§4.5), a `RELAY` → `LEAF` demotion (§5.3), a slot count $K_v(m)$ that falls when parity rises or the mapping changes (§1.3), and a node whose depth reaches $D_{\max}$ (*Depth Propagation*). All five use one mechanism:

1.  The parent sends **`DRAIN_NOTICE`** (0x1E, Appendix D §D.4.19) on the tree's QUIC stream: `TreeID`, a `Reason` from the `DISCONNECT` code space, a `Scope` (*this child only* or *every child in this tree*) and a `DeadlineSegmentSeq` — the current segment plus $\tau_{\text{drain}} = 5$ segments, or `EffectiveSegmentSeq` for a resize. It **keeps serving** the child.
2.  The child immediately runs the Ch3 §3.3 promotion for tree $m$ **with its current parent still delivering** — a *warm* repair, with no eviction timeout and no gap. With scope *every child*, the children run the Deputy election of §3.2 over their most recent `ROSTER` exactly as they would for a dead parent, using the notice as the trigger; a child alone in scope promotes directly.
3.  When the new parent begins delivering, the child sends `DISCONNECT_CHOKE` to the old parent, which frees the slot. The parent sends `DISCONNECT(Reason)` at the deadline to any child that has not left.

The notice is what makes the drain window worth having. Without it the child learns of its release only at `DISCONNECT`, after the window, and then repairs with exactly the gap the window was meant to avoid; an earlier draft said the parent "announces" the change and defined no frame that did so.

**Handover budget.** A preemption or displacement means the parent serves one child beyond $K_v(m)$ until the drained child leaves. That transient slot is charged to the parent's PULL reserve under the owning expression of §1.3 — $B_m \Omega_v$ per handover in progress — and PULL slots are re-computed from the remainder at the next Tit-for-Tat cycle. If the reserve cannot hold $B_m \Omega_v$ even with every PULL slot choked (any relay below $10\,B_m\Omega$: a $10$ Mbps relay in a $1.5$ Mbps tree), the handover is **sequential**: the parent answers `ACCEPTED` with `AcceptFlags.PENDING` (Appendix D §D.4.4) and begins pushing to the joiner only when the drained child's slot is released; the joiner keeps any parent it already has meanwhile. An earlier draft charged the overlap to the reserve unconditionally, which on the reference $10$ Mbps relay meant a link at $107\%$ for five seconds on every preemption in a $1.5$ Mbps tree, and $125\%$ in a $3.0$ Mbps tree.

A preempted or displaced child that finds every candidate saturated counts the round toward the shed rule like any failed round.

### On `FAILURE_RETRY_BACKOFF`

This return value is the protocol's **capacity-saturation signal** for tree $T_m$: every candidate either had no free slots or sat at the depth limit. Its handling is defined normatively in [Chapter 1 §1.1.5 Capacity Adaptation](../1.1_scale_latency/5_capacity_adaptation.md): the peer retries with backoff, and after **2 consecutive failed rounds** sheds tree $T_m$ (dropping to the next lower SVC layer) rather than continuing to search, with a 10-second hysteresis before the standard upward-migration loop attempts re-join. The base-layer tree is never shed.

## 2.3 Continuous Optimization (Greedy Upward Migration)

Parent selection is not a one-time event. Trees degrade over time due to churn. Therefore, nodes run a background thread every $5\text{ seconds}$ to continuously optimize the tree.

1. Node $i$ currently has parent $P_{\text{current}}$ with $\text{Score}(P_{\text{current}})$.
2. Through the gossip layer (HyParView Passive Set), node $i$ learns of a new node $P_{\text{new}}$.
3. If $\text{Score}(P_{\text{new}}) > \text{Score}(P_{\text{current}}) + \text{Margin}$, where

    $$\text{Margin} = \begin{cases} \min\left(30\text{ ms},\ \tfrac{1}{2}\,\Delta_h(h_{\text{new}})\right) & \text{if } h_{\text{new}} < h_{\text{current}} \text{ (the candidate is shallower)} \\ 30\text{ ms} & \text{otherwise} \end{cases}
    \qquad \Delta_h(h) = w_h \left(e^{\lambda (h+1)} - e^{\lambda h}\right)$$

    with $\text{HysteresisMargin} = 30$ ms (Appendix B) and $\Delta_h(h)$ the score value of one hop of depth at depth $h$: $13.0$ ms for $1 \to 0$, $21.4$ for $2 \to 1$, $35.3$ for $3 \to 2$, $58.1$ for $4 \to 3$. An earlier draft justified a flat 30 ms as "small enough that one hop of depth (at least $\approx 55$ ms at $h \le 3$) always wins"; $55$ ms is the *largest* one-hop swing at $h \le 3$, not the smallest, and under a flat margin a node at depth $2$ never moved to an otherwise-equal parent at depth $1$ — the levels with the most fan-out leverage were the ones the migration loop could not lift. Halving the depth gain for the shallower direction means one hop always wins on its own, while the full 30 ms is still required to move *back* to the deeper parent, so RTT jitter cannot flap a node between two depths:
    * Node $i$ connects to $P_{\text{new}}$.
    * Once $P_{\text{new}}$ begins delivering chunks, node $i$ sends a `DISCONNECT_CHOKE` frame to $P_{\text{current}}$.
    * Node $i$ has successfully migrated upward to a faster/shallower branch without dropping a single video frame. The margin prevents nodes from oscillating back and forth rapidly between two similar parents; the depth-propagation rule above means the score is evaluated on current depths, so a migration also lifts the whole subtree beneath $i$ within about a second.
