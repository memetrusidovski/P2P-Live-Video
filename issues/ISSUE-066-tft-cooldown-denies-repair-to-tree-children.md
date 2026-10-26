# ISSUE-066: The Tit-for-Tat cooldown denies repair service to tree children that cannot reciprocate

**Status:** OPEN
**Filed:** 2026-09-16 (surfaced by the reference implementation, `src/`, core test `missing_block_is_pulled_before_the_deadline`)
**Spec sections:** Ch5 §5.1.2 (what the unchoker governs), Ch5 §5.1.3 (optimistic exploration, FIFO rules), Ch4 §4.3 (PULL zone repair)

## Problem

Ch5 §5.1.2 says PULL service, including *repair of lost blocks*, goes only to unchoked peers, and that "a
leaf … relies on optimistic slots for repair". Ch5 §5.1.3 then says a peer that consumes an optimistic
slot and fails to reciprocate above `Threshold_min` within 2 s "goes to the tail with a 30 s cooldown
before it is eligible again", keyed on NodeID and `/24`.

Put together, a child that receives a pushed stripe from its parent, loses a block on that link, and
has nothing to give back (a leaf, or a relay whose neighbours simply never lose blocks) cannot repair
that block from its parent for up to 30 s after any optimistic window it did not "pass". In the two-node
case (one viewer under the publisher) it can never repair at all after its first backfill window. The
PULL zone is 1.5–2 s wide (Ch4 §4.3.1), so the block starves.

The parent is the natural repair source for its own stripe: it holds the block, the loss was on its
link, and the child's `PROOF_OF_UPLOAD` for that tree already credits the parent for the pushed bytes.
Refusing that request punishes the child for the link's loss, not for free-riding.

## Options

1. **Exempt a child's repair of the tree its parent pushes to it** from the choke decision (still
   inside the PULL budget). Non-children and backfill remain governed by Tit-for-Tat. Free riders are
   still removed by the receipt deadline, which is the mechanism the spec already assigns to them.
2. **Scope the cooldown to the bootstrap queue** (its stated purpose is to stop head-of-queue cycling)
   and let an idle slot serve any waiting peer.
3. **Fold repair into the push path** (parent re-sends symbols on a NACK) and reserve PULL for
   backfill and cross-tree repair only. A larger change to Ch4.

## Implementation decision (pending spec resolution)

Option 1: `Node::pull_allowed` serves a PULL_REQUEST from a current child when the requested block
belongs to a tree in which the requester is our child. Everything else follows Ch5 §5.1 as written,
with one reading choice documented in `src/ARCHITECTURE.md`: a waiting peer takes a *free* optimistic
slot immediately rather than at the next 2 s boundary (the budget is unchanged).
