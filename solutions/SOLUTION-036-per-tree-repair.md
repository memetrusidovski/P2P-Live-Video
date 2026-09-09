# SOLUTION-036: Repair Is a Per-Tree Sub-State; Forwarding Never Stops

**Closes:** ISSUE-036 (High)
**Lives in:** `protocol/chapter1/1.3_peer_lifecycle/1_transition_model.md` (state table, *Repair Is Per Tree*, diagram), `2_algorithmic_core_loop.md`; `protocol/chapter3/3.3_churn_recovery/2_active_probing.md`; `appendix_a_sequence_diagrams.md` A.2
**Class:** A single-tree state machine wrapped around a multi-tree node (recurring pattern #8)

---

## The problem in one line

`CHURN_REPAIR` was a node-level state in which the core loop ran only the re-route, so a relay whose tree-3 parent died stopped forwarding tree-1 data to its children; they saw silence, evicted it, entered repair themselves, and stopped forwarding — one dead parent became a churn wave down every tree of every affected subtree.

## The decision

`CHURN_REPAIR` is a **sub-state of `ACTIVE` scoped to one tree**. The node keeps $\mathcal{T}_{\text{repairing}} \subseteq \mathcal{T}_{\text{sub}}$; for every tree not in it, receiving, verifying, forwarding, PULL service, rosters, receipts and heartbeats continue without interruption. The node leaves `ACTIVE` only when its *last* parent is gone. Losing every base-layer parent stalls playback, not relaying. The core loop is rewritten accordingly, and the Ch3 §3.3.2 algorithm — which was always written per slice — is now what Ch1 §1.3 also says.

## Why this and not the alternatives

*   **Node-level `CHURN_REPAIR` only when Tree 1 is lost** (the issue's suggestion) still stops forwarding on the trees that *do* have parents. Playback and relay duty are independent; the state machine should not couple them even for the base layer.
*   **Renumbering the state codes** to remove `0x05` was rejected: the code stays, as the sub-state's identifier, so nothing that references the enum breaks.

## Defects found during verification

*   The cascade arithmetic: one failure at depth $d$ produced a wave with one detection period per level; with $D_{\max} = 8$ and $\tau_{\text{evict}} \approx 240$ ms that is up to $\approx 1.7$ s of subtree-wide outage across all $M$ trees — for a fault the forest was designed to confine to $1/M$ of the bitrate for $\approx 300$ ms.
*   The state table's own sentence — "each state restricts the packet types a peer can process" — confirmed the pseudocode reading was the intended semantics, not an abbreviation.
*   With heartbeats (SOLUTION-022) a repairing relay keeps its children's connections warm while it re-attaches; without them, even per-tree repair would have looked like death to children of the repairing tree. The two fixes are load-bearing for each other.

## The generalisable lesson

**A node with $M$ independent roles needs $M$ independent failure states; a single state machine over the node reintroduces exactly the correlation the roles were separated to avoid.**

## Residual risk

`DISCOVERY` on "every parent lost" is now a rarer transition and the only path back for a fully disconnected node; a node with one surviving low-value parent (an $L_2$ stripe) stays in `ACTIVE` repairing five trees rather than falling back to a fresh DHT walk, which is right when the pools are good and slow when they are empty. Tier 4 of Ch3 §3.3 (a per-tree `GET_PEERS`) covers this without leaving `ACTIVE`.

## Validation owed (Chapter 8)

*   Scenario A (30% of Layer-1/2 relays fail at once): subtree outage duration and blast radius with per-tree repair versus node-level, in trees other than the failed one.
