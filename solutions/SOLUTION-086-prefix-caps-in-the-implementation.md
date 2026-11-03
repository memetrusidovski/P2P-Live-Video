# SOLUTION-086: Prefix Caps Enforced in the Implementation

**Closes:** ISSUE-086 (High, implementation)
**Lives in:** `src/crates/bs-core/src/dht.rs` (`prefix_key`, `prefix_cap`, `RoutingTable::insert`), `src/crates/bs-core/src/node.rs` (`note_prefix`, `prefix_cap`, `candidates_for`, `on_neighbor`), `src/crates/bs-core/src/node/membership.rs` (`active_add`, `passive_insert`)
**Class:** Specification rule with no implementation (Ch2 §2.2.2, App C §C.2.1)

---

## The problem in one line

The per-prefix caps are the protocol's Sybil and eclipse bound. The reference implementation applied none of them, so one /24 filled a victim's k-buckets, Active Set and parent set.

## The decision

*   `P_obs` is the count of distinct /24 (IPv4) or /48 (IPv6) prefixes the node saw on verified plain frames, Peer Records and Active Set members.
*   `prefix_cap(K) = max(1, ceil(K / min(P_obs, 20)))` bounds, per prefix, the contacts in one k-bucket, the Active Set members, the parents across trees, and the children per tree. A refused child gets `DISCONNECT(REJECTED_SATURATED)`.
*   The k-bucket cap is the diversity-relative form too, not the unconditional 1 of 20 that Ch2 §2.2.2 states. A swarm inside one /24 (a LAN test, a dorm, a Docker fleet) has `P_obs = 1`, so every cap equals the set size and the swarm forms. At `P_obs >= 20` the k-bucket cap is 1, as specified.

## Why this and not the alternatives

*   **The unconditional k-bucket cap**: a single-/24 swarm holds one DHT contact per node and does not bootstrap. SOLUTION-042 made the Active Set cap diversity-relative for exactly that case; the k-bucket was left behind.
*   **Counting prefixes only from the DHT**: a node that joined through gossip alone would never see twenty prefixes and never tighten its caps. Every verified source counts.

## Residual risk

*   `P_obs` only grows. A node that once saw twenty prefixes keeps the tight caps if its swarm later shrinks into one prefix; a restart resets it.
*   The source is exempt from the parent cap, as the spec's own candidate rule exempts it.

## Validation owed

*   A simulator scenario with one /24 attacker against a 1,000-peer swarm (Ch8 §8.3 Sybil), measuring the attacker's share of k-buckets, Active Sets and parent slots.
