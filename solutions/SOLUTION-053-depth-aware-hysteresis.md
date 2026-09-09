# SOLUTION-053: The Hysteresis Margin Yields to One Hop of Depth

**Closes:** ISSUE-053 (Low); corrects the justification recorded in SOLUTION-021
**Lives in:** `protocol/chapter1/1.2_multi_forest_overlays/2_parent_selection_algorithm.md` §2.3; `appendix_b_parameters.md` (HysteresisMargin)
**Class:** An arithmetic claim about a formula, checked at the wrong end of its range (recurring pattern #11)

---

## The problem in one line

The 30 ms margin was justified as "small enough that one hop of depth (at least $\approx 55$ ms at $h \le 3$) always wins"; with $w_h = 20$, $\lambda = 0.5$ one hop is worth $12.97\,e^{0.5h}$ ms — $13$, $21$, $35$, $58$ for $h = 0 \ldots 3$ — so 55 ms was the *largest* swing at $h \le 3$, and a node at depth 2 never moved to an otherwise-equal parent at depth 1.

## The decision

$\text{Margin} = \min(30\text{ ms}, \Delta_h(h_{\text{new}})/2)$ when the candidate is strictly shallower, $30$ ms otherwise, with $\Delta_h(h) = w_h(e^{\lambda(h+1)} - e^{\lambda h})$. One hop always wins in the shallow direction on its own; the full 30 ms is still needed to move back down, so jitter cannot flap a node between depths.

## Why this and not the alternatives

*   **Lower the margin to $\approx 10$ ms for everyone**: below cellular RTT jitter, so equal-depth parents would flap.
*   **Keep 30 ms and state that depth alone does not move nodes at $h \le 2$**: honest, but it gives up the claim "contributors drift toward the source" exactly where fan-out leverage is highest.
*   **Exempt depth from the margin entirely** (migrate to any shallower parent that is not worse): flaps under jitter of $\pm$ the small non-depth difference; halving the depth gain keeps a margin in both directions.

## Defects found during verification

*   The table of one-hop swings; the earlier claim confused the maximum over $h \le 3$ with the minimum.
*   Between equal-depth candidates nothing changes; the asymmetry is confined to moves that change depth, which is the only place the old margin was wrong.

## The generalisable lesson

**Check a monotone formula at both ends of the range a claim covers.** "At least X at $h \le 3$" was evaluated at $h = 3$ only.

## Residual risk

None beyond the jitter validation SOLUTION-021 already owed.

## Validation owed (Chapter 8)

*   Migration oscillation rate at cellular jitter with the asymmetric margin.
*   Mean depth of top-decile contributors over time, with and without the change.
