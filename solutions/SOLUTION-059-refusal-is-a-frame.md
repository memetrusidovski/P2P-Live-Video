# SOLUTION-059: A Refused Join Is a Frame, Scoped to a Tree

**Closes:** ISSUE-059 (High); completes SOLUTION-023 (rank admission outcomes) and SOLUTION-028 (stale-bitmap correction)
**Lives in:** `appendix_d_frame_registry.md` §D.3 (`DISCONNECT` row), §D.4.3b (new amendment); `protocol/chapter3/3.2_topology_gossip/1_shuffle_neighbor_frames.md` (`DISCONNECT` layout, reply rule); `protocol/chapter3/3.3_churn_recovery/2_active_probing.md` (lines 25–27a); `protocol/chapter1/1.2_multi_forest_overlays/2_parent_selection_algorithm.md` (join pseudocode, *A refusal is a frame*); `protocol/chapter1/1.1_scale_latency/5_capacity_adaptation.md` §5.3 item 1; `appendix_a_sequence_diagrams.md` A.2; `appendix_b_parameters.md`; `schemas/p2p_live.proto`
**Class:** A trigger described but never given a wire encoding (recurring pattern #4) — the fourth time this pattern has been found on the join path

---

## The problem in one line

Two algorithms branched on `ACCEPTED` / `REJECTED` / `REJECTED_NOT_ASSIGNED` and the registry defined only `ACCEPTED`; under silence a join round cost one probe timeout per refusing candidate, the stale-bitmap correction was unreachable, a full relay was forgotten by everyone it refused, and the shed rule could not tell a saturated forest from a dead one.

## The decision

*   **`DISCONNECT` gains `TreeID`** in its reserved bytes: `0` ends the whole connection (the only meaning the old layout could carry); `m > 0` ends only the tree-$m$ relationship or declines a `NEIGHBOR(TreeID = m)` that never became one. A peer that is a gossip neighbour and a parent in one tree can now refuse a join in another without severing either.
*   **Three refusal reasons** in the shared code space: `REJECTED_SATURATED` (0x0A, Rank Admission cases 3–5 and full Active Set), `REJECTED_NOT_ASSIGNED` (0x0B), `REJECTED_DEPTH` (0x0C).
*   **Every `NEIGHBOR` is answered within $\tau_{\text{sched}}$**; silence past that plus one RTT means unreachable and is the only outcome that drops a candidate from the passive set. Saturated and depth refusals keep the record and count toward the shed threshold; a timeout round is a discovery failure, not a shed signal.

## Why this and not the alternatives

*   **A dedicated `REJECTED` frame**: one more frame type for `[TreeID][Reason]` that `DISCONNECT` carries already once it has a `TreeID`; and a refusal *is* the end of a relationship, just one that never began.
*   **`AcceptFlags.REJECTED` on `ACCEPTED`**: `HopDepth` is meaningless on refusal and a reason field would be needed anyway; the frame's name would lie.
*   **Silence as refusal** (the de-facto reading): $200c$ ms per round against $c$ candidates, indistinguishable from a dead forest, and dead code in two algorithms.

## Defects found during verification

*   The old `DISCONNECT` had no tree scope at all. That was already a latent defect for drains: `DISCONNECT(PREEMPTED)` after a drain in tree $m$ would, read literally, have closed a connection that may also carry the peers' gossip relationship. The distinct-parent rule (one parent per NodeID across trees) kept it from being worse; the `TreeID` field fixes it for drains and refusals alike.
*   Ch3 §3.3.2 line 27 removed a candidate from the passive set on *any* non-`NOT_ASSIGNED` answer, so a relay full for one round was forgotten by every joiner it refused. It now keeps saturated and depth refusals and drops only on timeout or `EVICTION`.
*   The shed rule's trigger read "every candidate rejects the join"; with timeouts distinguishable from refusals, a round of timeouts is now a re-query, not a shed.
*   `RELAY_BIND` already had a `REJECTED` bit (SOLUTION-050), showing the pattern was applied once and not generalised.

## The generalisable lesson

**Every request needs a defined negative answer as well as a positive one, and the negative answer must carry enough to tell the requester what to do next.** `ACCEPTED` alone was half a handshake.

## Residual risk

A hostile parent can answer `REJECTED_NOT_ASSIGNED` to make joiners clear a correct tree bit; the bit is restored by the next Peer Record or `PROBE_RESPONSE` for that peer, and the damage is one wasted round. Bounded.

## Validation owed (Chapter 8)

*   Join-round latency with framed refusals versus the silence reading, at $c = 5$–$20$ candidates and 20–250 ms RTT.
*   Passive-set churn with saturated refusals retained versus dropped.
