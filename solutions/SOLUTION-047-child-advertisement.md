# SOLUTION-047: The Child Advertisement Rides on the Receipt; `NEIGHBOR` Carries Class

**Closes:** ISSUE-047 (Medium)
**Lives in:** `appendix_d_frame_registry.md` §D.4.3 (`NEIGHBOR` `NodeClass`, `AssignedTrees`), §D.4.12 (`PROOF_OF_UPLOAD` `K_avail`, `AssignedTrees`, `NodeClass`), §D.4.15 (roster data sources); `protocol/chapter5/5.2_proof_of_upload/1_receipt_cryptography.md`, `3_pou_frame.md`; `protocol/chapter3/3.2_topology_gossip/1_shuffle_neighbor_frames.md`; `protocol/chapter1/1.2_multi_forest_overlays/3_topology_healing.md`; `appendix_b_parameters.md`
**Class:** Prose amended, byte layout not (recurring pattern #3); a consumer with no producer (pattern #4)

---

## The problem in one line

The per-tree `ROSTER` listed each child's `K_avail`, `AssignedTrees`, `NodeClass`, flags and address, and no frame from child to parent carried the first three; `NEIGHBOR` was said in §1.2.5 to carry `NodeClass` and its byte diagram did not, so the leaf floor at admission decided on a class the request did not contain.

## The decision

*   **`PROOF_OF_UPLOAD` gains `[K_avail 2B][AssignedTrees 1B][NodeClass 1B]`** — the child advertisement — inside the signature, before $K_s$. It is the one child → parent frame that already recurs once per segment per tree, the roster's own cadence. A third party ignores the three fields. Size $184 + L$.
*   **`NEIGHBOR` gains `[NodeClass 1B][AssignedTrees 1B]`** in its two reserved bytes; zero size change.
*   The roster's `Flags` and address are the parent's *observed* values. A child whose advertisement is older than two segments leaves the roster.

## Why this and not the alternatives

*   **Probe one's own children every second**: $8{,}700$ signed plain-UDP `PROBE`s per second at a super node, each costing a validation block and two Ed25519 operations — for data the child could have volunteered.
*   **A new `CHILD_STATUS` frame** at the same cadence: a frame type and 12 bytes of framing per segment per child for 4 bytes of payload that already had a signed carrier.
*   **Put the advertisement in `NEIGHBOR` only**: $K_{\text{avail}}$ changes every segment (parity recomputation) and on every join; admission-time data is stale within seconds.

## Defects found during verification

*   Without a source of `K_avail` the roster's Deputy ordering was undefined; every orphan would have skipped the election (empty roster) or elected a Deputy with unknown free slots and waited out $\tau_{\text{deputy}}$ — the case §1.2.3 spends a page avoiding.
*   Putting a volatile field inside a signed receipt is semantically odd but harmless: the signature attests the child *said* it, and nothing downstream (rank, symmetry audit) reads it. Noted rather than hidden.
*   The `NEIGHBOR` diagram in Ch3 and the amendment in Appendix D are now identical.

## The generalisable lesson

**For every field a frame lists, name the frame that delivered it to the sender.** The roster was specified as an output with no inputs.

## Residual risk

The advertisement is self-declared; a child claiming free slots it lacks becomes a Deputy that cannot absorb orphans, who fall back after $\tau_{\text{deputy}}$ — the same bounded cost as a stale roster.

## Validation owed (Chapter 8)

*   Election success rate (orphans served by the Deputy versus fallback) with advertisement-driven rosters, under Scenario A churn.
