# SOLUTION-034: Proof-of-Work Is a Rate Limit; Address Prefixes Are the Sybil Bound

**Closes:** ISSUE-034 (Medium)
**Lives in:** `protocol/chapter2/2.2_crypto_node_id/1_static_dynamic_puzzles.md` (opening), `2_sybil_defense_math.md` (rewritten); `appendix_c_threat_model.md` (C.1 Spoofing, C.2.1–C.2.3); `protocol/chapter5/5.1_tit_for_tat/3_optimistic_exploration.md` (FIFO re-entry); `protocol/chapter5/5.2_proof_of_upload/1_receipt_cryptography.md` (Known Limitation); `appendix_b_parameters.md` (prefix cap)
**Class:** Threat-model claims wrong in kind, not degree

---

## The problem in one line

$C_1 = 16$ is $\approx 10$ ms of one CPU core per identity and $10^4$–$10^5$ identities per second on a GPU; the spec called this "significant expenditure" that "completely neutralizes" Sybil attacks, and three anti-abuse arguments rested on "a fresh identity is expensive".

## The decision

*   **State the cost honestly** and reclassify PoW as a rate limit on identity creation and IP migration — not a price on identities.
*   **Name address-prefix diversity as the operative Sybil bound**, and collect every place it applies into one table: $5\%$ of slots per `/24`/`/48` in k-buckets, Active Sets, parent and child sets; $20\%$ of a receipt bundle; three prefixes to evict; cooldowns keyed on prefix.
*   **Re-derive the three dependent arguments** on prefixes: FIFO re-entry cooldown keyed on prefix as well as NodeID; reveal-before-prove exposure bounded per prefix per cooldown by the $5\%$ slot cap; eclipse resistance stated with $f$ as the *prefix* fraction the attacker controls.
*   $C_1$ stays at 16; raising it is explicitly rejected as ineffective.

## Why this and not the alternatives

*   **Raising $C_1$ to 24–28** costs a phone 2–40 s per identity and a GPU still under a second per thousand. The asymmetry is the problem and difficulty does not touch it.
*   **A memory-hard puzzle** (Argon2-class) narrows the CPU/GPU gap by one or two orders of magnitude, does not close it, and taxes exactly the low-end devices the protocol wants as viewers. Deferred to Chapter 8 as a measured question.
*   **Leaving the text and fixing only the dependent mechanisms** would leave a reader believing PoW suffices and skipping the prefix machinery — the failure mode the issue identified.

## Defects found during verification

*   Appendix C's "5,000 IDs computationally expensive": $\approx 50$ s on one core, $\approx 0.05$ s on a GPU.
*   The `/48` choice for IPv6 was correct but unexplained; a per-`/64` cap would hand one subscriber millions of prefixes. Stated.
*   CGNAT cuts the other way — thousands of honest viewers behind one IPv4 address — which is why caps are on *slots held* rather than *connections attempted*, and why SOLUTION-037 raised the per-address XDP budget. Stated.

## The generalisable lesson

**Every "X is expensive" claim in a threat model needs a number, the hardware it was measured on, and the adversary's hardware.** Ten milliseconds is expensive for a phone and free for a data centre; the defence has to work against the second.

## Residual risk

Prefix diversity is a purchasable resource — a determined attacker with a few hundred `/24`s (a hosting provider's allocation) can hold a meaningful share of a target's slots. The caps bound damage per prefix; they do not make large-scale Sybil attacks impossible, and the spec now says so rather than claiming otherwise.

## Validation owed (Chapter 8)

*   Eclipse success probability versus number of attacker prefixes under the $5\%$ cap, at $k = 20$, $\alpha = 3$.
*   Whether a memory-hard static puzzle is worth its cost on ARM Cortex-A53 class devices.
