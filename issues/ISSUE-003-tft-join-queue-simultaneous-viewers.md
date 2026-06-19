# ISSUE-003: TFT Optimistic Unchoke Creates a Join Queue for Simultaneous Viewers

**Status:** Open  
**Priority:** Medium  
**Component:** Chapter 5 — Tit-for-Tat Incentive Layer  
**Affects:** Streams where multiple viewers join at the same time (stream launch, shared links, events)  
**File:** `protocol/chapter5/5.1_tit_for_tat/3_optimistic_exploration.md`, `2_sliding_window_unchoker.md`

---

## Summary

New viewers joining a stream have empty buffers and cannot reciprocate uploads immediately. The only mechanism to bootstrap them is the single Optimistic Unchoke slot — one random peer is unchoked every 2 seconds regardless of contribution. When multiple viewers join simultaneously (common at stream launch or when a link is shared), they queue for this one slot, each waiting 2N seconds where N is their position in the queue. Viewers joining at the same time may wait 4–10 seconds before receiving their first video data.

---

## Detailed Description

From `3_optimistic_exploration.md`:

> Every 2 seconds (4 TFT cycles), the node selects a **random** peer from its Passive Set or Active Set and unchokes it regardless of its current upload rate.

There is exactly **1 optimistic unchoke slot** per peer per 2-second window. The slot is awarded randomly, not in arrival order.

**Scenario: 5 viewers join at T=0**

All 5 are new, have empty buffers, and contribute 0 upload. The source's TFT unchoker sees 5 peers all at 0 contribution. It unchokes its top 4 contributors from regular slots (none qualify) and 1 from the optimistic slot.

| Time | Viewers with data | Viewers waiting |
|---|---|---|
| T=0s | 0 | 5 |
| T=2s | 1 (optimistic slot fires) | 4 |
| T=4s | 2 | 3 |
| T=6s | 3 | 2 |
| T=8s | 4 | 1 |
| T=10s | 5 | 0 |

The last viewer in the queue waits 10 seconds before seeing any video. Since the optimistic selection is random (not ordered), some viewers may wait longer than 10 seconds if they keep losing the random selection.

Additionally, once a viewer receives data via the optimistic slot and starts contributing back, TFT promotes them to a regular unchoke slot — freeing the optimistic slot for the next new joiner. But a new viewer needs at least one full TFT cycle (500ms) of data before they can upload back at a measurable rate. So the effective unlock cadence is roughly one new viewer every 2–3 seconds.

---

## Impact

- At coordinated stream launches (e.g. announced start time), many viewers arrive in a burst. All experience a staggered delay.
- Viewers waiting for the optimistic slot see a buffering spinner despite the stream being live and healthy.
- The random selection means some viewers may wait substantially longer than average.

---

## Proposed Fix

**Option A (preferred): Scale optimistic unchoke slots with the number of new joiners.**

Track new peers (those with zero or near-zero contribution score in the last 2 seconds). Allocate one optimistic slot per N new peers up to a cap:

```
optimistic_slots = min(ceil(new_joiners / 3), 4)
```

This allows up to 4 simultaneous new-joiner bootstraps, capped to prevent a flood of new peers from consuming all unchoke slots and starving established contributors.

**Option B: Priority queue instead of random selection.**

Replace random optimistic peer selection with FIFO based on join timestamp. The peer that has waited longest gets the next optimistic slot. This at least makes the wait time deterministic and bounded.

**Option C: Reduce the optimistic cycle from 2s to 500ms for the first 30 seconds of a stream.**

During the initial growth phase (detectable by DHT peer count < 20), fire the optimistic unchoke every 500ms instead of 2s. Once the swarm is established, revert to 2s. No slot count change needed.

Option A is the most robust. Option C is the lowest-effort quick fix.

---

## Effort

Low–Medium. Changes confined to the TFT unchoker loop. No wire format changes needed.
