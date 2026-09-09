# SOLUTION-052: Re-Attachment Costs Round-Trips; the PULL Zone Follows the Path

**Closes:** ISSUE-052 (Low); completes SOLUTION-022
**Lives in:** `protocol/chapter3/3.3_churn_recovery/1_recovery_timeline.md` (timeline, *Re-attachment costs round-trips*, per-path table); `protocol/chapter4/4.3_hybrid_push_pull/1_buffer_sliding_timeline.md` ($W_{\text{pull}}$); `appendix_a_sequence_diagrams.md` A.2; `appendix_b_parameters.md`
**Class:** A timeout made RTT-scaled in one half of a timeline while the other half kept its LAN constant (recurring pattern #2)

---

## The problem in one line

SOLUTION-022 scaled detection with RTT and left re-attachment at "+50 ms" for every path; the standby has no open socket and `NEIGHBOR` rides a QUIC control stream, so re-attachment is a handshake plus a request — 1.5–2 RTT — and at 250 ms RTT the fixed 1.5 s PULL zone had no margin for one repair plus two PULL round-trips.

## The decision

*   Re-attach is quoted as $\approx 2$ RTT to a peer never spoken to and $\approx 1$ RTT with a 0-RTT resumption ticket; peers retain tickets for every peer they have held a session with (which covers every standby demoted from their own Active Set) and send `NEIGHBOR` in the 0-RTT flight. Recovery by path: $\approx 280$ ms at 20 ms, $\approx 480$ at 80, $\approx 990$ at 250.
*   The PULL zone is $W_{\text{pull}} = \text{clamp}(\tau_{\text{evict}}^{\max} + 6\,SRTT^{\max}, 1.5, 2.0)$ s over the peer's parent paths — the 1.5 s floor up to $\approx 150$ ms RTT, 2.0 s at 250 ms. $\Delta_{\text{buffer}}$ is unchanged.

## Why this and not the alternatives

*   **Pre-open QUIC sessions to the per-tree pool floor** ($\le 18$ idle sessions): makes "+50 ms" true at the cost of 18 heartbeated connections per peer, on a design whose passive set is defined by having none. Resumption tickets buy most of the benefit for none of the idle cost.
*   **Widen $\Delta_{\text{buffer}}$**: the largest term in the glass-to-glass budget (SOLUTION-033); moving the zone boundary costs only some premature pulls.

## Defects found during verification

*   At 250 ms: repair $\approx 0.99$ s, plus two PULL round-trips $0.5$ s, $= 1.49$ s against a 1.5 s zone; with the Deputy path failing first ($+45$ ms $+$ another repair) $\approx 1.9$ s. The zone now reaches 2.0 s there.
*   The Deputy election inherits the same cost twice (the Deputy's own re-attach, the orphans' handshake to a sibling not in their Active Set); §1.2.3's 250 ms figure is a local-path figure and is now labelled by its RTT.

## The generalisable lesson

**When one term of a sum is made RTT-scaled, every other term that contains a handshake must be re-derived on the same RTT.** A timeline is only as scaled as its least scaled step.

## Residual risk

$W_{\text{pull}}$ evaluated over the *worst* parent path pulls blocks earlier on every tree when one parent is far; wasted pulls are bounded by the PULL budget and Chapter 8 should measure them.

## Validation owed (Chapter 8)

*   Repair-complete time distribution at 20 / 80 / 250 ms RTT with and without resumption tickets.
*   Premature-pull rate under $W_{\text{pull}} = 2.0$ s.
