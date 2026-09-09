# SOLUTION-030: Manifests Are Admitted by Sequence, Not Wall Clock; Duplicates Are Not Forgeries

**Closes:** ISSUE-030 (High); resolves ISSUE-029 item 7 (`MANIFEST_REQUEST`) and item 6 (rarity signal)
**Lives in:** `protocol/chapter7/7.1_source_pinning/2_replay_attack_prevention.md` (rewritten), `1_genesis_key_anchoring.md` (ban semantics); `protocol/chapter4/4.3_hybrid_push_pull/1_buffer_sliding_timeline.md` step 3; `3_rarest_first_heuristics.md`; `appendix_d_frame_registry.md` §D.4.11, §D.4.16; `appendix_b_parameters.md` ($\tau_{\text{ban}}$, lookahead)
**Class:** A security check that rejected the protocol's own legitimate traffic (recurring pattern #6: transient and permanent conflated)

---

## The problem in one line

The validator rejected any manifest more than 2 s from local wall-clock time *before* verifying its signature, which rejected every manifest a late joiner backfills, made a skewed clock fatal, and — because a duplicate returned the same verdict as a forgery under a "permanently banned" rule — would have banned every parent within one segment.

## The decision

*   **Signature first; sequence window second; no wall-clock check.** A manifest is admitted iff its signature verifies and `SegmentSeq` $\in [X_{\text{edge}} - \tau_{\text{retain}},\ X_{\text{edge}} + 2]$, where $X_{\text{edge}}$ is the live edge the peer itself has observed on the push path.
*   **Five verdicts, one penalty.** `ACCEPT`, `DUPLICATE`, `OUT_OF_WINDOW`, `EQUIVOCATION`, `FORGED`. Only `FORGED` — a signature that fails against the pinned key — is a verification failure; only it is penalised. A duplicate is silent; an out-of-window manifest is dropped; two valid roots for one chunk indict the *source* and are logged.
*   **The ban is local and time-bounded**: $\tau_{\text{ban}} = 10$ min, doubling to 24 h, never gossiped.
*   **`MANIFEST_REQUEST` (0x1D)** fetches past chunk manifests from a parent's retained window, which is what late-join backfill actually needed and the push path could never supply.
*   **The rarity signal** is `PULL_REQUEST.Flags.BROADCAST_WANT` to every Active Set neighbour, bounded to one per missing block per scheduler tick; it is no longer a source-reserve trigger (the guardians' starved counts are).

## Why this and not the alternatives

*   **Widening the drift bound to ±30 s** (retention plus the old leaf offset) keeps a check that adds nothing: everything a wide band rejects, the sequence window already rejects, and the band still fails on a skewed clock.
*   **Keeping the wall-clock check after the signature check** removes the skew-bans-everything failure but still rejects legitimate backfill. Sequence relative to the observed edge is the only quantity the peer can trust and that every legitimate manifest satisfies.
*   **Gossiped bans** for forgeries would let one hostile relay's garbage remove honest peers swarm-wide; the fix for that is the evidence-carrying accusation of ISSUE-035, not a wider blast radius here.

## Defects found during verification

*   The old code's step 3 (`if seq_num in cache: return False`) was indistinguishable to the caller from a signature failure, and §7.1.1 said failures were "permanently banned". With up to $M = 6$ parents delivering each manifest, an implementation following both rules bans five parents per segment.
*   The lookahead bound ($X_{\text{edge}} + 2$) is new; without it a hostile relay could push a validly-signed *future* manifest (there are none — but a manifest with a wildly high sequence number, if the source ever signed one in error, would advance $X_{\text{edge}}$ and skip playback). Two segments covers honest record staleness.
*   The rarity path (`TriggerRarityGossip`) was a function name with no frame; it is now one flag bit.

## The generalisable lesson

**Every reject path in a validator must say whether the rejection is evidence of misbehaviour, and by whom.** A validator that returns one bit conflates "I have seen this" with "this is forged" and turns ordinary redundancy into a ban.

## Residual risk

Before $X_{\text{edge}}$ is first set (the peer's very first manifest) there is no window, so the first accepted manifest anchors everything after it; a hostile first parent could anchor a joiner at a stale edge. The Stream Record's `live_edge_segment_id` bounds this (a joiner ignores a first manifest more than $\tau_{\text{retain}}$ below the record's value), and the re-anchor rule of Ch4 §4.3.1 corrects it within one segment once a second parent pushes.

## Validation owed (Chapter 8)

*   Backfill success rate for late joiners under the sequence window at $\tau_{\text{retain}} = 8$ s.
*   False-`FORGED` rate: it should be exactly zero in an honest swarm.
