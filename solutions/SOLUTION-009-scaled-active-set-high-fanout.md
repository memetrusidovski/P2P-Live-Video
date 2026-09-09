# SOLUTION-009: Scaled Active Set for High-Fan-Out Relays

**Closes:** ISSUE-009 (Medium)
**Lives in:** `protocol/chapter3/3.1_neighbor_sets/1_memory_structures.md`, `appendix_b_parameters.md`
**Class:** Control-plane scaling

---

## The problem in one line

$c_a = 8$ sizes the gossip layer, but a super node has $K_v$ tree children — so a relay with 10,000 children exchanges bitfields with 8 of them (0.08%), and the rest must hunt for missing blocks through multi-hop gossip among siblings who are usually missing the same blocks.

## The decision

$$c_a^{\text{eff}} = \min\left(64,\ \max\left(c_a,\ \lfloor K_v / 10 \rfloor\right)\right)$$

Flat at 8 until $K_v = 80$, linear to the ceiling at $K_v = 640$, 64 thereafter. Extra slots are filled preferentially with the relay's own tree children.

## Defect found: the applied formula was a regression for mid-size relays

The issue proposed `min(64, max(8, floor(K_v/10)))`. The spec dropped the `max`, writing a two-branch form that used $\lfloor K_v/10 \rfloor$ whenever $K_v > 2c_a$:

| $K_v$ | As specified | Intended |
| ---: | ---: | ---: |
| 17 | **1** | 8 |
| 20 | **2** | 8 |
| 50 | **5** | 8 |
| 80 | 8 | 8 |
| 10,000 | 64 | 64 |

For every $K_v$ between 17 and 79 the "scaling" rule scaled relays *down*, bottoming out at a single gossip peer for a relay with 17 children — strictly worse than doing nothing, and squarely in the range where ordinary residential relays live. The single-expression form with the `max` restored is monotone by construction, so the class of error cannot recur silently.

## The honest accounting the original fix omitted

At $K_v = 10{,}000$ the fix raises direct bitfield visibility from 0.08% to 0.64%. That is an 8× improvement on a negligible number, and 99.36% of children still cannot see their parent's bitfield. The spec presented this as the resolution; it is a mitigation.

It is nonetheless *acceptable*, for a reason worth writing down because it reframes the problem:

* **On the push path, children need no bitfield at all.** Symbols arrive proactively. The bitfield matters only for repair.
* **For repair, the parent is the wrong source.** Siblings under the same relay in the same tree hold exactly the same blocks, are one hop away, and are not a shared bottleneck. A repair pulled from the parent competes with the parent's push duty; a repair pulled from a sibling does not. Converging $K_v$ children onto the parent for repair would be actively harmful even if the bitfield were universally visible.
* **FEC absorbs the common case first.** Per-link adaptive parity (Ch4 §4.2.2) repairs most loss with no PULL at all.

So the quantity that actually bounds PULL latency at extreme fan-out is **sibling mesh density**, not parent visibility — and sibling peering happens through ordinary `SHUFFLE` gossip at $O(c_a)$ per peer, *independent of $K_v$*. It does not degrade as the relay grows. Scaling $c_a$ helps the parent see more of its children; it was never going to be the mechanism that scales repair, and the spec now says so.

## Why the relay-broadcast channel stayed rejected

The architecturally clean alternative — the relay unicasts its bitfield to all $K_v$ children every 100 ms — was kept as a documented future option. Beyond costing a new frame type and a second distribution mechanism, it re-creates exactly the $O(K_v)$ per-parent cost that bounding the Active Set exists to avoid. It moves the scaling problem rather than solving it.

## The generalisable lesson

**A formula transcribed from an issue into a spec must be re-checked at its boundaries, not just at the extreme it was designed for.** The proposal was validated at $K_v = 10{,}000$, where both forms agree at 64. The `max` looked redundant there, was dropped, and broke the mid-range that nobody re-evaluated. Piecewise definitions invite this; a single monotone expression resists it.

## Validation owed (Chapter 8)

* PULL latency versus $K_v$ under correlated loss, decomposed by repair source (parent / sibling / FEC), to confirm siblings carry the load as claimed.
* Whether 64 is the right ceiling, and whether $K_v/10$ is the right slope, once sibling peering is measured rather than assumed.
* Control-to-Data Overhead at $c_a^{\text{eff}} = 64$ against the $\text{CDO} \le 2\%$ target.
