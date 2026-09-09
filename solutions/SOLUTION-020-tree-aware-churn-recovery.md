# SOLUTION-020: Tree-Aware Churn Recovery and Deputy Election

**Closes:** ISSUE-020 (High); resolves ISSUE-029 item 1 (roster wire encoding)
**Lives in:** `protocol/chapter3/3.1_neighbor_sets/1_memory_structures.md` (per-tree pools); `3.3_churn_recovery/1_recovery_timeline.md`, `2_active_probing.md`; `3.2_topology_gossip/1_shuffle_neighbor_frames.md`; `protocol/chapter1/1.2_multi_forest_overlays/3_topology_healing.md` §3.2–3.3; `appendix_d_frame_registry.md` §D.4.10, §D.4.15 (`ROSTER` 0x18); `appendix_b_parameters.md`
**Class:** A recovery path designed for a single tree, run unchanged over $M$ of them (recurring pattern #2)

---

## The problem in one line

Both candidate sources for the 250 ms repair — the Passive Set and the child roster — picked by reputation or raw $K_v$ with no knowledge of which tree a candidate relayed, so the expected number of handshakes to the first usable parent was $M/(1-\ell)$ (six to twelve), each miss deleted a passive entry, and the elected Deputy could honour the orphans' request with probability $\approx 1/M$.

## The decision

*   **The Passive Set is indexed by tree.** Entries are Peer Records, so $\mathcal{P}_m$ — relays of $T_m$ that are reachable — is a filter, not a probe. A floor of 3 candidates per subscribed tree is maintained through `WantedTrees` steering on `SHUFFLE`/`GOSSIP_EXCHANGE` and, failing that, a per-tree `GET_PEERS` at most every 30 s.
*   **Tier 1 is a filtered pick from $\mathcal{P}_m$**; tiers 2–4 are filtered the same way. The 250 ms budget is stated as holding *when $\mathcal{P}_m$ is non-empty*, and what it costs otherwise is stated too.
*   **The roster is a per-tree frame** (`ROSTER` 0x18) listing only children that relay this tree, ranked by their free slots *in this tree*, with addresses, and carrying the total child count $k$. An empty roster (no sibling relays $m$ — the $23\%$ case at $k = 8$, $M = 6$) sends every orphan straight to tier 1 with no 45 ms penalty.
*   **Multi-Deputy partition and the surplus rule are computed from the roster alone**: $D = \min(E, \lceil k / K^{(1)} \rceil)$, and an orphan is surplus iff its hash fraction exceeds $C/k$ — no orphan needs to know its index among the $k$.

## Why this and not the alternatives

*   **Probing passive candidates before promotion** would make the pick informed without changing the record — at one RTT per candidate, inside a 40 ms budget. The record has to carry the tree.
*   **A roster of all children ranked by raw $K_v$, with the orphan filtering locally**: the orphan cannot filter without the tree bitmap, and once the bitmap is present ranking by *this tree's* free slots is strictly better than ranking by total capacity — a super node with 1,000 free slots in $T_1$ and none in $T_m$ is useless to $T_m$'s orphans.
*   **Siblings as a tier-3 source** were considered again and rejected again: they are children of the same dead parent. The roster handles the coordinated case; the pools handle the independent one.

## Defects found during verification

*   The roster's `deputy_index` formula divided $k$ by "the Deputy's $K_v$", but the bounded roster (SOLUTION-014) had removed $k$ from the frame; the formula was uncomputable. `ChildCount` restores it.
*   The roster carried no address, yet siblings are not generally in one another's Active Sets, so an orphan could elect a Deputy it had no way to contact. Addresses are now in every entry.
*   Roster cost was restated with the larger entry: $\approx 450$ bytes per child per second, $0.36\%$ of uplink at every fan-out — the earlier $0.22\%$ was for a 35-byte entry.
*   The expected size of a random per-tree pool at $c_p = 32$ is $32(1-\ell)/M \approx 2.7$ at $M = 6$, $\ell = 0.5$ — below the floor of 3, which is why steering exists rather than relying on shuffle alone.

## The generalisable lesson

**Every candidate list in a multi-tree protocol must say which tree each candidate serves, or it is a list of things to probe, not a list of candidates.** The passive set was inherited from HyParView, where every neighbour is interchangeable; here none of them are.

## Residual risk

Wanted-tree steering biases the passive set toward the trees a peer is short of; whether that biases the HyParView graph's degree distribution (§3.1.3) enough to matter is a Chapter 8 question.

## Validation owed (Chapter 8)

*   Repair latency distribution at $M = 6$ with $\ell \in \{0, 0.5\}$, split by which tier served the repair.
*   Pool occupancy over time under churn; fraction of repairs that find $|\mathcal{P}_m| = 0$.
*   Deputy hit rate (orphan's first `RELAY_JOIN_REQUEST` accepted) with the per-tree roster, against the $\approx 1/M$ baseline.
