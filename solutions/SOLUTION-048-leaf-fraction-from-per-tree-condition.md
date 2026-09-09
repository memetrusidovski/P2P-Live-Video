# SOLUTION-048: The Leaf-Fraction Bound Follows From the Per-Tree Condition

**Closes:** ISSUE-048 (Medium); supersedes the $51.6\%$ figure of SOLUTION-016 and its restatement in SOLUTION-031
**Lives in:** `protocol/chapter1/1.2_multi_forest_overlays/5_node_classes.md` (*How Many Leaves the Swarm Can Absorb*, rewritten); `protocol/chapter1/1.1_scale_latency/5_capacity_adaptation.md` (§5.1 addendum, §5.2)
**Class:** A guarantee derived by summing a resource the mapping does not pool (recurring pattern #1)

---

## The problem in one line

§5.3 bounded the leaf fraction at $51.6\%$ by treating $20\%$ of *all* relay upload as base-layer supply for leaves; only the $t_0/M$ of relays assigned to $L_0$ trees supply $L_0$, every peer draws on it, and a relay delivers $0.78\,\bar{u}$ of media, not $\bar{u}$ — the true bound at 8 Mbps and $M = 6$ is $28\%$.

## The decision

The bound is the §5.1 per-tree condition solved for $\ell$: $\ell \le 1 - M B_0 \Omega / ((1 - r_{\text{pull}}) \bar{u}_{\text{relay}})$ with $B_0 = b_0/t_0$ the current $L_0$ stripe bitrate. Tabulated per mapping at 6, 8, 10, 12 Mbps: $4/28/42/52\%$ at $(2,2,2)$; $36/52/62/68\%$ after folding $L_2$; $68/76/81/84\%$ with $L_0$ alone. The publisher lever is the **mapping** (the fold rule of SOLUTION-040), not the floor cap.

## Why this and not the alternatives

*   **Keep the $20\%$-of-upload derivation and lower the number.** The derivation has the wrong shape, not the wrong constant: it does not depend on $M$ or $t_0$, and the true bound moves from $4\%$ to $68\%$ across mappings at the same 6 Mbps.
*   **Raise the floor cap to 25–30%** (the old guidance). The cap decides which peers go without $L_0$ when it is short, not whether any do; at $\ell = 40\%$ and 8 Mbps relays the $L_0$ stripes supply $1.6N$ of $2N$ needed slots regardless of the cap. The guidance is deleted; the cap remains a fairness knob (SOLUTION-049).

## Defects found during verification

*   Slot rounding tightens the first column further: at 8 Mbps, $K_v = \lfloor 7.2/0.8625 \rfloor = 8$, supply $2.67(1-\ell)N \ge 2N \Rightarrow \ell \le 25\%$.
*   The $M = 4$ rung is the worst for the base layer under minimax allocation: $(1,1,2)$ gives $L_0$ one tree in four at $1.5$ Mbps, needing $7.67/(1-\ell)$ Mbps against $5.75$ at $M = 3$ and $4.79$ at $M = 5$. The ladder step $3 \to 4$ *raises* the base layer's requirement. Filed as ISSUE-058; the fold rule mitigates it dynamically.
*   SOLUTION-016's "$25\%$ buys $57\%$, $30\%$ buys $62\%$" was an artefact of the same formula.

## The generalisable lesson

**An entitlement bound must be derived under the same partition of resources the protocol actually enforces.** "20% of relay upload" was a number nobody could spend, because no relay can spend upload in a tree it is not assigned to.

## Residual risk

The bound assumes relays of an $L_0$ tree are drawn uniformly from the relay population (rendezvous makes this true in expectation) and that leaves hold no more of $L_0$ than their share; a leaf-heavy prefix concentrated under a few relays is bounded separately by the prefix cap (SOLUTION-042).

## Validation owed (Chapter 8)

*   Observed base-layer PSR against $\ell$ at 8 Mbps relays, $M = 6$, with and without the fold rule — the curve should break at $\approx 25$–$28\%$, then at $\approx 52\%$.
