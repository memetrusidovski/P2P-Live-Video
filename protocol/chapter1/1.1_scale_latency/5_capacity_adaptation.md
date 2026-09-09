# 5. Capacity Adaptation and Degraded Operation

The Swarm Sustainability Condition (§1) states when the swarm can carry itself:
$$\sum_{i=1}^{N} (1 - r_{\text{pull}})\, u_i \ \ge\ \Omega \cdot N \cdot B$$

where $\Omega \approx 1.15$ is the per-slice overhead factor (FEC parity plus framing) and $r_{\text{pull}} = 0.10$ the PULL-service reserve, both defined with the slot count in §1.2.1. That inequality is an *assumption* in the depth and latency proofs of §2–§3, not a guarantee the network provides. This document defines what the protocol does when the assumption fails — which, given real residential upload distributions, is the common case rather than the exception.

## 5.1 The Two Thresholds

Let $\bar{u} = \frac{1}{N}\sum u_i$ be the mean upload capacity of the swarm and define the **capacity ratio**:
$$\sigma = \frac{\bar{u}}{B}$$

Two distinct thresholds matter, and the spec has historically conflated them. Both include the overhead that every pushed slice actually costs:

| Condition | Meaning |
| :--- | :--- |
| $\bar{u} \ge \sigma_{\text{target}} B \approx 10.2\text{ Mbps}$ ($\sigma \ge 1.70$) | Mean fan-out $k = 8$ at the nominal $1$ Mbps slice is achievable ($8 \cdot 1\text{ Mbps} \cdot \Omega / (1 - r_{\text{pull}})$), so the $D \le 7$ depth proof of §3 holds **and** full quality is sustainable. This is the design target. |
| $\sigma_{\text{full}} B \approx 7.7 \le \bar{u} < 10.2\text{ Mbps}$ | Full quality is sustainable ($\sigma_{\text{full}} = \Omega / (1 - r_{\text{pull}}) = 1.28$), but mean fan-out drops below 8 and trees deepen toward $D_{\text{max}} = 8$. Still inside the playout budget, with less headroom. |
| $\bar{u} < 7.7\text{ Mbps}$ ($\sigma < 1.28$) | The swarm **cannot** carry full bitrate. Layer shedding (§5.3) applies. |

An overhead-blind reading of the same table ($6$ and $8$ Mbps) was $15$–$25\%$ optimistic; the earlier draft carried that error.

Sustainability is also a **per-tree** property, not only a global one. Tree $T_m$ carries its declared bitrate $B_m$ (§1.2.4) to every subscriber and draws its relays from the $\approx N_{\text{relay}}/M$ nodes assigned to it, each holding $K_v(m)$ slots. Tree $m$ is sustainable iff its relays' slots cover the swarm:
$$\frac{N_{\text{relay}}}{M} \cdot \frac{(1 - r_{\text{pull}})\, \bar{u}_{\text{relay}(m)}}{B_m\, \Omega} \ \ge\ N
\quad\Longleftrightarrow\quad
\bar{u}_{\text{relay}(m)} \ \ge\ \frac{1}{1 - \ell} \cdot \frac{M \cdot B_m \cdot \Omega}{1 - r_{\text{pull}}}$$

where $\ell = 1 - N_{\text{relay}}/N$ is the leaf fraction (§1.2.5). With no leaves and the nominal $B_m = B/M$ this collapses to the global condition; with real per-tree bitrates it does not. On the $M = 6$ reference mapping the $0.75$ Mbps $L_0$/$L_1$ stripes need $\bar{u}_{\text{relay}} \ge 5.75$ Mbps and the $1.5$ Mbps $L_2$ stripes need $11.5$ Mbps — the top layer's trees are the first to starve, which is the order the shed rule drops them in. That coincidence is deliberate: it is what the minimax allocation rule of §1.2.4 exists to produce. A single global $\sigma$ can look healthy while one tree starves, which is why adaptation is driven by per-tree signals, not by a gossiped global estimate.

The per-tree form also says something the global form hides: **a relay's upload is available only to the layer its assigned tree carries.** The $N_{\text{relay}}/M$ relays of a $1.5$ Mbps $L_2$ stripe cannot lend a slot to a starving $L_0$ stripe, however idle they are. Any capacity statement that sums relay upload across layers is therefore an upper bound the mapping may not let the swarm reach — see §5.2 and §5.6.

## 5.2 Sustainable Layer Set

With SVC slicing (§1.2.4), the question is which layers the *relay population* can carry, and the answer is the per-tree condition of §5.1 applied to each layer's trees. Layer $l$, carried by $t_l$ trees of bitrate $b_l / t_l$, is sustainable iff

$$\bar{u}_{\text{relay}} \ \ge\ U_l(M) \;=\; \frac{1}{1 - \ell} \cdot \frac{M \cdot (b_l / t_l) \cdot \Omega}{1 - r_{\text{pull}}}$$

An earlier draft derived the layer table from the global condition $\sum_{l \le j} b_l \le (1 - r_{\text{pull}})\bar{u} / \Omega$, which treats every relay's upload as available to every layer. It is not (§5.1): the global form is exact only when every tree in the forest carries the same bitrate — on the reference ladder, at $M = 4$ and after the top layer has been folded (§5.6) — and everywhere else it overstates what the top layer can get by the ratio of the largest per-tree bitrate to the nominal $B/M$, which is $1.5\times$ at $M = 3$ and $M = 6$. The earlier table's "1080p at $7.7$ Mbps" was that overstatement; at $M = 6$ the true figure is $11.5$ Mbps.

For the reference ladder (480p base $= 1.5$ Mbps, 720p enhancement $= 1.5$ Mbps, 1080p enhancement $= 3.0$ Mbps; $B = 6$ Mbps), $\Omega = 1.15$, $\ell = 0$, evaluated at every rung with the mapping of §1.2.4 §4.2.1 before and after each fold:

| $M$ | $\bar{u}_{\text{relay}}$ for 1080p (three layers) | for 720p (after folding $L_2$) | for 480p (after folding $L_1$) |
| :---: | :---: | :---: | :---: |
| 2 | $7.67$ ($3.0,\,3.0$) | $3.83$ ($1.5,\,1.5$) | $1.92$ ($0.75 \times 2$) |
| 3 | $11.5$ ($L_2$ at $3.0$) | $5.75$ ($0.75,\,0.75,\,1.5$) | $1.92$ ($0.5 \times 3$) |
| 4 | $7.67$ ($1.5 \times 4$) | $3.83$ ($0.75 \times 4$) | $1.92$ ($0.375 \times 4$) |
| 5 | $9.58$ ($L_2$ at $1.5$) | $4.79$ ($0.5 \times 3,\ 0.75 \times 2$) | $1.92$ ($0.3 \times 5$) |
| 6 | $11.5$ ($L_2$ at $1.5$) | $3.83$ ($0.5 \times 6$) | $1.92$ ($0.25 \times 6$) |

Each entry is $M \cdot B_{\max} \cdot \Omega / (1 - r_{\text{pull}})$ for the binding (largest) per-tree bitrate of the layer set. Three things follow:

*   **The base layer's threshold is the same at every rung** ($1.92$ Mbps) *once the layers above it are folded*, because a single layer striped over all $M$ trees has $B_m = b_0 / M$ and the per-tree condition collapses to the global one. Below that the source top-up of §5.4 applies.
*   **Between folds, lower layers can be starved while upper-layer relays sit on stranded slots.** At $M = 6$ and $\bar{u}_{\text{relay}} = 5$ Mbps, the $0.75$ Mbps $L_0$ stripes need $5.75$ and starve, while the two $L_2$ trees hold a third of the relays. Peers shedding $L_2$ frees nothing for $L_0$; only the source can re-map (§5.6).
*   **With leaves, divide the relay population's target by $(1 - \ell)$**: at $\ell = 0.5$ every threshold doubles.

On a lossy swarm at the $30\%$ parity ceiling ($\Omega = 1.42$) every threshold rises by a further $23\%$. Degradation is *resolution loss*, never playback freeze — the property SVC slicing exists to provide — **provided the mapping tracks the capacity**, which is what §5.6 makes the source's job.

## 5.3 Saturation Detection and Layer Shedding

No global measurement is required. Capacity shortage manifests locally, as failure to acquire a parent in a specific tree:

1.  **Signals.** The tree-join algorithm (§1.2.2) returns `FAILURE_RETRY_BACKOFF` when every candidate refuses the join with `REJECTED_SATURATED` or `REJECTED_DEPTH` (Appendix D §D.4.3b), or when every candidate advertises hop depth $h \ge D_{\text{max}}$. Parents **must** reject any join that would place a child deeper than $D_{\text{max}} = 8$; a saturated forest may not silently grow deeper. A candidate that **times out** is unreachable, not saturated, and a round in which every candidate timed out is a discovery failure (re-query, Ch2 §2.3.2), not a shed signal; a `REJECTED_NOT_ASSIGNED` corrects a stale record and is neither.
2.  **Warm-up rounds do not count.** A candidate whose `PROBE_RESPONSE` reports `TreeState = WARMING` for the probed tree (Appendix D §D.4.7) is *warming up*, not saturated (§1.2.2): it has a parent in that tree but has not yet verified its first segment there, and will have capacity within one segment period. A retry round in which at least one candidate was warming is **not** a failed round for the purposes of the shed rule below. Without this exclusion a growth burst — many relays warming at once, which is exactly what a flash crowd or a forest resize produces — is indistinguishable from a capacity shortage, and whole cohorts of peers would shed a quality layer for a condition that resolves in under a second. For the same reason the retry backoff interval **must** be at least one segment period ($1.0\text{ s}$), so that no shed decision is ever taken on a sample shorter than a single warm-up window. A candidate reporting `SERVING` with $K_{\text{avail}} = 0$, or `UNPARENTED` (assigned to the tree but currently without a parent in it), **does** count as a failed candidate: both mean the tree cannot take this peer now, and an unparented relay is itself a victim of the same shortage.
3.  **Shed rule — by layer.** After **2 consecutive failed retry rounds** in any tree $T_m$ carrying layer $l$, the peer sheds **layer $l$ and every layer above it**: it stops attempting to join every tree that carries those layers *and that it is not assigned to relay* (below), removes them from its subscribed set $\mathcal{T}_{\text{sub}}$ (§1.3), marks the layers unavailable, and instructs its decoder to render at layer $l - 1$. Shedding is by layer because an SVC layer is undecodable without the layers beneath it, and a striped layer is undecodable without all of its stripes (§1.2.4). The manifest `priority` values encode the order — all trees of one layer share a priority, and lower priority is shed first. **The trees carrying the base layer $L_0$ are never shed.**
4.  **A relay never abandons its assigned trees.** A `RELAY`-class node's join set is $\mathcal{T}_{\text{join}} = \mathcal{T}_{\text{sub}} \cup \mathcal{T}_{\text{assigned}}$ (§1.3): it keeps a parent in every tree it is assigned to relay, whether or not it renders the layer that tree carries, and **forwards what it does not render**. Shedding changes what the node decodes and which *non-assigned* trees it joins; it never removes an assigned tree from the join set. Without this rule, a relay assigned by rendezvous to a top-layer tree that shed that layer would hold no parent there, forward nothing, and answer every probe for that tree with $K_{\text{avail}} = 0$ — manufacturing saturation evidence for every joiner that probed it, and stranding exactly the slots the tree was short of. An assigned tree the relay cannot join is retried at the upward-migration cadence (§1.2.2, every $5$ s) without the 10 s hysteresis below; while unparented it reports `TreeState = UNPARENTED`. The extra download is one stripe per assigned-but-unrendered tree, which is downstream capacity the node has in abundance.
5.  **Hysteresis.** A shed layer is not retried for $10$ seconds, after which the existing 5-second upward-migration loop (§1.2.2) attempts re-join of its trees naturally; the layer is restored only when *all* of its trees have parents again. This prevents whole cohorts of peers from oscillating between quality levels in lockstep.
6.  **Grace after a matrix change.** A `MANIFEST_UPDATE` (§1.2.4 §4.5) changes which layer every affected tree carries and, for new trees, starts from an empty forest. For every tree whose entry in the slicing matrix changed, failed rounds do not count toward the shed threshold until `EffectiveSegmentSeq` $+ \tau_{\text{grace}}$, with $\tau_{\text{grace}} = D_{\text{max}} = 8$ segments — one warm-up window per level of the deepest permitted tree. A peer without a parent in such a tree renders without that layer meanwhile and repairs any blocks it can from the PULL zone (Ch4 §4.3.1); it does not shed. Without the grace, a swarm-wide resize converts the fill-in of a new tree into a swarm-wide 10 s shed of the top layer (SOLUTION-043).

$D_{\text{max}}$ overflow and capacity saturation are thus the *same condition* with the same response: shed a layer rather than deepen the tree past its latency budget.

## 5.4 Source-Side Base-Layer Reserve

The base layer is the protocol's floor and is protected by the broadcaster directly. The source reserves at least $R_{\text{src}} = 3 \cdot b_0 \cdot \Omega$ of its own upload capacity as a **base-layer emergency pool** — three full copies of $L_0$, however it is currently striped — held back from ordinary child allocation. Its ordinary slots are those of a relay assigned to every tree with that reserve removed:

$$K_S(m) = \left\lfloor \frac{u_S - R_{\text{src}}}{M \cdot B_m \cdot \Omega} \right\rfloor$$

(the source serves no `PULL_REQUEST`s from the live edge, so no $r_{\text{pull}}$ share is withheld). The pool serves base-layer slots directly when it observes starvation signals: `GET_PEERS` re-queries whose `StarvedTrees` bitmap (Appendix D §D.4.6b) marks an $L_0$ tree, counted per tree by the DHT guardians and returned to the publisher on every Stream Record store (Ch2 §2.3.2), from which the publisher forms the starved-fraction estimate $\hat{s}_m$ of §5.6. The reserve therefore has a concrete, counted trigger rather than an inferred one, and it is the *last* response: §5.6 re-maps the forest first, because a fold moves relay capacity to the base layer at no cost to the source, and the reserve pays only for what no mapping can carry.

This bounds the worst case. A swarm with $\sigma \ll 1$ degrades to "480p, partially source-fed" rather than collapsing — and the source's obligation stays a small constant ($3 \cdot 1.5 \cdot 1.15 \approx 5.2$ Mbps at the reference ladder), never the $O(N)$ CDN cost the protocol exists to avoid. A source that is itself behind NAT delegates this pool to its ingress relays (Ch6 §6.3.3).

## 5.5 Interaction with Incentives

Under saturation, the two layer classes are allocated differently:

*   **Enhancement-layer child slots** are allocated by contribution rank (Chapter 5): a saturated enhancement-tree parent may preempt its lowest-ranked child for a better-ranked joiner. Scarcity makes contribution the deciding factor for quality, exactly as the protocol's core axiom requires.
*   **Base-layer slots** form a **universal service floor**: every verified peer is entitled to $L_0$ regardless of rank. Base-layer slots are never rank-preempted. A bounded share of each relay's $L_0$-tree slots, the **leaf share** $R_{\text{leaf}}(m) = \lceil 0.2 \cdot K_v(m) \rceil$, is enforced by displacement rather than held idle: while leaf-class children hold fewer than $R_{\text{leaf}}(m)$ of the relay's slots in an $L_0$ tree and none is free, a leaf-class request displaces the relay's lowest-ranked relay-class child *that does not itself relay that tree* (Ch1 §1.2.2 *Rank Admission Rule*, item 3). At the margin the source reserve of §5.4 serves the rest.

This is what keeps leaf-class devices (§1.2.5) from being free-riders: they receive the floor by right and pay for it in quality ceiling, not in stalled playback.

## 5.6 Publisher-Side Layer Fold

Shedding is a per-peer response and **cannot move capacity**. A relay's slots are in its assigned tree, and its assigned tree carries whatever layer the source put there. When the relay population cannot carry the base layer under the current mapping, peers shedding the top layer free nothing for $L_0$: the relays assigned to the top layer's trees keep the top layer's stripes while $L_0$ starves. Worked at $M = 6$, $\bar{u}_{\text{relay}} = 4$ Mbps, $\ell = 0$:

| Mapping | $L_0$ stripe | $K_v$ per $L_0$ relay | $L_0$ stripe-slots supplied | needed | Result |
| :--- | :---: | :---: | :---: | :---: | :--- |
| $(2,2,2)$ — three layers | $0.75$ Mbps | $4$ | $2 \cdot \frac{N}{6} \cdot 4 = 1.33\,N$ | $2N$ | one peer in three has **no base layer** |
| $(3,3)$ — $L_2$ folded | $0.5$ Mbps | $6$ | $3 \cdot \frac{N}{6} \cdot 6 = 3\,N$ | $3N$ | exactly sustainable |

The relays are sufficient; the mapping strands a third of them on a layer nobody can afford. Only the source can change the mapping, and it already receives the signal: `StarvedCount[m]` per tree on every `STORE_RECORD_ACK` (Ch2 §2.3.2).

**Estimate.** A `GET_PEERS` reaches $\approx \alpha$ of the $k = 20$ guardians, so each guardian sees a fraction $\approx \alpha / k$ of the peers that re-queried with a starved bit in its 10 s window. The publisher's estimate of the fraction of the swarm starved of tree $m$ is

$$\hat{s}_m = \frac{k}{\alpha} \cdot \frac{\text{median}_g\, \text{StarvedCount}[m]}{\hat{N}}, \qquad \hat{N} = \text{median}_g\, \text{ActiveCount} \cdot 2^{s}$$

`StarvedCount` is **not** scaled by $2^{s}$ — every peer queries, sampled or not (Ch2 §2.3.3). The estimate is coarse and is used as a trigger, never as a measurement.

**Fold rule.** Let $l_{\text{top}}$ be the highest layer in the current mapping. If for some layer $l < l_{\text{top}}$ the mean of $\hat{s}_m$ over the last $\tau_{\text{fold}}$ of acknowledgements exceeds $s_{\text{fold}} = 0.05$ for any tree $m$ carrying $l$, the source emits a `MANIFEST_UPDATE` (§1.2.4 §4.5) with the **same $M$**, the layer list $L_0 \ldots L_{\text{top}-1}$, and the mapping re-run by §4.2.1 over that list. $\tau_{\text{fold}} = 10$ s when the starved layer is $L_0$ — a starved base layer is a freeze, and the minimax allocation makes it the layer that starves first at the $M = 4$ rung for ordinary residential uploads (§1.2.4 §4.2.1) — and $30$ s otherwise. One layer is folded per update; a further fold needs its own $\tau_{\text{fold}}$. Starvation confined to the **top** layer never triggers a fold: it is resolution loss for the starved peers, and folding would take the top layer from everyone to give it back to no one — never better in aggregate. Starvation of a lower layer is a *freeze* for the starved peers, and folding the top layer hands its relays to the layers below.

**A fold is a resize in which no relay moves.** $M$ is unchanged, so every rendezvous assignment is unchanged; every tree's content changes at `EffectiveSegmentSeq`. Because a layer removed from the greedy allocation of §4.2.1 only hands its trees to the layers that remain, **no remaining tree's bitrate rises at a fold**, so no relay's slot count falls and no child is drained by it; slot counts rise, and relays admit up to the new $K_v(m)$ from the switch. Peers that had shed the folded layer hold no parent in the trees that carried it and must join them — those trees now carry lower-layer stripes they need. Their relays are warm (they were forwarding the folded layer), so the join is one round; blocks missed during it are repaired from the PULL zone (Ch4 §4.3.1), and the grace of §5.3 item 6 applies.

**Unfold.** The publisher has no direct signal that capacity has returned, and it must not go looking for one: a fold happens only because a *lower* layer was starving, so an unfold that turns out to be wrong re-creates that starvation — a base-layer freeze for the starved fraction — for a full $\tau_{\text{fold}}$ before the fold fires again. A timer-driven probe would do that to a stable swarm every period forever. The source therefore restores a folded layer **only when the relay population has grown by $\ge 50\%$ since the fold** ($N_{\text{relay}}$ from the guardians' counts), which adds capacity in the only way the publisher can see, and never on a timer. A swarm whose relays' upload improved without their number changing stays folded; that is accepted, and Chapter 8 owes the measurement of how often it happens. The forest dwell $\tau_{\text{forest}} = 30$ s (§1.2.1 §1.4) applies between any two matrix changes, fold, unfold or resize alike, and a fold never pre-empts it. The source may stop encoding a folded layer. Under MDC (§1.2.4 §4.3) the same rule folds the highest-numbered description.

The base-layer reserve of §5.4 is what remains when even a single-layer mapping is unsustainable: at $\bar{u}_{\text{relay}} < 1.92 / (1 - \ell)$ Mbps on the reference ladder.
