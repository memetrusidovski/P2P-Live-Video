# SOLUTION-032: Reachability Is a Class Input; Relays Bridge Leaves, Not Relays

**Closes:** ISSUE-032 (Medium); resolves ISSUE-029 item 8 (signaling path and relay discovery)
**Lives in:** `protocol/chapter1/1.2_multi_forest_overlays/5_node_classes.md` (*Reachability Is a Class Input*); `protocol/chapter2/2.1_skademlia_routing/3_bootstrap_sequence.md`; `protocol/chapter6/6.1_udp_hole_punching/1_nat_permutation_matrix.md`, `6.2_quic_ice/1_candidate_gathering.md`, `6.3_emergent_relays/*`; `appendix_d_frame_registry.md` §D.4.14 (`PUNCH_REQUEST` 0x1C)
**Class:** A capability assumed of every node that 15% of nodes lack

---

## The problem in one line

A symmetric-NAT peer hash-assigned as a relay advertised slots nobody could reach; nothing said how a NAT-blocked peer found an emergent relay or how two peers exchanged punch candidates; relayed traffic was not charged to anyone; and the broadcaster was assumed reachable, which a home streamer on CGNAT is not.

## The decision

*   **Three-valued `Reachability`** (`PUBLIC`, `CONE`, `SYMMETRIC`), determined from two seed reflections at bootstrap and carried in every Peer Record and `PROBE_RESPONSE`. **`RELAY` requires `PUBLIC` or `CONE`**; `SYMMETRIC` defaults to `LEAF` and contributes over connections it initiates.
*   **`PUNCH_REQUEST` through the referrer.** The guardian or gossip neighbour that introduced two peers forwards the requester's *observed* address to the target, which opens its NAT with a `PING`. One extra RTT for `CONE` parents; `SYMMETRIC` targets are never probed. No ICE candidate frame exists because no self-declared address is ever trusted.
*   **Emergent relays bridge leaves only**, charged to the relay's PULL reserve $r_{\text{pull}} u_v$ — never to tree slots. There is no "relayed relay", so no slot accounting spans two machines. Relay-capable nodes advertise with `Flags.RELAY_CAPABLE`, register at a denser sample, and are found with `GET_PEERS(ReqFlags.WANT_RELAY_CAPABLE)`. Bridged blocks earn ordinary receipts with a `RELAYED` flag, weighted $3\times$.
*   **The source must be `PUBLIC`/`CONE` or bind ingress relays**, which register with `Flags.SOURCE_INGRESS` and act as hop 0. A source that can do neither cannot broadcast, and the spec says so.

## Why this and not the alternatives

*   **Letting `SYMMETRIC` nodes relay through a bridge** would give the forest their upload — at the cost of the relay carrying $K_v \cdot B_m$ of someone else's children, an accounting that has to be split across two nodes and that doubles the cost of every hop through the bridge. The 15% of upload forgone is cheaper than the mechanism, and a symmetric peer can still contribute as a PULL server.
*   **Full ICE with candidate lists over a signaling frame** was the Ch6 text's intent. Rejected because the protocol's rule is that no peer declares its own address (SOLUTION-012); the referrer already holds the only address that matters.
*   **Charging bridges to tree slots** instead of the PULL reserve: bridging is a service outside the relay's tree duty and would otherwise silently shrink the forest's capacity. The reserve is already budgeted (SOLUTION-025) and bounds the $3\times$ multiplier so it cannot be farmed.

## Defects found during verification

*   Ch6 §6.3.1 required "publicly accessible … No NAT or Full Cone" for relays but nothing tied this to `NodeClass`, so a CGNAT node could declare `RELAY` and pass every check.
*   The 3× multiplier referenced "Relay PoU Receipts" that no frame defined; ordinary receipts with a flag are the definition.
*   The hole-punch matrix says a symmetric child reaches a port-restricted parent at 10%. Most home routers are port-restricted, so "a symmetric peer just connects outbound" is true only toward `PUBLIC` parents — which is why the bridge remains necessary and why `SYMMETRIC` children prefer `PUBLIC` parents.

## The generalisable lesson

**Every physical constraint on a node (reachability, battery, bandwidth) must appear as an input to its role, and every role must be checkable by the peers that depend on it.** Reachability was measured, described in Chapter 6, and consulted by nothing in Chapter 1.

## Residual risk

*   Two-reflection classification misfires on NATs that are endpoint-independent for some destinations and not others; a node misclassified as `CONE` will fail punches and be scored down by reliability, which is the right slow correction.
*   Ingress relays make the root depend on volunteers. Two are recommended; the spec does not yet say what the source does when both leave at once (it re-binds; the swarm sees a root outage of the rebind time).

## Validation owed (Chapter 8)

*   Scenario C (75% behind symmetric NAT): fraction served by direct `PUBLIC` parents versus bridges, and total bridge load against the PULL reserve.
*   Punch success rate for `CONE` parents via the referrer path, against the §6.1.1 matrix.
*   Root availability with one versus two ingress relays under Scenario A churn.
