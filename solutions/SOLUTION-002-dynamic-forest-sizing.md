# SOLUTION-002: Dynamic Forest Sizing (the M Ladder)

**Closes:** ISSUE-002 (High)
**Lives in:** `protocol/chapter1/1.2_multi_forest_overlays/1_graph_theory_and_slicing.md` §1.4, `4_stream_slicing_architecture.md` §4.5, `appendix_b_parameters.md`, `appendix_d_frame_registry.md` §D.4.8
**Class:** Cold-start economics

---

## The problem in one line

$M = 6$ was a constant, but a peer relays only the one tree its NodeID hashes to — so all six trees only get relay coverage around $N \approx 18$ (a birthday-problem effect), and until then the source pushes every slice to every viewer. The protocol was a CDN precisely in the range where the streamer's upload budget is tightest.

## The decision

$M$ is a **function of swarm size**, published by the source in the manifest and the DHT Stream Record:

| $N$ | $< 6$ | $6$–$11$ | $12$–$17$ | $18$–$23$ | $24$–$29$ | $\ge 30$ |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: |
| $M$ | 1 | 2 | 3 | 4 | 5 | 6 |

Transitions are announced with a signed, version-monotonic `MANIFEST_UPDATE` (0x17) and a 5-second window during which both layouts stay live, so playback never gaps.

## Why the ladder is spaced at 6

Each step is sized so that the trees the step *creates* are statistically covered the moment they exist. Adding a tree divides the relay population by one more bucket; with $N$ peers hashing uniformly into $M$ buckets, the probability that some bucket is empty stays negligible while $N/M \ge 5$. The ladder holds $N/M \in [5, 6)$ at every step — which is why the thresholds are multiples of 6 rather than of $M_{\text{max}}$.

Going the other way is the failure the issue described: $M = 6$ at $N = 6$ gives $N/M = 1$, where the expected number of empty trees is $6(1 - 1/6)^6 \approx 2.0$. Two of six slices with no relay at all.

## The two guards added on top (and why the fix was incomplete without them)

The ladder as originally applied was a bare threshold table. Two things were missing, both of which bite only in deployment, not on paper:

**1. Hysteresis + minimum dwell.** A bare table flaps. A swarm oscillating around $N = 30$ recrosses the $M{=}5 \leftrightarrow 6$ boundary indefinitely, and every crossing costs a 5-second window carrying *both* forest layouts — roughly double the tree state, repeatedly. Resolution: shrink thresholds sit 3 peers below the grow thresholds (half a ladder step), and $\tau_{\text{forest}} = 30\text{ s}$ bounds the rate of change outright.

The exception carved out matters as much as the rule: **upward** transitions may pre-empt the dwell timer when $N$ has passed twice the next threshold. In a flash crowd the source is the bottleneck, so waiting 30 s is the more expensive mistake. Downward transitions never pre-empt — there is no emergency that a *smaller* forest resolves.

This mirrors the shed hysteresis of Ch1 §1.1.5 deliberately. Both are the same lesson: in a swarm, any threshold that many peers evaluate against the same shared input will be crossed by all of them simultaneously, so every such threshold needs a cost-of-change term.

**2. Shrink semantics.** The original text said "typically growing it as viewers arrive" and left shrink undefined. Shrink is not symmetric with growth, for one non-obvious reason: $B_m = B/M$ **rises** as $M$ falls, so every relay's slot count $K_v = \lfloor u_v/B_m \rfloor$ *drops* at the transition. A 10 Mbps node holds 10 slots at $M{=}6$ and 3 at $M{=}2$. Without a rule, a shrink mass-orphans children. Resolution: relays shed down to the new $K_v$ *before* the old layout drains, releasing children through sibling election (§3), identically to a `RELAY`→`LEAF` downgrade.

## The invariant that keeps all of this consistent

**Peers derive their own relay assignment; they never derive the layer mapping.**

A peer computes only $a = (\text{Blake3}(NodeID) \bmod M) + 1$. What tree $T_m$ *contains* is read from the signed `tree_mapping` in the manifest. This one-line rule is what makes resizing safe in both directions: two peers can briefly disagree about $N$, or about which tree they should relay, and recover — but they can never disagree about what is inside a tree, because nobody computes that locally.

## Known gap this fix exposes

The ladder runs to $M = 6$; the reference SVC ladder has 3 layers. How trees map to layers at $M \ne 3$ — and therefore what the shed order of Ch1 §1.1.5 §5.3 means at $M = 6$ — is not specified. Filed as **ISSUE-019**. The frame carries the mapping (D.4.8), so this is a gap in the *source's policy* for constructing it, not in the wire format.

## Validation owed (Chapter 8)

* Source egress vs. $N$ across the ladder, confirming the CDN regime actually ends where the ladder claims.
* Flash-crowd transition: does the dwell pre-emption rule keep source egress bounded from $N = 2 \to 5{,}000$?
* Flap cost: measure the doubled-tree-state overhead of a transition, to confirm the 3-peer / 30-second guards are sized right rather than merely present.
