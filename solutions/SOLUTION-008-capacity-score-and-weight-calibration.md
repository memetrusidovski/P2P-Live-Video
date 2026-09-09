# SOLUTION-008: Capacity Compression and Parent-Score Weight Calibration

**Closes:** ISSUE-008 (Low — with a High-severity consequence)
**Lives in:** `protocol/chapter1/1.2_multi_forest_overlays/2_parent_selection_algorithm.md` §2.1, §2.1.1, `appendix_b_parameters.md`
**Class:** Parent selection / topology quality

---

## The problem in one line

`CapacityScore = ln(1 + K_avail) · R` is nearly flat at the top: it gave a 10 Gbps server (10,000 slots) only a 3.8× advantage over a 10 Mbps node, so nearby mid-tier peers routinely outscored idle backbone capacity.

## The decision

Square-root compression, **saturating** at a reference slot count, with all three score terms re-expressed in milliseconds:

$$\text{Score}(p, i) = w_c \cdot \min\left(\sqrt{K_{\text{avail}}}, \sqrt{K_{\text{ref}}}\right) \cdot R \;-\; \text{RTT}(p,i) \;-\; w_h\left(e^{\lambda h_p} - 1\right)$$

with $w_c = 12$, $w_h = 20$, $K_{\text{ref}} = 256$, $\lambda = 0.5$.

## The defect: the weights were never re-derived

This is the whole lesson of this issue, and it is worth stating plainly because it is an easy mistake to repeat.

The weights $(w_1, w_2, w_3) = (1000, 1, 50)$ were **calibrated for the logarithm**. That matters because $\ln(1 + K_{\text{avail}})$ spans only $2.4 \to 9.2$ across the entire plausible range of $K_{\text{avail}}$ — a 3.8× spread, comparable in magnitude to the hop term. *That* is what made those weights balanced. The flatness the issue complained about was simultaneously the property holding the score together.

Swapping in $\sqrt{\cdot}$ widened the span to $3.2 \to 100$ — 31.6× — and the weights were left untouched. Result:

| Candidate | Old score $(1000, 1, 50)$ with $\sqrt{\cdot}$ |
| :--- | ---: |
| 10 Gbps server, another continent (RTT 300, $h{=}1$) | ~99,618 |
| Excellent peer, same city (RTT 10, $h{=}3$) | ~3,070 |

Capacity outweighs latency **thirty to one**. Every peer on earth routes to the super node until it saturates — and with 10,000 slots that takes a long time. The forest scatters globally, and $RTT_{\text{avg}} = 80\text{ ms}$, the assumption the entire latency proof of §1.1.3 rests on, stops holding. The fix for under-using backbone capacity had replaced it with latency-blind monopolisation of backbone capacity.

The issue text even flagged the exponent as "a tuning parameter to be validated by simulation" — but treated the *exponent* as the tunable thing, when the exponent and the weights are one joint calibration.

## Two changes that restore balance

**1. Capacity saturates at $K_{\text{ref}} = 256$.** The justification is a distinction the original formula elided: beyond a few hundred free slots, more capacity does not make a parent better **for this child**. It makes it better *for the swarm* — and that value is realised by the super node *accepting many children*, not by it outscoring every alternative for each one individually. Unbounded per-child reward for a global property is the modelling error. Credit caps at $12 \times 16 = 192\text{ ms}$.

**2. Everything is in milliseconds.** RTT enters at unit weight and defines the scale; capacity and depth are expressed as millisecond credit and penalty. Every term is now legible and boundable instead of being an arbitrary product of dimensionless constants. The hop penalty also gained a $-1$ so it is zero at the source rather than carrying a constant offset.

Behaviour at $R = 1$:

| Candidate | RTT | $h_p$ | $K_{\text{avail}}$ | Score |
| :--- | :---: | :---: | :---: | ---: |
| Local mid-tier peer | 20 | 3 | 10 | $-52$ |
| Distant super node | 250 | 1 | 10,000 | $-71$ |
| **Local super node** | 20 | 1 | 10,000 | $+159$ |
| Nearly-saturated local peer | 20 | 3 | 1 | $-78$ |
| Deep local peer | 20 | 7 | 10 | $-625$ |

Backbone capacity wins when reachable, loses to a good local peer when it is not, and depth still dominates — which is what keeps the tree shallow.

## The generalisable lesson

**Weights and the transform they weight are one calibration, not two.** Any future change to the compression curve, to $\lambda$, or to $K_{\text{ref}}$ invalidates the others and requires re-deriving all of them together. Recorded in §2.1.1 in the spec so the next person changing the curve sees it.

A corollary worth carrying: **keeping a scoring function dimensionally consistent is a correctness property, not a style preference.** The bug was invisible while the terms were dimensionless products; it is obvious once every term is milliseconds.

## Validation owed (Chapter 8)

* $D_{\text{avg}}$ and end-to-end latency under the rebalanced weights versus the old ones, at $N = 10^4$–$10^6$ with a realistic geographic RTT distribution.
* Whether $K_{\text{ref}} = 256$ is right, or whether super-node utilisation suffers at a cap that low.
* Whether the sqrt exponent itself is optimal once the weights are balanced — the two must be swept jointly, not independently.
