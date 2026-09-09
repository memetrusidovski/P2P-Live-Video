# SOLUTION-031: Leaf-Class Peers Are Tree Children; the Live-Edge Offset Is Removed

**Closes:** ISSUE-031 (Medium)
**Lives in:** `protocol/chapter1/1.2_multi_forest_overlays/5_node_classes.md` §5.1, §5.4; `protocol/chapter4/4.3_hybrid_push_pull/1_buffer_sliding_timeline.md` (*One Anchor for Every Class*); `protocol/chapter7/7.3_privacy_routing/2_onion_latency_tradeoffs.md`; `appendix_b_parameters.md`
**Class:** Two delivery paths described, neither chosen

---

## The problem in one line

`LEAF` was defined both as "a leaf in all $M$ trees" — a push child at the live edge — and as joining "5–10 s behind the live edge, drawing from older well-replicated segments" — a pull consumer of retention — and the spec never said which; the second reading would have routed base-layer delivery for up to half the swarm through a repair path sized for loss.

## The decision

**Leaves are push children.** They receive the base layer down Tree 1 at the live edge, anchor at $X_{\text{edge}}$ with the same $\Delta_{\text{buffer}}$ as everyone, and the 5–10 s offset is deleted along with the retention window it inflated. The price of leaf class is restated as what the protocol actually enforces: a quality ceiling (enhancement layers only from surplus), **lowest preemption rank** in contended enhancement trees (Ch5, with ISSUE-023), and no Deputy or relay role. Floor accounting stays in tree-slot units, which is now unambiguous.

## Why this and not the alternatives

*   **Pull-served leaves from retention** would need: a PULL rate of $\approx 11$ blocks/s per leaf for the base layer alone, responder TFT that sees zero reciprocity (so only the 20% floor serves them), 1 s Bloom-filter bitfield gossip from 8 neighbours as the availability signal, and `BLOCK_TRANSMISSION` with inline proofs per block. That is a second primary delivery mechanism, unspecified, carrying up to 50% of the swarm on a path designed for the odd lost block. Rejected.
*   **Keeping a small offset (1–2 s) for leaves as a "deprioritisation"** buys nothing on the push path — a parent pushes the current chunk to every child at once — and complicates the anchor rule for no gain.

## Defects found during verification

*   The 14 s retention window (SOLUTION-004) was derived entirely from the leaf offset; with the offset gone and $\Delta_{\text{buffer}} = 3$ s it is 8 s.
*   "Relieves flash-crowd pressure" was the offset's second justification. On the push path it does the opposite of nothing: a leaf still needs a Tree-1 slot at the moment it joins. Flash-crowd relief comes from the service floor cap and the source reserve, not from delay.
*   Ch7 §7.3.2 restated the offset for `LEAF_PRIVATE`; it is corrected there too.

## The generalisable lesson

**A class's "price" must be a rule some component enforces, not a description of behaviour nobody implements.** The offset sounded like a cost and was a dangling reference.

## Residual risk

Without the offset, leaves compete for Tree-1 slots at the live edge exactly like relays. The service floor cap (20%) and the source reserve bound this; the leaf-fraction bound of Ch1 §1.2.5 ($\ell \le 51.6\%$) is unchanged because it was already computed on base-layer demand, not on the offset.

## Validation owed (Chapter 8)

*   Base-layer PSR for leaf-class peers under the floor cap at $\ell \in \{0.3, 0.5, 0.6\}$.
