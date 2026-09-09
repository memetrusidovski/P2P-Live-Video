# SOLUTION-007: Capacity-Proportional Multi-Tree Assignment (Super Nodes)

**Closes:** ISSUE-007 (High)
**Lives in:** `protocol/chapter1/1.2_multi_forest_overlays/1_graph_theory_and_slicing.md` §1.3
**Class:** Capacity utilisation / infrastructure economics

---

## The problem in one line

Every peer relayed exactly one hash-assigned tree, so a 10 Gbps server spent its entire budget on one slice and gave the other five nothing — and because assignment is by hash, covering all six trees took roughly 14 servers rather than one.

## The decision

A node relays in as many trees as it has *full-stream-equivalent* upload for:

$$\text{max\_trees}(v) = \min\left(M,\ \max\left(1, \lfloor u_v / B \rfloor\right)\right), \qquad \text{per\_tree\_slots}(v) = \left\lfloor \frac{u_v}{\text{max\_trees}(v) \cdot B_m} \right\rfloor$$

Assignments spread round-robin from the hash-derived primary tree, preserving deterministic dispersion. A 10 Gbps server becomes a Layer-1 relay in all six trees at ~1,666 children each.

## Why the threshold is $u_v \ge 2B$ and not $2B_m$

Multi-tree eligibility is measured in *whole streams*, not slices, because a node interior in $k$ trees is committing to carry $k$ slices' worth of children — and the meaningful unit of "this node can do more than its share" is the full bitrate it is itself consuming. Using $B_m$ would grant multi-tree status to almost every peer at $M = 6$, recreating the hotspots the Orthogonal Placement Rule exists to prevent.

## Two defects found while verifying — both serious

The applied fix addressed bandwidth concentration via the per-tree slot cap, and stopped there. Concentration has two other dimensions.

### 1. It silently destroyed failure independence

**This is the important one.** The entire reason the overlay is $M$ edge-disjoint trees rather than one tree is that losing a node costs a child $1/M$ of its bitrate. Multi-tree assignment breaks that guarantee — and worse, breaks it *by default*, because the same super node wins the capacity score in every tree simultaneously. Parent selection converges on it in all $M$ trees, and one machine's failure takes 100% of that child's stream at once. The multi-forest becomes an expensive way to build a single tree.

The per-tree slot cap does not help: it bounds how many children one node serves *per tree*, not how many trees one child takes from *one node*.

The fix is per-child, not per-server:

> A peer selects at most one parent per distinct NodeID across all $M$ trees.

The elegant part is that **this costs the super node nothing**. It still fills $6 \times 1666$ slots — with 10,000 distinct children instead of 1,666 children taken six times. The safe configuration was already the natural one; the rule exists purely to stop greedy per-tree scoring from finding the unsafe one.

Where it cannot be satisfied (too few relays to offer $M$ distinct parents), the peer marks those trees as a **correlated failure domain**, and churn recovery pre-selects replacements for the whole set rather than repairing tree by tree.

### 2. It multiplied the blast radius of a lie by M

$u_v$ is self-declared — the one input in parent selection taken purely on trust. Under the single-tree rule, a node lying about capacity could attract and starve one tree's children, bounded and quickly corrected by the reliability term $R$. Multi-tree assignment hands the same liar interior status in *every* tree. Warm-up gating (SOLUTION-005) forces it to receive a verified segment, but nothing forced it to forward one.

Resolution: eligibility is gated on **demonstrated** throughput, not claimed capacity.

$$\text{max\_trees}^{\text{eff}}(v) = \min\left(\text{max\_trees}(v),\ 1 + \left\lfloor \Theta^{\text{rate}}_v / B \right\rfloor\right)$$

where $\Theta^{\text{rate}}_v$ is PoU-verified delivered throughput over the trailing 60 s — receipts signed by children actually served. Every node starts at one tree and earns each further one by having already delivered a full stream's bitrate. A real 10 Gbps server reaches six trees in a couple of minutes; a liar never leaves its first tree.

This also removes an inconsistency: capacity was the sole claim the protocol accepted unverified, in a design whose stated axiom is that standing follows contribution.

## The generalisable lesson

**When you relax a constraint, enumerate everything it was accidentally providing.** The Orthogonal Placement Rule was documented as anti-hotspot, and the fix preserved that property explicitly. It was *also* silently providing failure independence and a bound on how much damage one false capacity claim could do. Neither was written down, so neither was preserved. The per-tree slot cap addressed the one stated purpose and left the two unstated ones broken.

## Validation owed (Chapter 8)

* Stream loss when a super node backboning all $M$ trees is killed, with and without the distinct-parent rule — should be $1/M$ versus total.
* How quickly a genuine high-capacity node ramps to full multi-tree standing under the $\Theta^{\text{rate}}$ gate, and whether 60 s is the right window.
* Whether the distinct-parent rule measurably degrades parent quality at moderate $N$, where distinct high-capacity parents may be scarce.
