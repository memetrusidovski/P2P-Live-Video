# SOLUTION-029: Every Control Path the Spec Depends On Has a Frame

**Closes:** ISSUE-029 (High) — resolved item by item across the discovery, manifest and incentive clusters
**Lives in:** `protocol/appendix_d_frame_registry.md` (registry, §D.4.6b, §D.4.8, §D.4.11, §D.4.12, §D.4.14–§D.4.18); `protocol/chapter2/2.3_stream_registration/2_store_get_rpcs.md`, `3_registration_frames.md`; `protocol/chapter5/5.2_proof_of_upload/3_pou_frame.md`
**Class:** Triggers described but never given a wire encoding (recurring pattern #4, eight instances)

---

## The problem in one line

Appendix D called itself the single normative registry, and eight mechanisms other normative rules consumed — the roster, the loss report, the guardian→publisher counts, the Stream Record's write path and byte layout, the rarity signal, past-manifest fetch, and NAT signaling — had no frame or field carrying them.

## The decision, per item

| # | Path | Now carried by | Solution |
| :-: | :--- | :--- | :--- |
| 1 | Child roster | `ROSTER` (0x18), per tree, with `ChildCount` and addresses | 020 |
| 2 | Per-link loss rate $\rho$ | `PROOF_OF_UPLOAD.LossRate`, once per segment per tree | 024 |
| 3 | Guardian → publisher counts | `STORE_RECORD_ACK` (0x1B): active, relay, per-tree relay and starved counts | 026 |
| 4 | Publisher → guardian record write | `STORE_RECORD` (0x1A), once per segment | 026 |
| 5 | Stream Record byte layout | Fixed-field canonical layout, 92 B + 7 B/tree + signature | 026 |
| 6 | Rarity signal | `PULL_REQUEST.Flags.BROADCAST_WANT` | 030 |
| 7 | Past-manifest fetch | `MANIFEST_REQUEST` (0x1D) | 030 |
| 8 | NAT signaling and relay discovery | `PUNCH_REQUEST` (0x1C) via the referrer; `Flags.RELAY_CAPABLE` + `GET_PEERS.ReqFlags` | 032 |

The source's address, also unstated, is resolved by the rule that the source registers under $K_s$ like any peer with `Flags.SOURCE` (or an ingress relay does so on its behalf).

## Why one issue and eight solutions

The eight gaps had one shape and eight different owners. Fixing them in a single "add frames" pass would have produced eight frames designed in isolation from the mechanisms consuming them — the roster without the per-tree filter that makes it useful, the loss report as its own frame when a receipt already travels the same path once per segment, a guardian stats request when the publisher already writes to the guardians every second. Each frame was instead designed inside the cluster that owned its consumer, and this record exists so a reader can see that the registry is now closed against the list.

## Defects found during verification

*   Two of the eight (items 3 and 4) were one round-trip once the publisher's write path existed; designing them separately would have doubled the guardian traffic.
*   Item 2 would have been a fifth per-block frame under the old receipt design; folding it into the per-segment receipt cost one byte.
*   Item 8's "ICE candidate exchange" as written violated the protocol's no-self-declared-address rule (SOLUTION-012); the referrer-forwarded observed address is the only design consistent with it.

## The generalisable lesson

**Before closing a spec section, list every value another section consumes from it and find the frame that carries each one.** The registry claimed completeness; the check that would have falsified the claim is a grep for "reports", "gossips", "presents" and "obtains" followed by a search for the field.

## Residual risk

The non-normative `schemas/p2p_live.proto` has not been regenerated for any of these frames and now lags the registry substantially; it is labelled as lagging rather than silently wrong. Frame conformance vectors (one encoded example per frame) remain future work.

## Validation owed (Chapter 8)

Control-to-data overhead ($\text{CDO} \le 2\%$) re-measured with all eight paths active, at $K_v = 10$ and at super-node fan-out.
