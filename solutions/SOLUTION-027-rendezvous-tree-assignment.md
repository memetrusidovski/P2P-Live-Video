# SOLUTION-027: Rendezvous Tree Assignment and Incremental Forest Resize

**Closes:** ISSUE-027 (Medium)
**Lives in:** `protocol/chapter1/1.2_multi_forest_overlays/1_graph_theory_and_slicing.md` §1.3 (*Deterministic Tree Assignment*, *Capacity-Proportional Multi-Tree Assignment*), §1.4 (*Shrinking*); `4_stream_slicing_architecture.md` §4.5; `5_node_classes.md` §5.3; `appendix_d_frame_registry.md` §D.4.8 (`EffectiveSegmentSeq`); `appendix_b_parameters.md`
**Class:** A rule sound at fixed $M$, catastrophic under the dynamic $M$ a later fix introduced (recurring pattern #2)

---

## The problem in one line

$a = (\text{Blake3}(NodeID) \bmod M) + 1$ reassigns $83\%$ of relays when $M$ goes $5 \to 6$ and $67\%$ at $2 \to 3$, so every rung of the forest ladder was a whole-swarm teardown — all relays warming at once, every peer re-joining every tree, and each relay free to accept $K_v$ children in *both* layouts during the 5 s window.

## The decision

*   **Rendezvous ranking.** $\text{rank}_v(m) = \text{Blake3}(NodeID_v \parallel \text{uint8}(m))$; a relay takes the top-ranked tree, and a multi-tree node the top-$t_v$. Growing $M \to M+1$ moves $\approx 1/(M+1)$ of relays; shrinking moves only the closed tree's relays. Everyone else keeps parent, children and tree.
*   **The window is a segment count, not a wall-clock hint.** `MANIFEST_UPDATE` carries `EffectiveSegmentSeq` $= S_{\text{now}} + 5$: the first segment emitted on the new layout. Every peer must hold a parent in each new tree by then; one that does not treats it as a failed round and follows the shed rule.
*   **Moving relays drain one-for-one.** A relay that changes tree keeps serving its old children for $\tau_{\text{drain}}$ (or until they re-attach) and admits new-tree children only as old ones release. Total children never exceed $\sum_m K_v(m)$; upload is never double-committed. Non-moving relays do nothing.
*   **Tree index and layer content are decoupled.** The source may remap which layer a tree index carries at a resize without moving any relay; a relay forwards whatever arrives on its tree.

## Why this and not the alternatives

*   **Consistent hashing on a ring** gives the same $1/(M+1)$ movement but needs virtual nodes for balance and a shared ring layout; rendezvous needs only NodeID and $M$, which every peer already has, and its multi-tree extension (top-$t$) is one line.
*   **Halving slots across both layouts during the window** ($\lceil K_v/2 \rceil$ old, $\lfloor K_v/2 \rfloor$ new) was the first draft of the window rule. It is unnecessary once only $1/(M+1)$ of relays move — for the rest there is no second layout — and it would have shed children from relays that had no reason to change anything.
*   **Keeping the modulus and adding a slower ladder** (bigger hysteresis, longer dwell) only bounds how *often* the rebuild happens, not what it costs — the issue's own observation.

## Defects found during verification

*   The old §4.5 step 2 had every peer "re-compute its tree assignment and join its new tree(s)" — but `tree_mapping` renumbering at a resize also changes each tree's *content* and bitrate, so even the design intent of "the same tree" was ambiguous. Stating that content follows the index and relays follow their rank resolves it.
*   The old shrink text asked relays to shed children "before the old layout drains" without defining when that was. `EffectiveSegmentSeq` is the definition.
*   Per-tree bitrate changes at a resize for non-moving relays too (Tree 3 carries $1.5$ Mbps at $M = 5$ and $0.75$ at $M = 6$), so $K_v(m)$ must be recomputed against the new matrix even by relays that do not move. This was invisible while $B_m = B/M$ was assumed uniform (SOLUTION-019).

## The generalisable lesson

**A deterministic assignment function must be checked for stability under every parameter it takes, not just for uniformity at fixed parameters.** The modulus is perfectly uniform and perfectly unstable; the property that mattered was the one nobody tested because $M$ was a constant when the rule was written.

## Residual risk

The new tree on growth is built from the source's slots plus $\approx N_{\text{relay}}/(M+1)$ relays that are simultaneously warming. For a large flash-crowd jump (say $M: 2 \to 6$ in one step, moving $\sim 2/3$ of relays across four new trees) the transition is still substantial; the ladder's "compute $M$ directly" rule makes this a single event rather than four, but Chapter 8 should measure the join storm.

## Validation owed (Chapter 8)

*   Fraction of relays moving and time-to-full-coverage of the new tree at each rung, with and without the flash-crowd pre-emption.
*   Peer-side join success rate within the 5-segment window at $N_{\text{relay}} = 12, 18, 24, 30$.
*   Whether 5 segments is the right window: shorter cuts the dual-layout period, longer tolerates slower joins.
