# ISSUE-089: Rendezvous assignment is a fair coin per relay, so one swarm in ten below about 50 relays has a tree with too few relays to serve every viewer, and nothing rebalances it

**Status:** Open
**Priority:** High
**Component:** Ch1 §1.2.1 §1.3 Rendezvous assignment; Ch1 §1.2.1 second-ranked fallback; Ch1 §1.2.4 forest ladder
**Affects:** Swarms of tens of relays at any latency; the first minutes of every stream
**File:** `protocol/chapter1/1.2_multi_forest_overlays/1_graph_theory_and_slicing.md`, `protocol/chapter1/1.2_multi_forest_overlays/2_parent_selection_algorithm.md`

---

## Summary

Each relay takes the tree its NodeID ranks highest. The spec says this "distributes relay burden uniformly over the $M$ trees, with approximately $N_{\text{relay}}/M$ relays per tree". That holds in expectation only. The count per tree is binomial, and at 45 relays the spread is wide enough to leave one tree below the slots its viewers need. The second-ranked fallback fires only when discovery returns **no** relay of the tree, so a tree with a few relays and too few slots gets no help.

## Detailed Description

1.  With $N_{\text{relay}} = 45$ and $M = 3$, a given tree has 9 or fewer relays with probability $3.7\%$ and 8 or fewer with $1.6\%$. Over three trees, about one swarm in ten has a tree at 60 % of the mean or less.
2.  Measured with `scenarios/internet_rtt_step.toml` (45 relays at 8 to 20 Mbps, 15 leaves, 60 viewers, source 50 Mbps):

    | Seed | Relays per tree | Slots per tree (relays) | Source slots | Join mean | Verdict |
    |---|---|---|---|---|---|
    | 21 | 18 / 11 / 16 | 127 / 73 / 47 | 8 / 8 / 4 | 2.8 s | pass |
    | 4 | 16 / 16 / 13 | 101 / 109 / 38 | 8 / 8 / 4 | 2.9 s | pass |
    | 25 | 9 / 21 / 15 | 57 / 136 / 39 | 8 / 8 / 4 | 5.2 s | fail |
    | 27 | 8 / 19 / 18 | 50 / 115 / 61 | 8 / 8 / 4 | 7.3 s | fail |

    Tree 1 carries the base layer, so all 60 viewers need a slot in it. Seed 27 offers $50 + 8 = 58$.
3.  The failure does not depend on latency: seed 27 fails the same way at 20 to 40 ms and at 200 to 300 ms (a third of the joins take over 15 s, and viewers that never get a base-layer parent starve for the whole run).
4.  The shortage feeds itself. A relay assigned to the short tree needs a parent in that tree before it can serve, and it competes for the source's 8 slots with every viewer.
5.  Tree 3 (the 3 Mbps stripe) is short in every seed, 38 to 61 slots for 45 relays, because a relay holds half as many slots on a stripe twice as wide. The layer is shed there, which the spec intends. The base layer has no such exit.

## Impact

A small or young swarm has a one in ten chance that a tenth of its audience cannot get the base layer, however much total upload the swarm holds. The simulator gates hide it: each runs one seed, and `baseline_100` has 70 relays.

## Proposed Fix

*   Let a relay take its second-ranked tree when its first-ranked tree is over-served and the second is short, judged from the per-tree relay counts the guardians already return (Ch2 §2.3.2). Keep the assignment sticky so the forest does not oscillate.
*   Or weight the base-layer tree in the rendezvous rank until it holds $\lceil N / K_{\text{mean}} \rceil$ slots.
*   Widen the second-ranked fallback of §1.3 from "no relay of the tree" to "no relay of the tree with a free slot".
*   Touches Ch1 §1.2.1 §1.3, the fallback rule, and the admission check of App D that verifies an assignment against the rendezvous rule.

## Effort

Medium. Found while building the Internet simulator scenarios on 2026-11-11 (`ROADMAP.md`, Round 2). Related: ISSUE-083 (admission in a full tree).
