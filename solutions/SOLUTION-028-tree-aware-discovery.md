# SOLUTION-028: Tree-Aware Discovery and the Peer Record

**Closes:** ISSUE-028 (High); completes the wire half of ISSUE-026
**Lives in:** `protocol/chapter2/2.3_stream_registration/3_registration_frames.md` (Peer Record, `REGISTER_PEER`, `GET_PEERS`), `2_store_get_rpcs.md`; `protocol/chapter1/1.2_multi_forest_overlays/2_parent_selection_algorithm.md` §2.2; `appendix_d_frame_registry.md` §D.4.5, §D.4.6, §D.4.6b, §D.4.7, §D.4.10
**Class:** A call the algorithm made (`get_peers(slice=m)`) that no frame could carry (recurring pattern #4)

---

## The problem in one line

The join algorithm asked the DHT for relays of tree $m$, but `REGISTER_PEER` carried no class or tree list, the `GET_PEERS` request had no tree field, and the record that came back was seven bytes of address — so a joiner probed twenty blind candidates to find the $\approx 1.7$–$3.3$ that could serve it, and a launch handed every joiner the same twenty peers.

## The decision

*   **One Peer Record everywhere**: `{NodeID, NodeClass, AssignedTrees, Flags, address}` — 42/54 bytes — in `GET_PEERS`, `FIND_*`, `SHUFFLE`, `GOSSIP_EXCHANGE`, `FORWARD_JOIN` and the `ROSTER`. The NodeID makes the ranked part of the tree bitmap *verifiable*; the flags carry reachability so unreachable candidates are never probed.
*   **`REGISTER_PEER` carries the same three bytes**, signed in the durable registration statement, re-sent on change. Guardians can now count $N_{\text{relay}}$ and relays per tree — the input ISSUE-026's ladder needed.
*   **`GET_PEERS` takes a `WantedTrees` bitmap** and returns a **uniformly random, per-query, stratified sample** of active relay registrations for those trees; `0x00` is a membership query. Retry means re-query.
*   **Selection is filtered before probing.** The join algorithm probes only records that relay $m$ and are reachable, applies the distinct-parent rule on NodeID, and uses the record NodeIDs to find coverage-grant candidates when no relay of $m$ exists.

## Why this and not the alternatives

*   **Deriving the tree from the NodeID** (carry NodeID only, compute $\arg\max_m \text{Blake3}(NodeID \| m)$) saves one byte and fails for every multi-tree node, every coverage grant and every leaf — the assignment is not a pure function of identity once standing is earned. The bitmap is carried *and* the NodeID lets a reader check its ranked part.
*   **Carrying $K_{\text{avail}}$ in the record** would let joiners skip saturated candidates too. Rejected: it changes every second, the record is re-served for up to 180 s, and a stale free-slot count is worse than none — the probe is where freshness lives.
*   **Guardian-side deterministic selection** ("20 most active") was what the prose implied. Guardians see registrations, not activity, so "active" was undefined; and any deterministic rule herds a launch onto one set of peers. Randomness per query is the property SOLUTION-003's local-$J_{\text{new}}$ argument silently depended on.

## Defects found during verification

*   §1.3's "multi-tree nodes register in the DHT under each assigned tree" and §2.2's `get_peers(slice=m)` were both consuming a field that did not exist; two closed solutions (007, 003) rested on it.
*   The `GET_PEERS` response at 20 IPv6 records plus a 6-tree Stream Record is $\approx 1{,}330$ bytes — inside one MTU, but with no room for a larger `Count`; 20 is a hard cap, not a default.
*   Blind-probe yield arithmetic: at $M = 6$, $\ell = 0.5$, 20 random records contain $\approx 1.7$ relays of the wanted tree *before* removing saturated, warming and unreachable ones. A round with zero usable candidates counted as a failed round, so the shed rule was firing for lack of discovery, not lack of capacity.

## The generalisable lesson

**When an algorithm calls a function on another subsystem, find the frame that implements it and check every argument has a field.** `get_peers(stream_id, slice=m)` looked like an API; it was a wish.

## Residual risk

`AssignedTrees` is self-declared and re-served for up to 180 s; a node that changes assignment and fails to re-register costs other peers a wasted probe. The `REJECTED_NOT_ASSIGNED` correction bounds the damage to one probe per stale record per peer.

## Validation owed (Chapter 8)

*   Candidates-probed-per-successful-join at $M = 6$, $\ell \in \{0, 0.5\}$, before and after filtering.
*   Load distribution over the 20 records returned during a 10,000-viewer launch: the herd is gone only if the guardian's sampler is genuinely uniform.
