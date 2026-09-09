# SOLUTION-042: The Prefix Cap Is Relative to Observed Diversity, With Its Rounding Defined

**Closes:** ISSUE-042 (High)
**Lives in:** `protocol/chapter2/2.2_crypto_node_id/2_sybil_defense_math.md` §2.2.2 (table and rule), §2.2.3; `protocol/chapter1/1.2_multi_forest_overlays/1_graph_theory_and_slicing.md` (*Distinct-Parent Rule*); `protocol/chapter5/5.2_proof_of_upload/1_receipt_cryptography.md`; `appendix_c_threat_model.md` §C.2.1; `appendix_b_parameters.md`
**Class:** A rule derived from large-$N$ intuition, written as unconditional, with its rounding at small $K$ never stated (recurring pattern #5)

---

## The problem in one line

An unconditional "5% of slots per /24" on parent sets of $\le 6$, Active Sets of 8 and home-relay child sets of 1–20 is less than one slot, so the unstated rounding decided everything: under floor no home relay could accept a child; under "at least one" a ten-viewer stream from one dorm became a chain that hit $D_{\max}$ at the ninth viewer and rejected the tenth in every tree forever.

## The decision

*   **k-buckets: exactly one of 20 entries per prefix, unconditionally** — the eclipse defence, and the thing that makes the next quantity trustworthy.
*   **Active Set, parent set, per-tree child slots**: $c_p(K) = \max(1, \lceil K / \min(P_{\text{obs}}, 20) \rceil)$, where $P_{\text{obs}}$ is the number of distinct prefixes the node has itself observed across k-bucket contacts, Peer Records and sessions. At $P_{\text{obs}} \ge 20$ this is $\lceil K/20 \rceil$ — the old 5% with ceiling rounding; at $P_{\text{obs}} = 1$ there is no cap.

## Why this and not the alternatives

*   **Waive the cap below $N_{\text{collusion}} = 20$ peers**: $N$ is not the right variable — a 200-viewer campus stream from one /24 is large and single-prefix. Observed prefix diversity is.
*   **Cap at a prefix's share of the node's observed *records***: a Sybil flood from one prefix inflates that share; the attacker raises its own cap by registering more identities, which are cheap. The *count* of distinct prefixes observed can be raised only by controlling more prefixes and lowered only by eclipse.
*   **Floor rounding**: no home relay could accept any child at $P_{\text{obs}} \ge 20$.

## Defects found during verification

*   5% of the spec's own slot counts: $0.3$ of $6$ parents, $0.4$ of $8$, $0.5$ of $10$ children, $0.25$ of $5$.
*   Same-prefix $N = 50$ lecture hall at $M = 6$ under "at least one": 8 relays per tree, 8 slots per tree from the prefix, 42 peers with no parent in any tree.
*   At large $N$ the cap bounds a CGNAT population: $\sum_v \lceil K_v(m)/20 \rceil \ge N_{\text{relay}}/M \approx 8\%$ of $N$ per tree at the reference numbers. A carrier pool larger than that share is capacity the forest cannot place. This limit is now stated in §2.2.2 rather than implied.
*   The distinct-parent rule text and Ch5 §5.2.1's "5% of active slots" both restated the cap; both now name the rule.

## The generalisable lesson

**A percentage cap on a small integer set is a rounding rule in disguise; write the rounding, and write the condition under which the cap is meant to apply at all.** Five percent of six is a policy decision, not arithmetic.

## Residual risk

*   $P_{\text{obs}}$ at bootstrap is small; a node whose first records are all one prefix and whose k-buckets have not yet filled runs with a loose cap for a few seconds. The one-per-prefix k-bucket rule fills $P_{\text{obs}}$ to 20 with the first healthy self-lookup.
*   The CGNAT placement limit is real: a carrier pool above $\approx 8\%$ of the swarm falls partly to the source reserve and to bridges. Chapter 8 should measure how often real deployments reach it.

## Validation owed (Chapter 8)

*   Join success in a single-/24 swarm of 10, 50 and 200 with the new cap.
*   Placement rate of a CGNAT population at 5%, 10% and 20% of $N = 10^6$.
