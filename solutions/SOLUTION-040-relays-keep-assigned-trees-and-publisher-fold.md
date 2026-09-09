# SOLUTION-040: Relays Never Abandon Their Assigned Trees; the Publisher Folds Layers

**Closes:** ISSUE-040 (High)
**Lives in:** `protocol/chapter1/1.1_scale_latency/5_capacity_adaptation.md` (§5.1 addendum, §5.2 rewritten, §5.3 items 4 and 6, §5.4 $K_S$, new §5.6); `protocol/chapter1/1.3_peer_lifecycle/1_transition_model.md` ($\mathcal{T}_{\text{join}}$), `2_algorithmic_core_loop.md`; `protocol/chapter1/1.2_multi_forest_overlays/4_stream_slicing_architecture.md` (§4.2.1 fold note); `protocol/chapter2/2.3_stream_registration/2_store_get_rpcs.md`, `3_registration_frames.md` (`QueryCount`, unscaled `StarvedCount`); `appendix_b_parameters.md`
**Class:** Two independently-scaled decisions (per-peer shedding, source-side mapping) with nothing connecting them (recurring patterns #1 and #2)

---

## The problem in one line

A relay's tree was fixed by rendezvous over all $M$ trees while its subscription shrank by shedding; a relay assigned to a tree whose layer it had shed held no parent there, forwarded nothing and answered every probe "saturated" — and the §5.2 layer table was derived from total relay upload as if a relay in an $L_2$ tree could lend a slot to $L_0$, which the mapping never lets it do.

## The decision

*   **A `RELAY` never sheds an assigned tree.** Its join set is $\mathcal{T}_{\text{join}} = \mathcal{T}_{\text{sub}} \cup \mathcal{T}_{\text{assigned}}$; it holds a parent in every assigned tree and forwards what it does not render. Shedding changes what the node decodes and which *non-assigned* trees it joins. `ACTIVE` is still gated on $\mathcal{T}_{\text{sub}}$ alone. An assigned tree the relay cannot join is retried at the 5 s migration cadence, without the 10 s hysteresis, and reported as `TreeState = UNPARENTED` (SOLUTION-044).
*   **The §5.2 table is derived from the §5.1 per-tree condition**, rung by rung: layer $l$ is sustainable iff $\bar{u}_{\text{relay}} \ge M (b_l/t_l) \Omega / ((1-\ell)(1 - r_{\text{pull}}))$. On the reference ladder 1080p needs $7.67$–$11.5$ Mbps relays depending on rung (not $7.7$), 720p $3.83$–$5.75$ after the top layer is folded, 480p $1.92$ at every rung.
*   **The publisher folds the top layer when a lower layer is starved.** From the guardians' `StarvedCount[m]` (unscaled — queries are not sampled) and the new `QueryCount`, the publisher forms $\hat{s}_m = (k/\alpha) \cdot \text{StarvedCount}[m] / \hat{N}$ and, when the 30 s mean exceeds $5\%$ for any tree of a layer *below* the top, emits a `MANIFEST_UPDATE` with the same $M$ and the allocation rule re-run over the remaining layers. A fold moves no relay (rendezvous is over $M$, unchanged), raises no tree's bitrate, and so drains no child. Unfolding was first specified as a probe (on $\ge 50\%$ relay growth or a doubling timer); the timer was removed while closing ISSUE-058, since a wrong unfold re-creates the lower-layer freeze that caused the fold — see SOLUTION-058.
*   **The source's slot count is defined**: $K_S(m) = \lfloor (u_S - 3 b_0 \Omega) / (M B_m \Omega) \rfloor$; it had never been stated and SOLUTION-043 needs it.

## Why this and not the alternatives

*   **Restrict rendezvous ranking to $\mathcal{T}_{\text{sub}}$** (the issue's other option): a relay that shed $L_2$ would re-rank over the $L_0$/$L_1$ trees and move — but so would every relay that shed, in lockstep, at the moment of shedding, with all the warm-up and re-join cost of a resize and none of the source's coordination; and the bitmap would no longer be verifiable from NodeID and $M$ (SOLUTION-028 relies on that). Keeping the relay in its assigned tree costs one downloaded stripe and nothing else.
*   **Fold whenever any layer is starved, including the top one.** Folding $L_2$ when only $L_2$ is short takes 1080p from the $67\%$ who have it to give 720p to the $33\%$ who already had 720p — strictly worse in aggregate. Top-layer starvation *is* the SVC design working; lower-layer starvation is a freeze for the starved and the only case a fold improves.
*   **A fold threshold on absolute `StarvedCount`.** Each guardian sees $\approx \alpha/k$ of the queries and the publisher does not know $N$ exactly; a fraction of the swarm-size estimate is the only scale-free trigger available without a new frame.
*   **Let peers derive the folded mapping themselves** when they observe starvation: violates the one rule SOLUTION-019 made load-bearing — no peer derives the mapping — and two peers observing different starvation would disagree about what tree 5 carries.

## Defects found during verification

*   Per-tree supply at $M = 6$, $\ell = 0$, $\Omega = 1.15$: $\bar{u} = 8$ Mbps gives $K_v = 8$ per $0.75$ stripe ($2.67N$ supplied vs $2N$) and $4$ per $1.5$ stripe ($1.33N$ vs $2N$) — a third of peers lose $L_2$ where the old table promised 1080p. At $4$ Mbps, $K_v = 4$ on $L_0$ stripes: $1.33N$ vs $2N$, **a third of the swarm has no base layer** while the old table promised 720p. Re-mapped to $(3,3)$: $K_v = 6$ on $0.5$ stripes, $3N$ vs $3N$, exactly sustainable.
*   The old §5.2 formula happens to be right after every fold (a single layer striped over $M$ trees has $B_m = b_0/M$), which is why its 720p and 480p thresholds ($3.8$, $1.9$) survive almost unchanged while its 1080p threshold was wrong by $1.5\times$ at $M = 3$ and $6$.
*   `StarvedCount` was scaled by $2^{s}$ with the registration counts; at $N = 10^6$ the publisher read starvation $128\times$ too high (ISSUE-054 item 1, closed here).
*   Folding is monotone: on every rung of the reference ladder, removing the top layer from the greedy allocation only hands its trees to the remaining layers — $(2,2,2) \to (3,3) \to (6)$, $(2,1,2) \to (3,2) \to (5)$, $(1,1,1) \to (2,1) \to (3)$ — so no relay's $K_v(m)$ falls at a fold. This is what lets the fold use the resize choreography with no drains.

## The generalisable lesson

**A per-peer adaptation cannot move capacity that a global assignment has pinned. When the two are designed in different chapters, check that the adaptation's promise ("resolution loss, never freeze") is actually reachable under the assignment, and give the owner of the assignment a rule that fires on the adaptation's failure signal.**

## Residual risk

*   $\hat{s}_m$ is coarse: it assumes each guardian sees $\alpha/k$ of queries and that starved peers re-query at the retry cadence. A mis-estimate by $2\times$ moves the effective threshold between $2.5\%$ and $10\%$, which is tolerable for a trigger; Chapter 8 should calibrate it.
*   The unfold policy is a probe with no capacity signal behind it; an oscillating swarm folds and unfolds at the doubling interval. Bounded, and visible as `MANIFEST_UPDATE` cadence.
*   A relay forwarding an assigned tree it does not render still pays that tree's download; for a $3.0$ Mbps tree at $M = 2$ on a metered link that is real, and is the cost of being relay-class.

## Validation owed (Chapter 8)

*   Base-layer PSR with and without the fold rule at $\bar{u}_{\text{relay}} \in \{3, 4, 5, 6\}$ Mbps, $M = 6$.
*   $\hat{s}_m$ against the true starved fraction under Scenario A churn.
*   Fold/unfold cadence over a two-hour stream with an upload distribution drawn from a real residential mix.
