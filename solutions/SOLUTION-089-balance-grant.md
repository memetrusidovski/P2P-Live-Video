# SOLUTION-089: Balance Grant for a Tree Short of Relays

**Closes:** ISSUE-089 (High)
**Lives in:** Ch1 §1.2.1 §1.3 (after the coverage grant); `src/crates/bs-core/src/node/balance.rs`, the admission check and `request_next` in `node.rs`, `Params::balance_grant`
**Class:** A property that holds in expectation, used as if it held in every swarm

---

## The problem in one line

Each relay picks its tree by a hash, so a tree's relay count is binomial. One swarm in ten below about 50 relays had a tree with too few slots for its viewers, and nothing rebalanced it.

## The decision

*   A relay takes its second-ranked tree when that tree holds under 60 % of the mean relay count and its own first-ranked tree is at or above the mean. It judges from the relay records it already knows.
*   It volunteers with probability `1 − c_b / mean`, drawn from its NodeID. The expected number of volunteers is the shortfall, and the set does not change between evaluations.
*   A grant spends spare capacity only. No child of the first-ranked tree is drained.
*   A parent verifies a claimed assignment against the first or second rendezvous rank.
*   The joiner never requests a relay that is its parent, or pending parent, in another tree.
*   No grant near a resize or with fewer than 6 known relays per tree.

## Why this and not the alternatives

*   **The joiner asks, as in the coverage grant**: every starved joiner would ask the same few relays at once. A relay cannot tell a short tree from one joiner's bad luck.
*   **The publisher announces the short tree**: it holds the true counts, but that needs a new field in a signed frame and a round trip.
*   **Halve the first tree's slots at once** (a plain `t_v = 2` node): measured, it drained half the relay's children and added repairs in swarms that were balanced.
*   **Weight the base-layer tree in the rank**: fixes one tree and breaks the stability of the rank under resizes.

## Measured

Sixty viewers, 45 relays, the two seeds that put 8 or 9 relays in tree 1 (`scenarios/tree_short_of_relays.toml` is seed 27):

| Seed | Relays in tree 1 | Join mean before | Join mean after | Starved before | Starved after |
|---|---|---|---|---|---|
| 25 | 9, then 13 | 5.2 s | 3.4 s | 0.11 % | 0.03 % |
| 27 | 8, then 13 | 7.3 s | 5.9 s | 0.47 % | 0.15 % |

Over eight seeds of `baseline_100`, `churn_storm` and `internet_far_swarm` the means of starvation, join time and repair time are equal or better with the grant.

## Residual risk

*   The grant starts after a relay is Active and knows 12 relays, so the first joins of a short tree are still slow: seed 27's join p95 is 13 s.
*   The counts are a sample of the swarm. A balanced swarm sees a spurious grant now and then. It costs nothing but spare capacity.
*   Under the dynamic forest ladder the grant stays off: trees there hold few relays by design. A short tree at a high rung still relies on the coverage grant, which the implementation lacks.
*   A simulator gate that tested bridges (`nat_symmetric`) needs none with the grant, and now runs with `balance_grant = false`.

## Validation owed

*   A core test that feeds a relay a skewed set of records and asserts the grant, the slot split and the lapse.
*   A swarm of a few hundred relays, where the sample is a small share of the swarm.
