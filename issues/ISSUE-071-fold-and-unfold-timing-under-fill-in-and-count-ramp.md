# ISSUE-071: Fold and unfold timing is undefined across the fill-in and the relay-count ramp

**Status:** OPEN
**Filed:** 2026-10-22 (surfaced by the reference implementation, `src/`, in the layer-fold and forest-resize gates)
**Spec sections:** Ch1 §1.1.5 §5.6 (fold rule, unfold), Ch1 §1.2.4 §4.5 (grace rule), Ch1 §1.2.1 §1.4

## Problem

Three timings the spec leaves open each produced a wrong matrix change in simulation.

1. **Starvation measured across a switch.** The fold rule averages the starved share over
   τ_fold. Samples taken before a switch describe the previous layout, and the fill-in after
   a switch produces starved re-queries for several seconds. Measured naively the source
   folded a second layer 30 s after the first fold, while the first fold was still filling in.
2. **Unfold against a ramping count.** The unfold rule restores a layer when the relay count
   grew by 50 % since the fold. Right after a stream starts the guardians' counts ramp from
   zero for tens of seconds. A fold taken during the ramp is followed by an unfold that
   re-creates the starvation, then another fold.
3. **The grace window is shorter than the fill-in.** Failed rounds stop counting toward a
   shed at `EffectiveSegmentSeq + D_max` segments. In a 35-relay swarm the fill-in of a
   changed tree took 8 to 19 s past the switch. Viewers shed the top layer in every resize
   and climbed back later.

## Options

1. **Measure from the fill-in, dwell from the start, grace for every tree.** Starvation
   samples count only from `EffectiveSegmentSeq + D_max` on, and the history is cleared at
   each switch. The initial matrix counts as the first change, so no fold is taken within
   τ_forest of the stream start. The grace applies to every tree at a resize, since relays
   move in all of them.
2. **Unfold against a stable baseline.** Take the baseline as the relay count at the fold
   or the highest count seen in the τ_fold before it, whichever is larger.
3. **Lengthen the grace window** to a multiple of D_max, or until the tree's first block.

## Implementation decision (pending spec resolution)

Option 1 for the fold measurement, the dwell and the grace. The unfold baseline is still the
count at the fold; with the dwell from the start the count has converged by the first fold in
every gate.
