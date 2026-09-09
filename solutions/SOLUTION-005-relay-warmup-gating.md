# SOLUTION-005: Relay Warm-Up Gating

**Closes:** ISSUE-005 (Low — but with a High-severity interaction)
**Lives in:** `protocol/chapter1/1.2_multi_forest_overlays/2_parent_selection_algorithm.md`, `protocol/chapter1/1.1_scale_latency/5_capacity_adaptation.md`, `appendix_d_frame_registry.md` §D.4.7
**Class:** Growth-phase transient

---

## The problem in one line

A relay advertises its full slot count the instant its tree join is accepted — but it has no data to forward until it has received and verified its first 1-second segment, so children route to it on a high capacity score and then receive nothing for a second, while it cheerfully answers keepalives so it is never evicted as dead.

## The decision

Advertise $K_{\text{avail}} = 0$ until the first segment is verified. The tree-join algorithm's existing `available_slots > 0` probe check then hides the relay from parent selection with no new mechanism.

## Why hiding beats the alternatives

The obvious alternatives make it worse. Letting children attach and absorb the gap via mesh PULL is what the unfixed spec did — and during a growth burst the peers they would PULL *from* are also warming, so the fallback fails exactly when it is needed. Advertising a reduced score rather than zero merely delays the same misrouting. Hiding is correct because the state is genuinely brief and self-resolving: costing a relay one second of invisibility is strictly cheaper than costing a child one second of starvation.

Cold start does not deadlock under this rule, which is worth checking explicitly: the source always has data, so the forest grows one generation per segment period — 8, 64, 512, … reaching a million within ~7 s at $K_v = 8$, which is the same geometric growth the depth proof of §1.1.3 assumes.

## The interaction that made a Low-priority issue serious

Rated Low in isolation — one second, absorbed by a four-second buffer. But it composes badly with the shed rule introduced separately by ISSUE-015, and the composition is not visible from either document alone.

The shed rule sheds an SVC layer after **2 consecutive failed tree-join rounds**, where a round fails when every candidate advertises no capacity. Warm-up gating makes warming relays advertise exactly that. So:

> A flash crowd causes many relays to warm up simultaneously → joiners see rounds where every candidate advertises $K_{\text{avail}} = 0$ → two such rounds → the joiner sheds a quality layer and enters a **10-second hysteresis** before retrying.

A one-second self-resolving transient is converted into ten seconds of unnecessary 480p, for the whole joining cohort, during the growth phase the protocol most wants to handle well. Two fixes that are individually correct produce a visible product regression when combined.

### Resolution: make the two zero states distinguishable

`K_avail = 0` was overloaded — it meant both "will have capacity in under a second" and "will not". Splitting it needs no new wire field, because `PROBE_RESPONSE` already carries `LiveEdgeSegmentSeq`, and the warm-up rule itself guarantees the two states differ in it:

| State | $K_{\text{avail}}$ | `LiveEdgeSegmentSeq` | Meaning |
| :--- | :---: | :---: | :--- |
| Warming | 0 | 0 | Accepted, no verified segment yet |
| Saturated | 0 | > 0 | Genuine shortage |

A round containing any warming candidate does not count toward the shed threshold, and the retry interval must be at least one segment period so no shed decision is ever taken on a sample shorter than a single warm-up window.

## Second correction: warm-up is per tree

The original pseudocode tested one global `verified_segment_buffer`. A multi-tree super node (SOLUTION-007) can hold verified segments for $T_1$ while still warming in $T_4$ — under a global test, one warming assignment hides its entire capacity from the whole forest, which is the opposite of what multi-tree assignment exists to achieve. Warm-up is now evaluated per assigned tree.

## The generalisable lesson

**A sentinel value that encodes "cannot serve you" must distinguish transient from persistent causes, or every consumer of it will conflate the two.** $K_{\text{avail}} = 0$ was read by parent selection (which only needed "not now") and by the shed rule (which needed "not ever") — and only the second reading was wrong. Worth auditing wherever else the spec uses a zero or absent value as a signal.

## Validation owed (Chapter 8)

* Spurious shed rate during a flash crowd, with and without the warm-up exclusion — this is the metric that justifies the added complexity.
* Whether one segment period is the right warm-up threshold, or whether a relay could safely forward a *partially* received segment's verified blocks and shorten the window.
