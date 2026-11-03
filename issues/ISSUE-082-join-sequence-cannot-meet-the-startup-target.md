# ISSUE-082: The specified join sequence cannot render inside the 1.5 s startup target: the pre-media path alone is 1.14 s and β_pull-bounded backfill cannot fill the first renderable segment before its deadline

**Status:** Open
**Priority:** High
**Component:** Ch4 §4.3.1 late-joiner sync steps 2 and 5; Ch5 §5.1.3 optimistic slots; Ch8 §8.2 SJL
**Affects:** Every join at $N \ge 10^4$; the README's "first frame within 1.5 s" claim
**File:** `protocol/chapter4/4.3_hybrid_push_pull/1_buffer_sliding_timeline.md`, `protocol/chapter5/5.1_tit_for_tat/3_optimistic_exploration.md`, `protocol/chapter8_simulation/README.md`

---

## Summary

Ch8 §8.2 sets SJL $\le 1.5$ s. The join path as specified (discovery, HyParView join, probe and handshake) takes about 1.14 s before any media can move, the playout deadline is 3.0 s behind the live edge (Ch4 §4.3.1 step 2), and the first segment a joiner can render has to be backfilled by PULL through optimistic slots that each neighbour opens one per 2 s window at $\beta_{\text{pull}} = 0.5$ Mbps. Under those rules the first frame renders between 3.3 and 4.3 s after pressing play. The reference implementation's own gate uses 3.6 s mean because of this.

## Detailed Description

1.  Discovery 0.58 s + `JOIN` 0.32 s + probe and handshake 0.24 s = 1.14 s at RTT 80 ms, $N = 10^4$.
2.  Backfill of segment $X{-}2$ ($L_0$ alone, 1.5 Mbit) needs 3 simultaneous slots; 8 neighbours open one optimistic slot each per 2 s at unsynchronised phase, so 4 are expected within 1 s: 0.75 s once slots open. A full 6 Mbit segment at 0.5 Mbps per slot needs 12 slots where a joiner has at most 8 neighbours.
3.  Segment $X{-}2$ misses its deadline; $X{-}1$ renders at 3.3–4.3 s. The most lenient reading (render the first complete $L_0$ segment inside the one-segment drift tolerance) gives 2.2 s.
4.  The simulator gate (`src/fleet/report.py`, `--sjl-max-ms`) is set to 4 s with the comment that join latency is buffer-bound until backfill is tuned.

## Impact

A headline requirement is unmet by the mechanism that is supposed to deliver it, and no spec text says so. Either the target or the join sequence has to change.

## Proposed Fix

*   Either restate SJL as time to the first verified block and set the claim at about 3.3 s, or allow playout at the first complete $L_0$ segment and converge on the 3.0 s offset by playing slightly slow.
*   Exempt late-join backfill from $\beta_{\text{pull}}$ and the optimistic-slot cadence (a bounded burst per joiner, charged to the PULL reserve).
*   Touches Ch4 §4.3.1, Ch5 §5.1.3, Ch8 §8.2, README design goals.

## Effort

Medium.

Found by the flash-crowd walk (`audits/traces/flash-crowd.md`); the cold-start, growth and DHT walks reached the same verdict on R2. Cluster with ISSUE-081.
