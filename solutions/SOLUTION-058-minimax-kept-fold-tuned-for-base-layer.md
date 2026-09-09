# SOLUTION-058: Minimax Allocation Is Kept; the Fold Rule Protects the Base Layer at the $M = 4$ Rung

**Closes:** ISSUE-058 (Low); amends the unfold policy of SOLUTION-040
**Lives in:** `protocol/chapter1/1.2_multi_forest_overlays/4_stream_slicing_architecture.md` §4.2.1 (*Minimax is not base-layer-first*); `protocol/chapter1/1.1_scale_latency/5_capacity_adaptation.md` §5.6 ($\tau_{\text{fold}}$ by layer, growth-gated unfold); `appendix_b_parameters.md`
**Class:** A ladder whose rungs were checked for the top layer and not for the floor (recurring pattern #11)

---

## The problem in one line

Minimax allocation gives the base layer extra trees only on ties, so at $M = 4$ the reference ladder puts $L_0$ on one tree in four at $1.5$ Mbps and its requirement rises from $5.75$ Mbps ($M = 3$) to $7.67$ — adding relays made the floor harder to carry, at precisely the residential uploads the specification calls the common case.

## The decision

*   **Minimax is kept.** With the fold rule in place, minimax and a base-first alternative reach the same steady state wherever they differ ($(2,2)$ after one fold for $3.83 \le \bar{u} < 7.67$ Mbps), and minimax delivers 1080p to everyone for $7.67 \le \bar{u} < 15.3$ where base-first delivers it only to $L_2$ slot winners. The per-rung table for both objectives is in §4.2.1.
*   **The transient is paid for by the fold rule.** Under minimax the layer that starves until the fold fires is the *base* layer, a freeze; so $\tau_{\text{fold}} = 10$ s when the starved layer is $L_0$ ($30$ s otherwise).
*   **Unfolding is growth-gated only.** SOLUTION-040's timer-driven unfold probe is removed: a fold happens only because a lower layer starved, so a wrong unfold re-creates that starvation — for $L_0$, a freeze — for a full $\tau_{\text{fold}}$, and a timer would do that to a stable swarm forever. A folded layer is restored only when $N_{\text{relay}}$ has grown $\ge 50\%$ since the fold.

## Why this and not the alternatives

*   **Base-first allocation** $(2,1,1)$ at $M = 4$: $L_0$ at $3.83$, $L_2$ at $15.3$. Strictly better for the floor before any fold, strictly worse for 1080p across the band $7.67$–$15.3$ Mbps that a healthy swarm lives in, and identical after the fold that both need below $7.67$.
*   **Skip the $M = 4$ rung** ($3 \to 5$ at $N_{\text{relay}} \ge 24$): $M = 3$ is better than minimax $M = 4$ for $5.75 \le \bar{u} < 7.67$ (no fold needed) and worse for $7.67 \le \bar{u} < 11.5$ (1080p partially starved). Mixed, and it costs coverage probability at 18–23 relays for no net gain once the fold rule exists.
*   **Keep the timer-driven unfold and shorten $\tau_{\text{fold}}$ further**: still a periodic freeze for the starved fraction, just a shorter one.

## Defects found during verification

*   The requirement sequence for $L_0$ across rungs, $7.67, 5.75, 7.67, 4.79, 5.75$, is not monotone; SOLUTION-019 checked that lower layers are never harder than upper layers *within* a rung and did not check the floor *across* rungs.
*   The unfold probe of SOLUTION-040 was a defect in a fix from the same day: evaluated at $M = 4$ with 5–7 Mbps relays, it would have frozen the base layer for a fraction of the swarm for 30 s every 10 minutes, doubling. Caught while tracing this issue's mitigation; fixed here and noted in SOLUTION-040.

## The generalisable lesson

**Check a ladder against the floor as well as the ceiling, at every rung — and never probe a state whose failure mode is a freeze.** A probe is only acceptable where being wrong is a resolution loss.

## Residual risk

*   A swarm whose relays' upload improves without their number changing stays folded. Accepted; the publisher has no signal for it, and Chapter 8 owes the frequency.
*   The 10 s base-layer fold window makes a transient $L_0$ shortage (a burst of leaf joins before their relays warm) more likely to trigger a fold; the warm-up exclusion and the $5\%$ threshold are the guards.

## Validation owed (Chapter 8)

*   Base-layer PSR through the $3 \to 4 \to 5$ rungs at $\bar{u}_{\text{relay}} \in \{5, 6, 7, 8\}$ Mbps with the layer-specific $\tau_{\text{fold}}$.
*   Spurious fold rate at $\tau_{\text{fold}} = 10$ s under Scenario A churn.
*   How often a folded swarm would have been safe to unfold on upload improvement alone.
