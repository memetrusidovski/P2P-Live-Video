# SOLUTION-054: Chapter 2 Counts and Records Made Consistent

**Closes:** ISSUE-054 (Low)
**Lives in:** `protocol/chapter2/2.3_stream_registration/2_store_get_rpcs.md` (step 4, two-signature rationale, publisher table), `3_registration_frames.md` (`STORE_RECORD_ACK` `QueryCount`, Stream Record `EffectiveSegmentSeq`/`NumTreesNext`); `appendix_d_frame_registry.md` §D.4.16 (`0xFE`); `appendix_b_parameters.md`
**Class:** Derived text drifting from its definitions (recurring pattern #12), plus one missing field

---

## The problem in one line

Four small defects in one subchapter: `StarvedCount` scaled by $2^{s}$ as if queries were sampled ($128\times$ too high at $N = 10^6$); "servable until 180 s" versus "only active registrations are returned"; a joiner inside a migration window holding the pending matrix and none for the manifests it received; and a third-party-verifiability claim for a registration signature no frame carried to third parties.

## The decision

1.  Only the three registration counts are scaled; `StarvedCount` and the new `QueryCount` (the 2 reserved bytes of `STORE_RECORD_ACK`) are never scaled.
2.  `GET_PEERS` returns active registrations only; the 135–180 s band is *retained* (for a cheap late refresh) but neither counted nor served.
3.  The Stream Record gains `EffectiveSegmentSeq 4B` and `NumTreesNext 1B` (the reserved byte) followed by the pending matrix; the first matrix is the one in force. Fixed part 96 B, $\le 244$ B at $M = 6 \to 6$; the `GET_PEERS` response stays under the MTU at $\approx 1{,}365$ B. `MANIFEST_REQUEST(ChunkIndex = 0xFE)` returns the pending `MANIFEST_UPDATE`.
4.  The registration signature's consumer is the guardian, which can present two contradictory statements as `EQUIVOCATION` evidence; the Peer Record is unsigned and trusted as far as its sender.

## Why this and not the alternatives

*   **Carry the registration signature in the Peer Record** (+72 B): a 20-record IPv6 response reaches $\approx 2.8$ KB, over the MTU §2.3.3 checks against, for a property only the equivocation evidence needs.
*   **Carry only the pending matrix in the Stream Record** during the window: the joiner then cannot map a missing block of the manifests on the wire (version $v$) to a tree until the switch.

## Defects found during verification

*   Item 1 is load-bearing for SOLUTION-040: the fold trigger $\hat{s}_m$ is normalised by the scaled swarm-size estimate and would have been $128\times$ off.
*   $\approx 16{,}000$ peers join inside each 5 s window at $N = 10^6$; item 3 is not an edge case at scale.

## The generalisable lesson

**When one frame carries several counts, say for each whether it counts a sampled or an unsampled population.** "All counts are raw" was true and insufficient.

## Residual risk

The Stream Record now changes size at every announcement; receivers already parse it by `NumTrees`, and `NumTreesNext` follows the same pattern.

## Validation owed (Chapter 8)

*   None specific; item 3 is covered by the resize validation of SOLUTION-043.
