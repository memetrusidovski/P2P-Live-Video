# SOLUTION-022: Heartbeats, RTT-Scaled Eviction, Source Pacing, and a Segment-Clock Receipt Deadline

**Closes:** ISSUE-022 (High)
**Lives in:** `protocol/chapter3/3.3_churn_recovery/1_recovery_timeline.md` (rewritten), `2_active_probing.md`; `protocol/chapter1/1.2_multi_forest_overlays/3_topology_healing.md` (Phase 1); `protocol/chapter5/5.2_proof_of_upload/1_receipt_cryptography.md`; `appendix_b_parameters.md`; `appendix_a_sequence_diagrams.md` A.2; `appendix_c_threat_model.md`; headline text in `INDEX.md`, chapter READMEs and `README.md`
**Class:** Absolute timers on a quantity that scales with the path (recurring pattern #5)

---

## The problem in one line

A 200 ms eviction deadline declared every parent more than 100 ms away dead on its first lost heartbeat, a 50 ms receipt deadline choked every honest child on any path over 50 ms, and nothing bounded the silence on a healthy connection — so at global scale the protocol manufactured churn on most links.

## The decision

*   **Heartbeat on idle**: a parent that has sent nothing for $\tau_{\text{ping}}$ sends a heartbeat. Healthy silence is bounded at 100 ms whatever the media pattern; silence alone is never evidence.
*   **$\tau_{\text{evict}} = \max(200, 2\tau_{\text{ping}} + SRTT + 4\,RTTVAR)$ ms** from the connection's QUIC estimator: 240 ms locally, 320 continental, 490 intercontinental. Recovery is "sub-300 ms locally, sub-600 ms worst case" — the flat 250 ms is retired everywhere it was quoted.
*   **Source pacing** over at most half the 250 ms chunk period: the forest carries a near-continuous flow, no relay's egress exceeds $2\times$ its mean, and super-node XDP buckets stay inside their burst.
*   **Receipts on a segment clock**: due before the first block of segment $k+2$; choke at the next TFT cycle; evict after three consecutive unreceipted segments. The 50 ms rule is deleted.

## Why this and not the alternatives

*   **A larger fixed timeout** (say 500 ms) fixes the far links by slowing every local repair; a path-scaled timeout gives each link the smallest deadline it can honestly meet.
*   **Media-silence detection with pacing but no heartbeats**: pacing bounds gaps at the source, but a relay whose *input* stalls (its own parent died) goes silent to its children through no fault of its own — and that is exactly the case where the children should *not* immediately evict it (SOLUTION-036: it is repairing, and its other trees are fine). Heartbeats let a repairing relay stay alive to its children while it re-attaches; media silence cannot.
*   **Keeping a short per-block receipt deadline with an RTT allowance** ($SRTT + $ margin per block) would work but the per-block receipt itself is going away (ISSUE-024); the segment clock is the natural unit of the aggregated receipt and is trivially RTT-safe.

## Defects found during verification

*   The two statements in Ch5 §5.2.1 — "within 50 ms … immediately chokes" and "chokes on the very next TFT cycle ($\le 500$ ms)" in the same section's Known Limitation — were contradictory; the second is the one kept.
*   The old timeline diagram fixed T = 200 ms for eviction and T = 250 ms for reconnect; with $RTT_{\text{avg}} = 80$ ms (the spec's own assumption) the PONG arrives at 180 ms with 20 ms of margin, and one more lost packet ends the connection. The sibling election's "all children detect simultaneously" held only for identical RTTs; it now says "within $\approx 4\,RTTVAR$", which the 5 s roster staleness bound covers.
*   Pacing's latency cost ($\le 125$ ms on a chunk's last block) was added to the glass-to-glass budget (SOLUTION-033) rather than hidden.

## The generalisable lesson

**A timeout is a claim about the path; write it as a function of the path's measured RTT, floor it, and state the resulting headline as a range.** "Sub-250 ms" was true of a LAN and was quoted for a planet.

## Residual risk

QUIC's RTT estimator needs a few samples; a brand-new connection uses the floor (200 ms) until it has them, so the first seconds after a join or repair have the old behaviour on far links. Seeding $SRTT$ from the parent-selection probe RTT (which the joiner just measured) closes most of this and is recommended to implementers.

## Validation owed (Chapter 8)

*   False-eviction rate versus path RTT with 1–2% UDP loss, for the fixed 200 ms and the scaled rule.
*   Recovery-time distribution by RTT band under Scenario A.
*   Whether $4\,RTTVAR$ is the right jitter multiplier on cellular last miles, or whether the estimator needs a floor on $RTTVAR$.
