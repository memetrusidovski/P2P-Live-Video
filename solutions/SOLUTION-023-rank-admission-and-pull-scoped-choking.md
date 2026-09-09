# SOLUTION-023: Tree Slots Are Allocated by Rank; Choking Governs PULL Service Only

**Closes:** ISSUE-023 (High)
**Lives in:** `protocol/chapter1/1.2_multi_forest_overlays/2_parent_selection_algorithm.md` (*Rank Admission Rule*); `protocol/chapter5/5.1_tit_for_tat/2_sliding_window_unchoker.md` (rewritten), `3_optimistic_exploration.md`; `protocol/chapter1/1.2_multi_forest_overlays/5_node_classes.md` §5.5; `appendix_b_parameters.md`; `appendix_d_frame_registry.md` (`DISCONNECT` reason `PREEMPTED`)
**Class:** An incentive layer designed for one architecture (mesh pull) bolted onto another (tree push)

---

## The problem in one line

The unchoker allocated $K_v$ — the tree child slots — by each neighbour's ingress rate to us, which for a tree child is zero by construction, so read literally the forest dissolved after one cycle; read charitably, nothing at all connected contribution rank to tree slots, and the protocol's core axiom ("contribution buys placement and quality") was implemented by no algorithm.

## The decision

*   **Choking is scoped to PULL service.** Tit-for-Tat runs over the PULL reserve $r_{\text{pull}} u_v$ (SOLUTION-025), in slots of $\beta_{\text{pull}} = 0.5$ Mbps, measuring reciprocity in PULL bytes — the bidirectional mesh traffic it was designed for. A choked peer keeps its pushed stripe; it loses repair and backfill service.
*   **Tree slots are allocated by the Rank Admission Rule.** A join may carry a `RANK_PROOF`; the parent accepts on a free slot; otherwise, in an *enhancement* tree, it preempts its lowest-ranked child if the joiner's rank exceeds $1.25\times$ that child's, draining the preempted child over $\tau_{\text{drain}}$ and charging the one transient extra slot to its PULL reserve. Base-layer slots are never rank-preempted and $20\%$ of them are held for leaves — the service floor, now a tree-admission rule.
*   **Bootstrapping is through the base layer.** A new relay ranks 0, obtains an $L_0$ slot, relays, earns receipts, and preempts upward within a minute. Shallower placement emerges from the migration loop plus preemption.
*   **The FIFO bootstrap queue survives**, re-scoped to what it was always solving: a joiner with an empty buffer needs late-join backfill over PULL before it can reciprocate.

## Why this and not the alternatives

*   **Keeping tree slots under the unchoker with "reciprocity" redefined as receipts issued**: a child issues receipts to its parent for what it *received*, so every child reciprocates equally and the unchoker ranks nothing. Receipts measure the *parent's* contribution, not the child's.
*   **Rank-gated admission without preemption** ("full is full") means rank matters only while slots are free — i.e. never under the saturation that makes rank matter. Preemption is what turns rank into placement.
*   **Preempting in the base layer too** would let contributors push leaves off $L_0$, breaking the floor guarantee the leaf class rests on. The base layer is the on-ramp and the floor; contention there is the source reserve's job.
*   **A separate floor reservation inside the PULL budget** (the old 20% of $K_v$) mixed two budgets and left the floor with no enforcement point on the tree, where the base layer is actually delivered.

## Defects found during verification

*   The old §5.1.3 budget — optimistic $+$ floor $+$ regular $\le K_v$ — counted PULL slots against tree slots. With $r_{\text{pull}}$ now reserved separately (SOLUTION-025), the same file would have double-counted; it is restated over `PullSlots`.
*   The file carried a second, stale pseudocode block from before SOLUTION-003 (random selection over `NewJoiners`, no $K_v$ bound) after the FIFO version. Removed.
*   §5.2.2's "presents a bundle of verified receipts when connecting to a new parent" had no frame and no verifier rule; `RANK_PROOF` (SOLUTION-024) and this rule are that frame and that rule.
*   With rank 0 for both, a leaf and a brand-new relay are indistinguishable to an enhancement-tree parent; both get slots only when free. This is intended — a relay earns its way up through $L_0$ — and it is stated.

## The generalisable lesson

**Name the resource each incentive mechanism allocates, and check that the signal it reads is nonzero for the peers competing for that resource.** Tit-for-Tat reads reciprocal flow; tree edges have none. The mechanism was sound; it was pointed at the wrong resource.

## Residual risk

*   Preemption churn: a swarm with many near-equal contributors could see enhancement slots change hands often. The $1.25\times$ margin and one-preemption-per-tree bound limit the rate; Chapter 8 should measure it.
*   Rank is per join attempt via `RANK_PROOF`; a child's rank is not re-evaluated while it holds a slot, so a contributor who stops contributing keeps its slot until someone better arrives. Acceptable — decay in $\Theta$ makes it preemptible within minutes.

## Validation owed (Chapter 8)

*   Fraction of enhancement-tree slots held by rank $> 0$ peers under $\sigma < 1$, over time — the axiom's empirical test.
*   Preemption rate and its effect on PSR for the preempted.
*   Whether $\beta_{\text{pull}} = 0.5$ Mbps per slot lets a joiner backfill 750 KB within the 1.5 s SJL target from its unchoked neighbours.
