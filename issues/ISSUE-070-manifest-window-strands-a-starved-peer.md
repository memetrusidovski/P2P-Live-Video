# ISSUE-070: The manifest sequence window strands a peer that was starved

**Status:** OPEN
**Filed:** 2026-10-22 (surfaced by the reference implementation, `src/`, in the layer-fold gate)
**Spec sections:** Ch7 §7.1.2 (replay prevention), Ch4 §4.3.1

## Problem

Ch7 §7.1.2 accepts a manifest only within `X_edge + LOOKAHEAD` segments of the live edge
the peer has itself observed. The edge moves only on the push path. A peer that held no
parent for a while, for example one starved of a base-layer slot before a fold, has an edge
tens of segments old. When it attaches again every current manifest is above the window and
is dropped as out of window. The peer never re-anchors, never plays, and its rounds keep
failing. In the layer-fold gate half the swarm stayed in that state after the fold that was
meant to rescue it.

## Options

1. **Grow the window with the age of the edge.** Allow `LOOKAHEAD` plus the segments that
   elapsed since the edge last moved. The source can honestly be that far ahead of a stale
   edge, and a replay still cannot reach beyond the live edge.
2. **Re-anchor from the push path.** A manifest from a current parent is accepted whatever
   the window says. Loses the window's protection against a parent that replays ahead.
3. **Reset the edge on parent loss.** Treat a peer with no parent in any tree as a fresh
   joiner. Does not cover a peer that keeps one slow tree.

## Implementation decision (pending spec resolution)

Option 1: `SwarmBuffer::accept_manifest` widens the window ahead by the whole segments
elapsed since the edge last advanced.
