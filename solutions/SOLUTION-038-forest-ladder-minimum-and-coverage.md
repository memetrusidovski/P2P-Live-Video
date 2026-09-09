# SOLUTION-038: Forest Ladder Starts at $M = 2$; Coverage Grants for Empty Trees

**Closes:** ISSUE-038 (Medium); supersedes the $M = 1$ rung and the $N$-keyed ladder of SOLUTION-002
**Lives in:** `protocol/chapter1/1.2_multi_forest_overlays/1_graph_theory_and_slicing.md` §1.4; `appendix_b_parameters.md` ($M_{\min}$, coverage grant)
**Class:** A ladder sized for one cost (birthday coverage) without weighing the cost it created (slot granularity)

---

## The problem in one line

At $M = 1$ every relay with usable upload below one full stream ($\approx 6.9$ Mbps with overhead) had $K_v = 0$ and relayed nothing, and every peer had a single parent for 100% of the stream — in the regime where recovery is weakest.

## The decision

*   **The ladder begins at $M = 2$.** For a source plus four 5 Mbps viewers, $M = 1$ costs the source $24$ Mbps of egress (every viewer is a dead end) and $M = 2$ costs $\approx 12$–$13$ Mbps; for 10 Mbps viewers both cost $6$ Mbps. $M = 2$ is never worse, roughly $2\times$ better for the common upload class, identical at $N = 2$, and gives every peer two independent parents. The "splitting overhead" the $M = 1$ rung was meant to avoid is one QUIC connection.
*   **The ladder is keyed on $N_{\text{relay}}$**, the active `RELAY`-class registration count reported by the guardians (Ch2 §2.3.2 — the wire delivery is completed with ISSUE-026 in the discovery cluster). Registrations count as active only if refreshed within $135$ s.
*   **Coverage grant.** A joiner that discovers *no* relay for tree $m$ may ask a relay whose **second-ranked** rendezvous tree is $m$; the relay accepts iff it can hold at least one slot in each of two trees, then acts as a $t_v = 2$ node for as long as it has children in $m$. One granted tree per relay; the source is the last resort, not the first.

## Why this and not the alternatives

*   **Fixed $M = 6$ at every $N$ with a 1 Mbps slice unit** (the issue's first option) gives the finest slot granularity but relies entirely on grants and the source for coverage below $N_{\text{relay}} \approx 18$, and at $t_v > 2$ a 5 Mbps relay is down to one slot per tree — chains. Rejected as a re-design of SOLUTION-002 for a Medium-priority defect; the source-egress table shows $M = 2$ captures most of the gain.
*   **Choosing $M$ from advertised $\sum K_v$** would need every registration to carry an upload class and the guardians to sum it; the value is self-declared and becomes an input to a publisher decision. Keying on $N_{\text{relay}}$ uses a class the node already declares and that guardians already count. Deferred; noted as a Chapter 8 question.
*   **Publisher-side folding of an empty tree's layer into a covered tree** (ISSUE-026's first option) needs the publisher to learn about the empty tree — one guardian round-trip plus a `MANIFEST_UPDATE` and its 5-segment window — to fix a condition a joiner can resolve locally in one RTT. The grant is local and deterministic; folding remains available to the publisher as a matrix change if grants fail.

## Defects found during verification

*   SOLUTION-002's rationale assumed the only cost of a larger $M$ was empty trees. Slot granularity is a second cost that dominates whenever viewer uploads are below $B$ — which for the residential distributions §1.1.5 calls "the common case" is most of the time.
*   The unchoker's special case for $K_v \le 1$ (SOLUTION-003) was treating the symptom; the cause was the slice size. The special case still applies for uploaders between $3.8$ and $7.7$ Mbps at $M = 2$, and the text now says so.

## The generalisable lesson

**When a ladder is sized against one failure mode, evaluate the other failure modes at every rung before writing it down.** "No splitting overhead" was true and irrelevant; the relevant quantity — source egress as a function of $(N, \text{upload distribution})$ — was never computed for $M = 1$.

## Residual risk

*   At $M = 2$ the slot cut is $\approx 3.8$ Mbps usable upload. Uploaders below that (some LTE and DSL) still contribute nothing until $N_{\text{relay}} \ge 12$. This is accepted for the first minutes of a stream.
*   The grant makes a relay's assignment depend on something other than its NodeID, so it cannot be predicted by a third party — a joiner learns of granted trees only through the relay's `AssignedTreeBitmap`. That is intended, but it means grants are invisible to peers holding a stale record.

## Validation owed (Chapter 8)

*   Source egress versus $(N, \text{upload distribution})$ for $M = 2$ versus $M = 6$-with-grants at $N_{\text{relay}} \le 12$ — the measurement that would justify moving further toward a fixed fine slice.
*   Grant uptake and duration: how often a granted tree persists after the swarm grows enough to cover it by rank.
