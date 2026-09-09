# SOLUTION-049: The Service Floor Is a Leaf Share Enforced by Displacement

**Closes:** ISSUE-049 (Medium)
**Lives in:** `protocol/chapter1/1.2_multi_forest_overlays/2_parent_selection_algorithm.md` (*Rank Admission Rule* items 1, 3, 4); `5_node_classes.md` §5.5; `protocol/chapter1/1.1_scale_latency/5_capacity_adaptation.md` §5.5; `protocol/chapter5/5.1_tit_for_tat/3_optimistic_exploration.md`; `appendix_b_parameters.md` (Leaf share); `appendix_d_frame_registry.md` (`DISCONNECT`/`DRAIN_NOTICE` reason `DISPLACED`)
**Class:** One rule written two incompatible ways in four places, with its rounding undefined (recurring pattern #1)

---

## The problem in one line

Three sections said a relay must "keep 20% of its $L_0$ slots available to leaves" and two said leaves beyond a pending request "wait for surplus"; one reading idles a fifth of every base-layer relay in a swarm with no phones and forbids a one-slot relay any relay child, the other is a floor a relay filled by relays never honours — and 20% of one to four slots was never rounded.

## The decision

The floor is a **leaf share** $R_{\text{leaf}}(m) = \lceil 0.2\,K_v(m) \rceil$ — at least one slot at every $K_v(m) \ge 1$ — enforced by **displacement**: while leaves hold fewer than the share of a relay's $L_0$-tree slots and none is free, a leaf-class request displaces the relay's lowest-ranked relay-class child *that does not relay that tree* (a pure subscriber, out-degree 0 there), through the drain path with reason `DISPLACED`. A child that relays the tree is never displaced. Beyond the share, leaves wait for a free slot. No slot is ever held idle. The requester's class is read from `NEIGHBOR`, which now carries it (SOLUTION-047).

## Why this and not the alternatives

*   **Hard idle reservation** guarantees the floor but at cold start — $M = 2$, relays holding one or two slots each — reserves half or all of a relay's capacity for phones that may never arrive, and the source pays the difference.
*   **Pending-priority only** never displaces a sitting relay, so once relays fill a parent the floor is unenforced; "guarantee" would have to be struck from §1.1.5 and §1.2.5.
*   **Displace any lowest-ranked child, including relays of the tree.** A relay of $T_m$ has children in $T_m$; displacing it orphans a subtree to seat one phone. The exclusion is what keeps the exception bounded to nodes whose departure costs nobody but themselves.
*   **Hard reservation only above $K_v(m) \ge 5$, zero below** (the issue's first proposal): leaves the cold-start relays with no floor at all, which is exactly where the source reserve is thinnest.

## Defects found during verification

*   $20\%$ of $K_v(m) \in \{1,2,3,4\}$ is $0.2$–$0.8$ slots: floor gives no floor for every relay under $\approx 5$ Mbps on a $0.75$ Mbps stripe, ceiling gives a one-slot relay no relay child. The ceiling is kept because displacement makes it costless when no leaf asks.
*   The old rule read the requester's class at `NEIGHBOR` time from a frame that carried no class (ISSUE-047); the fix needs SOLUTION-047.
*   Displacement composes with the handover budget of SOLUTION-045: it is a handover like a preemption, one per tree at a time, sequential when the reserve cannot overlap.

## The generalisable lesson

**When a reservation would idle capacity, ask whether it can be enforced at the moment of demand instead — by a bounded displacement whose victim is chosen so that nobody downstream pays.** A share that is held empty costs everyone always; a share claimed on request costs one peer once.

## Residual risk

*   A leaf-class Sybil flood can displace up to $R_{\text{leaf}}(m)$ relay children per parent per tree; bounded by the prefix cap on children (SOLUTION-042) and by one displacement in progress per tree.
*   The displaced relay loses $L_0$ playback until it re-attaches; with the drain notice that is one warm repair, but in a swarm with no other $L_0$ slot it falls to the source reserve.

## Validation owed (Chapter 8)

*   Displacement rate and displaced-peer PSR at $\ell \in \{0.1, 0.25, 0.4\}$.
*   Whether $20\%$ is the right share once the true leaf-fraction bound (SOLUTION-048) is known.
