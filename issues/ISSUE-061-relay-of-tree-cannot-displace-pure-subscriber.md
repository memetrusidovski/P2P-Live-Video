# ISSUE-061: A relay assigned to tree m has no admission priority in m, so a saturated parent full of pure subscribers stops the tree from growing

**Status:** Open
**Priority:** High
**Component:** Ch1 §1.2.2 Rank Admission Rule; Ch1 §1.2.1 §1.3 Orthogonal Placement; Ch1 §1.1.5 §5.4 source reserve
**Affects:** Cold start and every growth phase, $N = 2$ through flash crowds; worst where the source's per-tree slot count is small (residential broadcasters)
**File:** `protocol/chapter1/1.2_multi_forest_overlays/2_parent_selection_algorithm.md`, `protocol/chapter1/1.2_multi_forest_overlays/1_graph_theory_and_slicing.md`, `protocol/chapter1/1.1_scale_latency/5_capacity_adaptation.md`

---

## Summary

Every peer subscribes to every tree of its layer prefix, but only the relays *assigned* to tree $T_m$ ever offer slots in $T_m$. A relay assigned to $T_m$ therefore has to obtain a parent in $T_m$ before it can serve anyone there, and the Rank Admission Rule gives it no way to do so when the candidate parents' $T_m$ slots are already held by peers that merely subscribe to $T_m$. Case 2 (rank preemption) needs $r_{\text{new}} > 1.25\,r_{\min}$, and a fresh relay ranks $0$; case 3 (leaf share displacement) applies only to leaf-class requesters in $L_0$ trees; cases 4–5 reject. The result is a deadlock the specification's own placement rule creates: the tree's future interior nodes are the ones that cannot get in. Found by the M1 simulator on the first cold-start run.

## Detailed Description

The worked case, from `scenarios/small_swarm.toml` in `src/`: a 20 Mbps broadcaster, $M = 3$ on the reference ladder ($1.5 / 1.5 / 3.0$ Mbps), ten viewers arriving over 15 s, five of them relay-class at 10 Mbps. By Ch1 §1.2.1 §1.3 the source's slots are $K_{\text{src}}(m) = \lfloor 0.9 \cdot 20000 / (3 \cdot B_m \cdot 1.094) \rfloor$: **3, 3, 1**. Rendezvous assignment puts one relay on $T_2$ (call it $R_2$) and two on $T_3$.

1.  Viewers $V_1, V_4, V_5$ join first and take all three of the source's $T_2$ slots as *pure subscribers* of $T_2$ (none is assigned to it).
2.  $R_2$ arrives, probes for $T_2$: the source answers `K_avail = 0`, `SERVING`; no other peer relays $T_2$, so the candidate list is empty and the round fails. Two failed rounds later the shed rule fires — but $T_2$ is $R_2$'s *assigned* tree, which Ch1 §1.3.1 says "is always joined, subscribed or not", so $R_2$ retries at the migration cadence forever.
3.  Every later viewer that wants $L_1$ finds exactly one relay of $T_2$ in the swarm, `UNPARENTED`, and one saturated source. $T_2$ has three leaves and will never have an interior node. The same happens in $T_3$ with a single source slot.

The rule texts, quoted, that produce this:

*   Ch1 §1.2.2 Rank Admission, case 2: "No free slot, $T_m$ carries an enhancement layer, and $r_{\text{new}} > 1.25 \cdot r_{\min}$ … `ACCEPTED`, and the parent preempts". $r_{\text{new}} = 0$ for a relay that has never served, so it never preempts.
*   Case 3: displacement is scoped to "the requester is leaf-class" and "$T_m$ carries $L_0$".
*   "Bootstrapping is through the base layer: a new relay ranks 0, obtains an $L_0$ slot wherever one is free … relays $L_0$ to children, earns receipts, and within a minute has the rank to preempt into enhancement trees." This assumes the new relay is *assigned* to an $L_0$ tree. A relay assigned to $T_2$ or $T_3$ has no $L_0$ children to earn rank from, because it has no slots in $L_0$ trees at all (§1.3: interior only in its assigned trees).
*   Ch1 §1.1.5 §5.4 gives the source a base-layer reserve keyed on `StarvedTrees` counts, which addresses $L_0$ starvation of *viewers*, not the unparented relay of an enhancement tree.

Scale sweep. The deadlock needs only that a parent's slots in $T_m$ fill with non-relays of $T_m$ before the relays of $T_m$ arrive. With uniform arrival the probability that the first $K(m)$ arrivals in $T_m$ include no relay of $T_m$ is $(1 - 1/M)^{K(m)}$ for a relay-only swarm ($\approx 0.30$ at $K = 3$, $M = 3$) and higher with leaves in the mix. It is not a corner case; it is the default outcome for a residential broadcaster, and it recurs at every relay in the interior whenever a tree is saturated: growth of the tree below any saturated relay is blocked until a relay of that tree happens to hold rank.

The specification's own remedy list is inverted here. Rank preemption exists so that *contributors* displace *free-riders*; but a node assigned to $T_m$ is the only kind of node whose admission to $T_m$ creates capacity for others, and it is the one the rule cannot admit.

## Impact

*   **Cold start does not converge** below the source's per-tree slot count times $M$ viewers, for any broadcaster whose upload gives fewer than roughly $2 M$ slots per tree.
*   **Enhancement trees never grow interior nodes** when their first slots are taken by subscribers; the swarm degrades to "everyone at $L_0$ except the first few", which is a stable state the shed rule then locks in.
*   **Flash crowds** hit the same wall at every saturated relay, not only at the source.

## Proposed Fix

Add a case to the Rank Admission Rule between cases 1 and 2, mirroring the leaf-share displacement of case 3 but keyed on relay duty rather than class:

> **1b. No free slot, and the requester's `AssignedTrees` (from `NEIGHBOR`, App D §D.4.3) has bit $m-1$ set, and the parent has at least one child in $T_m$ that does *not* relay $T_m$ (bit $m-1$ of its `AssignedTrees` clear — a pure subscriber with no children in $T_m$, so nobody is orphaned):** `ACCEPTED`, and the parent displaces its lowest-ranked such child through the drain path (reason `DISPLACED`). At most one displacement in progress per tree, as for case 2. If every child in $T_m$ relays $T_m$: fall through.

Justification the fixer can check: the displaced subscriber loses nothing but one re-attach, and it re-attaches under the *new* relay within one round, because the new relay's slots in $T_m$ open as soon as its first segment verifies (warm-up gating, §2.1). Capacity in $T_m$ goes from $K_p(m)$ to $K_p(m) + K_{\text{new}}(m) - 1$. The displaced peer's own contribution is unaffected: it relays some other tree, and that tree's edges are untouched (edge-disjointness holds).

Two guards: (i) the requester must actually be assigned to $m$ by the rendezvous rule the parent can recompute from the NodeID (Ch1 §1.3; the `AssignedTrees` bit alone is self-declared), so a peer cannot claim every tree to jump every queue; (ii) a displaced child is not displaced again for $\tau_{\text{drain}}$.

Also: Ch1 §1.3.1 should say that an assigned tree is retried at the migration cadence with **bounded** backoff, and that shedding is decided on the *subscribed* set only — the implementation found the two conflated ("do not shed an assigned tree" prevented a relay from ever reaching `ACTIVE` at $L_0$).

## Addendum: the sequential handover deadlocks the displacement

Implemented as proposed, the simulator (`scenarios/lossy_links.toml`, `baseline_100.toml`) shows a second-order effect the fixer should write into the rule. Ch1 §1.2.2 *Handover budget* makes the handover **sequential** (`ACCEPTED.PENDING`) whenever the parent's uplink is below $10 \cdot B_m \cdot \Omega$ — $16.4$ Mbps in a $1.5$ Mbps tree, $32.8$ in a $3.0$ Mbps tree — which is every residential relay in every tree of the reference ladder. Then:

1.  the displacer is admitted but not served until the displaced child releases the slot;
2.  the displaced child looks for another parent, but in a saturated tree the only slot that will ever open is under the displacer, which cannot warm up (Ch1 §1.2.2 warm-up gating) until it is served;
3.  both wait the full $\tau_{\text{drain}} = 5$ segments; the parent then cuts the child with `DISCONNECT(DISPLACED)`, the displacer warms for $\approx 1$–$2$ segments, and the child re-attaches under it. Net: the displacer is idle for five segments and the child takes a cold repair of $\approx 1.5$ s at the end of them, instead of a warm handover.

Proposed: when the handover is sequential, the drain deadline is `current segment + 1`, not $+\tau_{\text{drain}}$. The child still gets one segment to find an alternative; if none exists the displacer starts a segment later rather than five, and the child's gap shrinks from "five segments plus a repair" to "a repair". The full $\tau_{\text{drain}}$ is right only for the overlapping handover, where the child loses nothing by waiting.

## Effort

Small in text: one admission case, two sentences of guards, one clarification in §1.3.1. It reuses `DRAIN_NOTICE`, `DISPLACED`, and the pure-subscriber definition already in case 3.
