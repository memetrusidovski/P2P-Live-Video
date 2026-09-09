# SOLUTION-044: `PROBE_RESPONSE` Carries a Per-Tree State

**Closes:** ISSUE-044 (Medium); completes the per-tree warm-up of SOLUTION-005
**Lives in:** `appendix_d_frame_registry.md` §D.4.7 (`TreeState`, per-tree `LiveEdgeSegmentSeq`), §D.4.8 (segments start at 1); `protocol/chapter1/1.2_multi_forest_overlays/2_parent_selection_algorithm.md` (state table); `protocol/chapter1/1.1_scale_latency/5_capacity_adaptation.md` §5.3 item 2; `appendix_b_parameters.md`
**Class:** A node-level field read as if it were per tree (recurring pattern #6, sentinel conflation)

---

## The problem in one line

The warming/saturated distinction that keeps the shed rule off growth transients was keyed on `LiveEdgeSegmentSeq = 0`, a node-level value; a relay warming in the probed tree while serving any other tree returned a non-zero value and was read as saturated — a super node earning a tree, a coverage-grant acceptor, every relay moving at a resize.

## The decision

`PROBE_RESPONSE.Flags` bits 5–6 carry **`TreeState`** for the probed tree: `SERVING` (parent and $\ge 1$ verified segment there), `WARMING` (parent, no verified segment yet), `UNPARENTED` (assigned, no parent — SOLUTION-040). `LiveEdgeSegmentSeq` is redefined as the highest segment verified **in the probed tree**. Only `WARMING` exempts a round from the shed count; `UNPARENTED` counts, since the relay is a victim of the same shortage. `SegmentSeq` starts at $1$, so zero is unambiguous.

## Why this and not the alternatives

*   **Redefine `LiveEdgeSegmentSeq` alone** (the issue's first option) distinguishes warming from serving but cannot express *unparented*, which SOLUTION-040 introduces and which a joiner must not mistake for warming.
*   **A separate `WARMING` bit only** leaves the per-tree live edge undefined and keeps a field in the frame whose meaning depends on which section the reader trusts.

## Defects found during verification

*   All four cases the issue listed reduce to the same wrong branch of the §1.2.2 table; two of them (movers at a resize, coverage-grant acceptors) are exactly the transients SOLUTION-005 was written to protect.
*   Segment numbering had never been given a start; the Ch4 text spoke of "segment 0" only as the thing a late joiner must not start at. With segments from $1$, the zero sentinel in `PROBE_RESPONSE` and the zero `EffectiveSegmentSeq` in the Stream Record (SOLUTION-054) are both well defined.

## The generalisable lesson

**When a section says a property is per tree, every field that carries the property must be per tree in the byte diagram.** SOLUTION-005 fixed the pseudocode and left the frame node-level.

## Residual risk

None beyond the two reserved bits now spent.

## Validation owed (Chapter 8)

*   Spurious shed rate during a resize and a coverage grant, before and after — the metric SOLUTION-005 already owed and could not have passed.
