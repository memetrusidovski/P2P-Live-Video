# SOLUTION-050: The Bridge Is a Tree Edge; Ingress Relays Are Budgeted Roots

**Closes:** ISSUE-050 (Medium); reverses the reserve-charging decision of SOLUTION-032
**Lives in:** `protocol/chapter6/6.3_emergent_relays/1_recruitment_criteria.md` (*A bridge is a tree edge, twice*, *Cost accounting*), `2_3x_multiplier_economics.md`, `3_relay_frames.md` (field semantics, *An ingress relay is a root*); `protocol/chapter1/1.2_multi_forest_overlays/1_graph_theory_and_slicing.md` (out-degree exception, $U_{\text{bridge}}$); `protocol/chapter1/1.1_scale_latency/5_capacity_adaptation.md` §5.4; `appendix_b_parameters.md`
**Class:** A mechanism added to one chapter without its accounting, receipts or placement in the chapters that own them (recurring pattern #2)

---

## The problem in one line

The bridge charged $B_m\Omega$ to a 10% reserve that on the 20 Mbps node the criteria admit could not hold a single 3.0 Mbps tree; nothing said whether the relay or the NAT-blocked peer was the upstream parent's child, so under the receipt deadline the bridge died after three segments; and an ingress relay that "becomes hop 0 of the forest" had a root budget of 2 Mbps.

## The decision

*   **Bridge = two ordinary tree edges.** $R$ is $B$'s child in $T_m$ (joins with `NEIGHBOR` and its rank, takes a slot, decodes/verifies/re-encodes, issues $B$ ordinary receipts); $A$ is $R$'s child (issues $R$ `RELAYED` receipts). $R$'s out-degree one in a non-assigned tree is the single, bounded exception to the Orthogonal Placement Rule; it never sets that tree's bit in `AssignedTrees`.
*   **Bridged children are charged to the tree budget** $(1 - r_{\text{pull}})u_R$ as $U_{\text{bridge}} \le \frac{1}{2}(1 - r_{\text{pull}})u_R$, with $R$'s own $K_v(m)$ computed with $U_{\text{bridge}}$ removed. The $3\times$ multiplier is thereby bounded at $2\times$ the rank of pure relaying.
*   **Ingress relays are $t_v = M$ roots**: $K_R(m) = \lfloor ((1 - r_{\text{pull}})u_R - R_{\text{src}}/n_{\text{ingress}}) / (M B_m \Omega) \rfloor$, at least 2 per tree — $\ge 18$ Mbps at $M = 2$, $\ge 26$ Mbps at $M = 6$ with two ingress relays, 40 Mbps recommended. The source's base-layer pool is delegated to them. `RELAY_PROPOSAL` may leave the parent choice to $R$; `RELAY_BIND` gains `REJECTED`.

## Why this and not the alternatives

*   **Keep the reserve charge and raise the `RELAY_CAPABLE` bar** to $\approx 35$ Mbps so one 3.0 Mbps bridge fits: fewer relays qualify exactly at small $N$, where NAT-blocked broadcasters live, and the reserve still could not hold two.
*   **Transparent forwarding** ($A$ is $B$'s child at $R$'s address): $B$ never receives receipts for the slot ($A$ issues them to $R$) and evicts it in three segments; and $R$ cannot size parity per link without decoding.
*   **Charge bridges to a third budget**: a third inequality to keep consistent for a case the tree budget already describes — a bridged child costs exactly what a tree child costs.

## Defects found during verification

*   SOLUTION-032's argument — bridging "would otherwise silently shrink the forest's capacity" — was wrong in kind: a bridged child is a subscriber served, and the upload spent on it is not lost to the forest. The reserve charge was the actual defect; stated plainly.
*   Budget arithmetic at 20 Mbps, reserve 2 Mbps: $0.86$ Mbps per 0.75 tree (two fit), $1.73$ per 1.5 tree (one), $3.45$ per 3.0 tree (none). The $M \le 3$ swarm — the home-broadcaster case — could bind nothing.
*   The first draft of the ingress minimum in this run assumed slots spread evenly across trees and quoted 20 Mbps; the largest per-tree bitrate decides, and at $M = 6$ the two-slot floor is $25.9$ Mbps. Corrected before closing.

## The generalisable lesson

**A mechanism that creates an edge in the tree must say which node is the child, where the receipts go, and which budget pays — in the chapters that own trees, receipts and budgets, not only in its own.**

## Residual risk

*   The $2\times$ rank bound makes bridging the most lucrative use of upload; demand-limited (a bridge exists only on a proposal) and capped at half a relay's budget, but Chapter 8 should watch for relays preferring bridges to their assigned tree.
*   A peer bridged through one relay in several trees has correlated parents; marked as such under the Distinct-Parent Rule, but small swarms may offer no alternative.

## Validation owed (Chapter 8)

*   Scenario C (75% behind symmetric NAT): bridged fraction, $U_{\text{bridge}}$ utilisation, and PSR of bridged peers.
*   Root availability and per-tree slot counts with one and two ingress relays at 20 / 40 Mbps.
