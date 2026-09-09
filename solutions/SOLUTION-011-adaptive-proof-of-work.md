# SOLUTION-011: Adaptive Proof-of-Work Difficulty

**Closes:** ISSUE-011 (Low — with a High-severity consequence)
**Lives in:** `protocol/chapter2/2.2_crypto_node_id/1_static_dynamic_puzzles.md`, `3_validation_frame.md`, `appendix_b_parameters.md`
**Class:** Sybil defense / mobile usability

---

## The problem in one line

$C_2 = 12$ was constant, so a phone joining a 5-viewer stream paid the same IP-bound puzzle as one joining a million-viewer stream — and paid it again on every WiFi↔cellular handover, in the regime where nobody is Sybil-attacking anything.

## The decision

Split the two puzzles by what they actually defend:

* **$C_1 = 16$ stays fixed at every scale.** It is a one-time identity cost, and identity cost is what actually bounds a Sybil flood.
* **$C_2$ scales with swarm size** — 8 / 10 / 12 / 14 bits at $N <$ 50 / $10^3$ / $10^5$ / $\ge 10^5$ — because it is paid *repeatedly*, on every IP change.
* **Same-subnet reconnect** with a signed identity-continuity proof earns a halved re-solve.

## The design improvement over the issue's proposal

The issue proposed encoding the difficulty tier in the validation frame so verifiers know what to check. The spec found something better: **no wire field is needed at all.** A proof solved at a higher difficulty automatically satisfies every lower threshold, so a verifier just checks against the tier *it* computes from the swarm size *it* observes. That removes a field, removes a way for a solver to lie, and removes the need to keep two parties' notions of the tier in sync — a one-tier grace band absorbs the disagreement during rapid growth.

Worth keeping as a pattern: **a monotone verification predicate needs no negotiated parameter.** Whenever "harder also satisfies easier" holds, the difficulty does not have to travel on the wire.

## The defect: three reasonable discounts, applied multiplicatively

Each reduction to $C_2$ was introduced by a different fix, in a different document, each individually justified. Nothing said how they compose, so the natural reading is that they all apply:

| Applied in sequence at $N \ge 10^5$ | Bits | Hashes |
| :--- | :---: | ---: |
| Base tier | 14 | 16,384 |
| − leaf-class rung (Ch1 §1.2.5) | 12 | 4,096 |
| − verifier grace band (Ch2 §2.2.3) | 10 | 1,024 |
| − reconnect halving (Ch2 §2.2.1) | **5** | **32** |

**512× weaker than intended, at exactly the scale where the Sybil threat is real.** An attacker declaring `LEAF` class and asserting same-subnet reconnects faces a 32-hash puzzle. Every ingredient was defensible; the composition was never written down, so it defaulted to the worst case.

### Resolution

$$C_2^{\text{eff}} = \max\left(C_2^{\min},\ \text{tier}(N) - \max\left(\delta_{\text{class}},\ \delta_{\text{reconnect}}\right)\right), \qquad C_2^{\min} = 8$$

Three rules:

1. **Discounts are alternatives, not addends** — `max`, not sum. A leaf-class node doing a same-subnet reconnect pays the larger reduction, not both.
2. **The grace band is a verifier tolerance, not a solver discount.** It exists so a verifier whose swarm-size read runs *ahead* of the solver's does not reject an honest proof during growth. It applies to $\text{tier}(N)$ before any class reduction and is never composed with one.
3. **A hard floor of 8 bits**, whatever combination applies.

### Why an aggressive reconnect discount is still safe

Because the property the dynamic puzzle protects is not carried by its bit count in that case. The anti-migration guarantee — you cannot pre-generate identities elsewhere and move them in — comes from the **same-subnet requirement plus the signed continuity proof**. The puzzle only needs to stay expensive enough that automated re-solving is not free. That is what makes the floor a sufficient answer rather than a compromise.

## The generalisable lesson

**Discounts to a security parameter must have a stated composition rule at the point where the parameter is defined, not at the points where each discount is introduced.** Three documents each said "reduce $C_2$ by …" and none owned the total. This is the same failure shape as SOLUTION-003's slot budget (three allocations, no owner of the sum) and SOLUTION-004's retention window (two constants that had to satisfy a relation). Worth a systematic audit: for every protocol parameter with more than one modifier, is there a single normative expression for the effective value?

## Validation owed (Chapter 8)

* Cost to a mobile device of a full session's worth of handovers under the adaptive ladder versus fixed $C_2 = 12$ — the usability claim.
* Sybil-flood cost at $N \ge 10^5$ under maximally-exploited discounts, confirming the floor holds it where intended.
* Whether the one-tier grace band is exploitable by a solver that deliberately under-reads swarm size, and whether it should be conditioned on proximity to a ladder boundary.
