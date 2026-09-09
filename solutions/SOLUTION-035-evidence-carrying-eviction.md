# SOLUTION-035: Accusations Carry Their Evidence; Eviction Is Local and Bounded

**Closes:** ISSUE-035 (High)
**Lives in:** `protocol/chapter5/5.3_reputation_auditing/3_consensus_eviction.md` (rewritten); `appendix_d_frame_registry.md` §D.4.18; `appendix_b_parameters.md`; `protocol/chapter7/7.1_source_pinning/1_genesis_key_anchoring.md` (ban semantics, with SOLUTION-030)
**Class:** The mechanism meant to remove attackers was the cheapest attack (recurring pattern #5)

---

## The problem in one line

`REPUTATION_AUDIT_GOSSIP` carried unsigned `(Suspect, PenaltyScore)` opinions — up to 65,535 per frame — with no aggregation rule, no evidence, no standing requirement and no cost to accuse, and "all well-behaved nodes immediately sever" a suspect over an undefined consensus at $0.8$; with 10 ms identities one packet could orphan a super node's ten thousand children.

## The decision

*   **Admissible accusations are self-contained and signed**: `SYMMETRIC_TREE_PAIR` (two receipts, same tree, same window) or `EQUIVOCATION` (two contradictory statements signed by the suspect). Receivers re-verify before counting and never forward what they could not verify. At most two accusations per frame.
*   **Eviction requires $\ge 3$ distinct verified accusers of rank $\ge 0.25$ spanning $\ge 3$ address prefixes** within 10 minutes.
*   **Eviction is local and bounded**: 10 min, doubling to 24 h, never gossiped as a ban list.
*   **Accusing falsely costs**: an accuser whose evidence fails verification is treated as a forger (local ban). Accepted accusations are rate-limited to 4 per accuser per minute.
*   **Block poisoning and subnet clustering are not accusation types**, because neither can be proven to a third party from signatures; both remain local decisions.

## Why this and not the alternatives

*   **Weighting opinions by accuser $\Theta$ without evidence** still lets a coalition of high-rank nodes (or one compromised super node) remove anyone; evidence makes the accusation checkable by every receiver independently, so rank is a *standing* requirement, not the decision.
*   **A gossiped ban list** propagates a wrong decision swarm-wide and permanently; local, time-bounded eviction contains every error to the nodes that made it, for a bounded time.
*   **Allowing poisoning evidence** (block plus failing proof) was attractive but the relay's frames are not signed by the relay, so the evidence cannot name the offender. Adding per-hop signatures to the push path for this purpose would cost more than the attack.

## Defects found during verification

*   The old frame's `Suspect Count` was 16-bit: one datagram could accuse 65,535 peers. The new frame carries two.
*   With the aggregate symmetry test (ISSUE-039), *true* evidence of symmetric flow existed for most honest relays in mid-sized swarms — so evidence-carrying accusations alone would still have evicted honest nodes. SOLUTION-039's per-tree test is a precondition for this one.
*   The small-swarm guard's stated failure mode referenced the old $0.8$ threshold; restated against the three-accuser rule.
*   §7.1.1's "permanently banned" (fixed with SOLUTION-030) was the other half of the same problem: any local misjudgement became permanent.

## The generalisable lesson

**A distributed punishment must be checkable by every node that applies it, and its cost must fall on the accuser when it is wrong.** An unverifiable accusation is an attack primitive, whatever it is called.

## Residual risk

Three colluding ranked peers on three prefixes can evict an honest node locally at every receiver they reach — for 10 minutes, on evidence that must actually verify. The evidence types are constructed so that fabricating them requires the *suspect's* signature (equivocation) or two receipts the suspect and a counterparty both signed (symmetry), which a third party cannot forge. The remaining exposure is a suspect who genuinely equivocated once by software error; the bounded duration is the mitigation.

## Validation owed (Chapter 8)

*   Scenario B: time from a colluding pair's first symmetric segment to their eviction at a majority of honest nodes, under the three-accuser rule.
*   Audit-channel bandwidth at $N = 10^6$ with the 4-per-minute cap.
