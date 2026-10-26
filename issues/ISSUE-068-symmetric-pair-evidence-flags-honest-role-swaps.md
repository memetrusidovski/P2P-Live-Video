# ISSUE-068: SYMMETRIC_TREE_PAIR evidence flags honest parent-child swaps

**Status:** OPEN
**Filed:** 2026-10-20 (surfaced by the reference implementation, `src/`, while implementing reputation auditing)
**Spec sections:** Ch5 §5.3.1 (Symmetric Cycle Spotting), Ch5 §5.3.3 (evidence types), App D §D.4.18

## Problem

Ch5 §5.3.3 defines `SYMMETRIC_TREE_PAIR` evidence as two receipts, one each way, in the same
tree, with timestamps within 60 s and byte values within 10 %. Ch5 §5.3.1 names the one honest
topology that passes this test, a node evicted and re-joined under its former child inside the
window. It leaves that case to the ratio test.

Under the upward migration of Ch1 §1.2.3 and the rank preemption of Ch1 §1.2.2 that topology is
common, not rare. A child migrates above its parent; the parent is preempted and re-joins under
the child. Each direction then carries whole-stripe receipts of equal size, so the ratio test
passes too. In the simulator (56 nodes, 6 colluders, 240 s) the rule as written produced 108
honest-on-honest accusations and 104 honest consensus evictions in one run. Three ranked
accusers from three prefixes are reached because every top relay audits the same proofs.

## Options

1. **Same segment and one common block.** Evidence is two receipts for the same segment of the
   same tree whose bitmaps claim at least one block in common. A tree edge delivers a block once,
   parent to child, so no honest pair can both hold a receipt for the same block from each
   other. The rule is verifiable from the two receipts alone and has no honest false positive.
   It detects fewer forged pairs per sampled proof than the 60 s rule, which the auditor makes up
   for by keeping samples for τ_accuse.
2. **Interleaving.** Keep the 60 s window but require the auditor to hold flow in both directions
   that overlaps in time (A before and after B in the same tree). Needs three sampled receipts
   and is not verifiable from the two-receipt evidence a receiver re-checks.
3. **Keep the rule and rely on the three-accuser threshold.** Rejected: the threshold guards
   against amplification of true accusations, and these are true accusations of honest nodes.

## Implementation decision (pending spec resolution)

Option 1: `Node::symmetric_pair` requires the same segment and intersecting bitmaps on top of
the spec's tree, timestamp and byte checks. Accusers and receivers apply the same test. The
collusion gate passes on four seeds with zero honest evictions.
