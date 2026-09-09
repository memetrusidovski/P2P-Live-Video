# SOLUTION-045: Two Upload Budgets, Each Owned by One Expression

**Closes:** ISSUE-045 (Medium)
**Lives in:** `protocol/chapter1/1.2_multi_forest_overlays/1_graph_theory_and_slicing.md` (*The Two Budgets, Each Owned by One Expression*); `2_parent_selection_algorithm.md` (*Handover budget*, `ACCEPTED.PENDING`); `protocol/chapter5/5.1_tit_for_tat/2_sliding_window_unchoker.md`, `3_optimistic_exploration.md` (`PullSlots`); `appendix_d_frame_registry.md` §D.4.4; `appendix_b_parameters.md`
**Class:** Constants that must satisfy a joint relation, set independently in three chapters (recurring pattern #1)

---

## The problem in one line

The PULL reserve $r_{\text{pull}} u_v$ was spent three times — all of it as Tit-for-Tat slots, a preempted slot's overlap, and every emergent-relay bridge — with no expression owning the total, and on the reference 10 Mbps relay it could not hold even one preempted $1.5$ Mbps slot.

## The decision

*   **Tree budget** $(1 - r_{\text{pull}})u_v \ge \sum_m \text{children}_v(m) B_m \Omega_v + U_{\text{bridge}}$, with $U_{\text{bridge}} \le \frac{1}{2}(1 - r_{\text{pull}})u_v$ and $K_v(m)$ computed with $U_{\text{bridge}}$ removed.
*   **PULL reserve** $r_{\text{pull}} u_v \ge \beta_{\text{pull}} \cdot \text{PullSlots} + \sum_{\text{handovers}} B_m \Omega_v$; `PullSlots` is recomputed every $\tau_{\text{tft}}$ from the remainder.
*   **Sequential handover** when the reserve cannot hold $B_m \Omega_v$ even with every PULL slot choked (any relay below $10 B_m \Omega$): `ACCEPTED` with `AcceptFlags.PENDING`, pushing begins when the drained child releases. Bridges move to the tree budget (SOLUTION-050).

## Why this and not the alternatives

*   **Never preempt when the reserve lacks headroom** makes rank admission inert on every $10$ Mbps relay in every $1.5$ Mbps tree — the common relay in exactly the trees where scarcity makes contribution matter.
*   **Overlap and oversubscribe for five seconds**: $107\%$ of the link in a $1.5$ Mbps tree, $125\%$ in a $3.0$ Mbps one, on every preemption; loss on every child, parity up, $K_v$ down, a child drained — the loop SOLUTION-025 damped, reopened.
*   **Raise $r_{\text{pull}}$ to cover a handover** ($\ge 17\%$ for a $1.5$ Mbps overlap on 10 Mbps) taxes every relay permanently for a transient.

## Defects found during verification

*   $10 B_m \Omega$ thresholds: $8.6$ Mbps for a $0.75$ tree, $17.3$ for $1.5$, $34.5$ for $3.0$. The one case the reserve covered was the tree bitrate that preempts least, since preemption only happens in enhancement trees, which minimax puts at the largest $B_m$.
*   A multi-tree node could run $t_v$ concurrent overlaps, each "the one extra slot", all against the same reserve; the owning expression counts them.
*   `PullSlots` in Ch5 was a constant of $u_v$; it is now a function of the handovers in progress, which is what the drain notice (SOLUTION-046) keeps short.

## The generalisable lesson

**Every budget needs one inequality that lists all of its consumers; a consumer added in another chapter is added to the inequality or it is not added.** Design principle 2 stated this for parameters; it applies to shares of capacity too.

## Residual risk

A sequential handover leaves the joiner without a parent in the tree for up to the drain time; with the notice that is $\approx 2$ RTT, without a re-attach by the drained child it is $\tau_{\text{drain}}$. Acceptable for enhancement trees, where it applies.

## Validation owed (Chapter 8)

*   Fraction of preemptions that are sequential versus overlapped at a realistic upload distribution, and the joiner's wait in the sequential case.
*   PULL-service PSR impact of handover overlaps on relays between $10 B_m \Omega$ and $20 B_m \Omega$.
