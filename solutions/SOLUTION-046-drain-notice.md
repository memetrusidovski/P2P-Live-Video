# SOLUTION-046: `DRAIN_NOTICE` — the Parent-Side Half of the Drain Path

**Closes:** ISSUE-046 (Medium)
**Lives in:** `appendix_d_frame_registry.md` §D.4.19 (new frame 0x1E), §D.3 (reason codes 0x05–0x09 shared with `DISCONNECT`); `protocol/chapter1/1.2_multi_forest_overlays/2_parent_selection_algorithm.md` (*The Drain Path*); `1_graph_theory_and_slicing.md` (§1.3 $K_v$ fall, §1.4 shrink); `3_topology_healing.md` (election on notice); `4_stream_slicing_architecture.md` §4.5 step 3; `5_node_classes.md` §5.3; `protocol/chapter3/3.2_topology_gossip/1_shuffle_neighbor_frames.md`; `protocol/chapter1/1.3_peer_lifecycle/2_algorithmic_core_loop.md`; `appendix_b_parameters.md`
**Class:** A trigger described but never given a wire encoding (recurring pattern #4)

---

## The problem in one line

Five rules released children "through the drain path" and two said the parent "announces" it, but the registry had no frame for the announcement and `DISCONNECT` closes the connection *after* the window — so the child learned of its release only when the window was over and repaired with exactly the gap the window existed to avoid.

## The decision

**`DRAIN_NOTICE` (0x1E)**: `[TreeID][Reason][Scope][DeadlineSegmentSeq]` on the tree's QUIC stream, parent → child. The parent keeps serving until the deadline or the child's `DISCONNECT_CHOKE`, then sends `DISCONNECT(Reason)`. On receipt the child runs the Ch3 §3.3 promotion for the tree at once with its parent still delivering — a warm repair. With scope *every child* the children run the roster-based Deputy election with the notice as trigger. Reasons extend the `DISCONNECT` code space: `DISPLACED`, `REASSIGNED`, `DEMOTED`, `CAPACITY`, `DEPTH`, alongside `PREEMPTED`. All five trigger sites reference the frame.

## Why this and not the alternatives

*   **Send `DISCONNECT(reason)` at the start of the window and keep pushing** contradicts the frame's own definition ("terminate a peer connection") and any receiver that closes on it.
*   **Rely on the child's 5 s migration tick** to find a better parent: nothing tells it to go *now*, and a preempted child has no reason to move on score.
*   **A new election protocol for the multi-child drain**: the roster the children already hold is the same input the dead-parent election uses; only the trigger differs.

## Defects found during verification

*   The child-side half already existed (`DISCONNECT_CHOKE` once the new parent delivers, §2.3); only the parent-side trigger was missing, which is why the fix is one small frame.
*   Under the literal old text a preempted child took a $\tau_{\text{evict}}$-free but handshake-full repair of $1$–$2$ RTT with no push during it; the notice makes that repair overlap the old parent's delivery.
*   The core loop had no path for a drain; it now treats a notice as entering per-tree `CHURN_REPAIR` without evicting the parent.

## The generalisable lesson

**"The parent keeps serving for a window" is worth nothing unless the child knows the window has started.** Every graceful-release rule needs the release itself on the wire, distinct from the final close.

## Residual risk

A hostile parent can send notices to churn its children; each costs the child one warm repair, and a parent that does it repeatedly is scored down by reliability and abandoned. Not an amplification vector: one frame, one child, one repair.

## Validation owed (Chapter 8)

*   PSR of drained children with and without the notice at each of the five triggers, at 20 / 80 / 250 ms RTT.
