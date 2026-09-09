# SOLUTION-026: The Forest Ladder Reads Relay Counts the Guardians Actually Report

**Closes:** ISSUE-026 (Medium); resolves ISSUE-029 items 3, 4 and 5 (guardian → publisher path, Stream Record write path, canonical record layout)
**Lives in:** `protocol/chapter1/1.2_multi_forest_overlays/1_graph_theory_and_slicing.md` §1.4 (*The Ladder Input Is $N_{\text{relay}}$*, *Uncovered Trees*); `protocol/chapter2/2.3_stream_registration/3_registration_frames.md` (`STORE_RECORD`, `STORE_RECORD_ACK`, Stream Record layout); `2_store_get_rpcs.md` (*The Publisher's Write Path*); `1_publisher_genesis_key.md`
**Class:** A publisher decision keyed on a count nobody could compute (recurring pattern #4)

---

## The problem in one line

The forest ladder was keyed on `swarm_size` while coverage depends on the relay count, `REGISTER_PEER` carried no class so no guardian could count relays, and there was no frame in either direction between guardians and publisher — so the value the ladder consumed had no producer.

## The decision

*   **`REGISTER_PEER` carries `NodeClass` and `AssignedTrees`** (SOLUTION-028), so guardians count `RelayCount` and `PerTreeRelayCount[m]` over *active* registrations (refreshed within 135 s).
*   **The counts ride on the acknowledgement of the write the publisher already makes.** `STORE_RECORD` (0x1A) stores the Stream Record once per segment; `STORE_RECORD_ACK` (0x1B) returns `ActiveCount`, `RelayCount`, per-tree relay counts and per-tree starved counts. The publisher takes the median over responding guardians and scales by the sampling exponent. This one round-trip is the only guardian → publisher path, and it feeds the ladder, the JOINING threshold, the PoW tier, the source reserve trigger and coverage monitoring.
*   **The Stream Record has a byte-exact layout** (fixed 92 bytes + 7 per tree + signature) that the signature covers; the JSON is illustrative.
*   **Uncovered trees** are handled locally by the coverage grant (SOLUTION-038) and monitored globally through `PerTreeRelayCount`; a persistently empty tree is the publisher's cue to fold its layer via `MANIFEST_UPDATE`.

## Why this and not the alternatives

*   **A separate `STREAM_STATS` request from the publisher** adds a frame pair and a second round-trip per second for information the guardian can attach to a reply it already sends. Guardians know the publisher only by key, so they cannot initiate; piggybacking on the write is the only design that needs no publisher address anywhere.
*   **Deriving $N_{\text{relay}}$ from $N$ and an assumed leaf fraction** was the implicit status quo. With $\ell \approx 0.5$ tolerated by §5.3, the ladder would sit two rungs too high; at $N = 30$, $\ell = 0.5$ the chance of an empty tree was $\approx 33\%$.
*   **Keeping the record as canonical JSON** and specifying a canonicalisation. Rejected: two implementations verifying each other's signatures over JSON is a known interoperability trap; a fixed-field layout is shorter, unambiguous and already the style of every other frame.

## Defects found during verification

*   `swarm_size` over-counted departed peers for up to 180 s because a registration lived for its full TTL. Counting only registrations refreshed within 135 s (1.5 refresh periods) bounds the lag to one refresh period while leaving records servable to the TTL.
*   The source reserve's trigger (SOLUTION-015's `StarvedTreeBitmap`) had a producer but still no consumer path — guardians "aggregate and report to the publisher" through nothing. `StarvedCount[m]` in the ACK is that path.
*   With the publisher taking the median, one stale or hostile guardian cannot move $M$, $\theta_{\text{join}}$ or the PoW tier; with a mean or max it could.

## The generalisable lesson

**For every value a decision consumes, name the frame that produces it and the node that sends that frame.** "Reported by the guardians" was written in three chapters; no chapter said how.

## Residual risk

Counts lag by up to one refresh period (90 s) plus the 135 s activity window on departure; the ladder's hysteresis and 30 s dwell absorb this, but a swarm that halves in a minute will run one rung high for about two.

## Validation owed (Chapter 8)

*   Estimator lag of `RelayCount` against true relay population under 30% churn storms.
*   Whether median-of-20 is robust enough against a guardian set with several hostile members, or whether a trimmed mean over verified-active guardians is needed.
