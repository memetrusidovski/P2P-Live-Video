# SOLUTION-021: Depth Rides on Every Block

**Closes:** ISSUE-021 (High)
**Lives in:** `protocol/chapter1/1.2_multi_forest_overlays/2_parent_selection_algorithm.md` (*Depth Propagation*, §2.3); `appendix_d_frame_registry.md` §D.4.9 (`BLOCK_PROOF.SenderHopDepth`); `appendix_b_parameters.md` (HysteresisMargin)
**Class:** A value learned once and consumed forever (recurring pattern #6)

---

## The problem in one line

A node learned its depth once, as `ACCEPTED.HopDepth + 1`, while three mechanisms — upward migration, Deputy re-attachment, forest resize — moved whole subtrees after join and no frame told the descendants; so the largest term of the parent score and the $D_{\max}$ admission rule ran on data that was wrong for most of the forest most of the time.

## The decision

*   **`BLOCK_PROOF` carries `SenderHopDepth`** in its formerly reserved byte. A node sets $h(m) = \text{SenderHopDepth} + 1$ on every block it receives in tree $m$ and advertises that everywhere. Zero bytes added; the frame already goes from every parent to every child several times per second.
*   **A node whose depth reaches $D_{\max}$ stops accepting and drains its children** through $\tau_{\text{drain}}$, and migrates immediately instead of at the next 5 s tick. Being deep is not an error to punish; leaving the branch there would be.
*   **`HysteresisMargin` = 30 ms**: above RTT jitter, below the $\approx 55$ ms score swing of one hop at $h \le 3$, so a genuine one-hop improvement always wins and a jitter flip never does.

## Why this and not the alternatives

*   **A dedicated `DEPTH_UPDATE` control frame** emitted on change costs a frame type and a fan-out storm at every migration (a node at depth 2 with a 10,000-child subtree would generate 10,000 frames per level). Piggybacking on a frame that already flows at the block rate costs nothing and converges in one block interval per level.
*   **Carrying depth in `MANIFEST`** forwarded down the tree: the manifest is source-signed, so a per-hop field would have to sit outside the signature and the frame would need a mutable envelope. `BLOCK_PROOF` is already per hop and unsigned.
*   **Depth in the `ROSTER`**: once per second, and only to children that are relays of this tree. Too slow and too narrow.

## Defects found during verification

*   Convergence: on the slowest reference tree ($0.75$ Mbps, one block per $\approx 170$ ms) a depth change at the top reaches depth 8 in $\approx 1.4$ s. `PROBE_RESPONSE.HopCount` can therefore be up to $\approx 1.5$ s stale after a migration — bounded and small, where before it was unbounded.
*   `HysteresisMargin` appeared in §2.3 as a symbol with no value anywhere in the spec, so §2.3 was not implementable as written.
*   The shed rule's depth signal ("every candidate advertises $h \ge D_{\max}$") was firing on stale-high values after ancestors migrated up — a spurious layer shed with no capacity shortage. It now fires on current values only.

## The generalisable lesson

**Any value a node advertises about itself that depends on its ancestors must be refreshed on a path the ancestors already use.** The join handshake is the wrong place to learn something that changes every time the tree moves.

## Residual risk

`SenderHopDepth` is self-declared by the parent. A hostile parent claiming depth 0 makes its subtree look shallower than it is, attracting joiners; the reliability term and the depth-propagation of *its* parent's real value do not correct this, because it can lie downward freely. Bounded by the same mechanisms that bound capacity lies (children observe latency and leave), and by the source pinning of depth 0 to `SOURCE`/`SOURCE_INGRESS` records — a node claiming 0 that is not one of those is rejected.

## Validation owed (Chapter 8)

*   Distribution of $|h_{\text{advertised}} - h_{\text{true}}|$ across the forest under steady migration at $N = 10^6$.
*   Whether 30 ms hysteresis suppresses oscillation at RTT jitter typical of cellular last miles.
