# ISSUE-063: The WARMING exemption from the shed rule is unbounded and livelocks joiners at capacity

**Status:** OPEN
**Filed:** 2026-09-15 (surfaced by the reference implementation, `src/`, lossy-links simulator gate)
**Spec sections:** Ch1 §1.2.2 (parent selection, `TreeState` table), Ch1 §1.1.5 §5.3 (shed rule), App D §D.4.7

## Problem

Ch1 §1.2.2 says a join round that sees a `WARMING` candidate "does **not** count" toward the shed
threshold because "capacity is imminent". The exemption has no bound. In a swarm whose top tree is
within a few slots of capacity, the tree churns continuously: relays displace pure subscribers
(SOLUTION-061), drained children re-attach, and each new attachment is `WARMING` for roughly one
segment. At any instant some candidate in a joiner's sample is warming, so **every** round is exempt,
the shed counter never advances, and the joiner probes twice a second indefinitely.

Observed in `scenarios/lossy_links.toml` (60 relays, 1.5/1.5/3 Mbps ladder, tree 3 at 65 slots
for 60 viewers): eight nodes probed tree 3 for 20–24 s with `failed_rounds = 0` throughout,
although every round also saw saturated candidates. They stayed in `CONNECTING`, and (before the
implementation stopped it) pulled the entire layer-2 stripe from the publisher meanwhile.

The "imminent" premise fails for a second reason: the warming relay's slots, once it warms up, are
contested by every other joiner in the same shortage, so seeing a warming candidate is not
evidence that *this* joiner will get a slot.

## Options

1. **Bound the exemption.** Exempt at most `N_warm` consecutive rounds (implementation uses 4,
   about 2–3 s at the 250–500 ms round cadence); after that a warming candidate counts like a
   saturated one. Simple, local, no new wire state.
2. **Exempt only if a *specific* warming candidate is followed.** The joiner remembers the
   warming candidate and re-probes only it; the exemption lasts until that candidate reports
   `SERVING` (then join or count saturation) or `UNPARENTED` (count). Tighter but adds state and
   a probe target that may vanish.
3. **Time-bound by the warm-up itself.** Warm-up is "~1 second of buffering" (§1.2.2 Warm-Up
   Gating); exempt rounds for at most 2 × segment duration after the first warming sighting.

## Implementation decision (pending spec resolution)

Option 1 with `warming_exempt_rounds = 4` (`Params`). A saturated-only round still counts as
before; a round exempted by warming resets the backoff to the base cadence as the spec intends.
The counter of consecutive exempt rounds resets on attach or on leaving the tree.
