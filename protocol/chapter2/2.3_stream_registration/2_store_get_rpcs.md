# 2. STORE and GET RPCs

The Distributed Hash Table (DHT) acts as the decentralized ledger for stream lookup. Unlike standard static torrent files, live streaming streams require a dynamic peer lookup system to track active uploaders as viewers continuously join and leave.

These "RPCs" are **plain-UDP request/response frames** (`REGISTER_PEER` 0x0A, `GET_PEERS` 0x0B, `STORE_RECORD` 0x1A — see Appendix D), not remote procedure calls over a session protocol: they precede any QUIC session and therefore each carries the full 152-byte S/Kademlia validation block (Ch2 §2.2.3).

```text
      [ Joining Peer ]                      [ DHT Guardian Node ]                 [ Publisher ]
             |                                        |                                 |
             | ---- REGISTER_PEER(K_s, class, trees)->|                                 |
             |                                        | (Verifies PoW + 2 signatures,   |
             |                                        |  binds to observed address)     |
             | <--- REGISTER_RESPONSE(200 OK) --------|                                 |
             |                                        |                                 |
             |                                        |<--- STORE_RECORD(StreamRecord)--|  every 1 s
             |                                        |---- STORE_RECORD_ACK(counts) -->|
```

## Who Is a Guardian

The guardians of a stream are the $k = 20$ DHT nodes closest to $K_s$. They are ordinary peers, and at $N = 10^6$ they handle every registration and every discovery query for the stream — so *which* peers can be guardians matters. Two rules keep the role on nodes that can carry it:

1.  **Only reachable relay-class nodes are DHT storage nodes.** A peer inserts a contact into a k-bucket only after that contact has answered an **unsolicited** `PING` — the classic Kademlia liveness check, which here doubles as an inbound-reachability check. Nodes of class `LEAF` / `LEAF_PRIVATE`, and any node whose `Reachability` is `SYMMETRIC` (Ch2 §2.1.3), operate the DHT in **client mode**: they issue lookups, registrations and queries but do not answer `FIND_NODE`, `REGISTER_PEER`, `GET_PEERS` or `STORE_RECORD`, and are therefore never inserted into anyone's buckets and never become guardians. A phone on cellular is a DHT client, not a DHT server.
2.  **Registration is sampled above $N_0 = 10^4$ peers** (below), so guardian load is bounded regardless of $N$.

## Stream Registration Protocol (`REGISTER_PEER`)

To register as an active peer for a specific stream:
1.  The peer calculates the target 256-bit stream index:
    $$K_s = \text{Blake3}(\text{StreamID})$$
2.  The peer sends a `REGISTER_PEER` frame containing its NodeID, listening Port, its **`NodeClass`, `AssignedTrees` bitmap and `Flags`** (§2.3.3), and a signed registration statement. The statement deliberately carries **no self-declared IP address** — consistent with the validation block rule of Ch2 §2.2.3, the guardian binds the registration to the **observed UDP source address** of the packet:
    $$\text{Payload} = K_s \parallel \text{NodeID} \parallel \text{Port} \parallel \text{NodeClass} \parallel \text{AssignedTrees} \parallel \text{Flags} \parallel \text{Timestamp}$$
    $$\text{Sig} = \text{Sign}_{SK_{\text{node}}}(\text{Payload})$$
3.  The DHT guardian nodes closest to $K_s$ receive the packet, verify the S/Kademlia Proof-of-Work **against the observed source address**, validate both signatures, and store the peer's record under that observed address.
4.  The peer refreshes its registration every $\tau_{\text{ttl}}/2 = 90$ s, and **immediately** whenever `NodeClass`, `AssignedTrees` or `Flags` change. A registration counts as **active** for $135$ s after its last refresh; only active registrations are counted **and only active registrations are returned** by `GET_PEERS`. The guardian *retains* an inactive registration until $\tau_{\text{ttl}} = 180$ s so that a late refresh re-activates it without a fresh identity check; a retained-but-inactive registration is neither counted nor served. (An earlier draft called the $135$–$180$ s band "servable", which contradicted "only active registrations are returned"; nothing is served from it.)
5.  **A `CONE` registrant keeps its guardian mappings alive.** A guardian forwards `PUNCH_REQUEST`s to a `CONE` relay (Appendix D §D.4.14) through the UDP mapping that relay's own traffic to the guardian opened. Restricted and port-restricted NATs expire idle UDP mappings well inside the 90 s refresh interval on many deployments — RFC 4787 asks for $\ge 2$ min but common consumer and carrier NATs use $30$–$60$ s — so a mapping refreshed only by registration is dead for most of every refresh period, and every punch through it fails silently. A `CONE` relay therefore sends a `PING` (§D.4.1) to each guardian it registered with every $\tau_{\text{nat}} = 25$ s. Cost at $N = 10^6$, $s = 7$: roughly $7{,}800 \times 0.6$ (the `CONE` share) $\times 20 / 25 \approx 3{,}700$ pps swarm-wide, under $200$ pps per guardian — inside the guardian's unknown-source budget (Ch7 §7.2.1) and a small fraction of its registration and query load. `PUBLIC` registrants need no keepalive; out-of-sample peers are referred by gossip neighbours, who hold an open session to them.

**Why the frame carries two signatures.** The 152-byte validation block already contains an Ed25519 packet signature, so the separate registration signature looks redundant — it is not, and an implementation must not strip it. They have different lifetimes:

*   The **validation-block signature** authenticates *this datagram*, covering the timestamp for replay protection. It is meaningful only to the guardian that received the packet, at the moment it arrives.
*   The **registration signature** authenticates a *durable statement* — including the class and tree bitmap — that the guardian stores for the registration's lifetime. Its consumer is the **guardian**, which can later prove to a third party what the peer declared: two registration statements signed by one key with the same timestamp and different class or trees are the `EQUIVOCATION` evidence of Ch5 §5.3.3. The Peer Record the guardian re-serves is *not* signed — it carries the guardian's observation of the address and 42–54 bytes of the peer's declaration, and a reader trusts it exactly as far as it trusts the guardian or gossiper that sent it. An earlier draft claimed the record itself let third parties verify the registration; no frame carried the signature to them, and adding it would put a 20-record IPv6 `GET_PEERS` response over the MTU.

### Sampled Registration

Every registration lands on the same 20 guardians. Un-sampled, at $N = 10^6$ that is $10^6 / 90\text{ s} \approx 11{,}000$ registrations per second per guardian: $\approx 22{,}000$ Ed25519 verifications per second, $\approx 23$ Mbps inbound, $\approx 110$ MB of state — and, because registrations are pre-session UDP from unknown sources, more than twice the guardian's own XDP new-connection ceiling of $5{,}000$ pps (Ch7 §7.2), so the guardian's own DDoS shield would drop over half of them. Discovery needs 20 candidates, not a census.

The publisher therefore sets **`RegisterSampleLog2`** $= s$ in the Stream Record, and a peer registers **only if**

$$\left(\text{Blake3}(NodeID)[0{:}2]\right) \bmod 2^{s} = 0$$

i.e. a deterministic $2^{-s}$ sample keyed on identity, so a peer cannot choose to be in or out. The publisher chooses $s = \max\left(0, \left\lceil \log_2 (N_{\text{est}} / N_0) \right\rceil\right)$ with $N_0 = 10^4$, and estimates $N_{\text{est}} = \text{median}_{\text{guardians}}(\text{ActiveCount}) \cdot 2^{s}$ from the `STORE_RECORD_ACK` counts. At $N = 10^6$, $s = 7$: $\approx 7{,}800$ registrations, $\approx 90$ per second per guardian, under 1 MB of state.

Three exceptions always register regardless of the sample, because discovery *specifically* needs them: the **source** (`Flags.SOURCE`), any **ingress relay** it has bound (`Flags.SOURCE_INGRESS`), and **emergent-relay-capable** nodes (`Flags.RELAY_CAPABLE`), which use $\max(0, s - 3)$ — an $8\times$ denser sample, since they are rare and NAT-blocked peers must be able to find one.

A peer outside the sample is not invisible: it is discovered through HyParView gossip (`SHUFFLE`, `GOSSIP_EXCHANGE`, Ch3), which carries the same Peer Record and scales with $c_p$, not with $N$. The DHT sample is the *entry point*; the gossip layer is the *steady state*.

### Query Discipline

`GET_PEERS` load must be bounded the same way. A peer queries the DHT at join, and afterwards **only** when a per-tree candidate pool is thin (Ch3 §3.1.1: fewer than 3 known relays for a subscribed tree), in which case it re-queries that tree at most every $30$ s. There is no unconditional periodic re-query; registration refresh is `REGISTER_PEER`, not `GET_PEERS`. At $10^6$ peers with 5-minute mean sessions this leaves $\approx 3{,}300$ joins per second spread over the guardians, a few hundred queries per second each.

## Stream Discovery Protocol (`GET_PEERS`)

To fetch a list of active uploaders:
1.  The client sends a `GET_PEERS` request targeting $K_s$ with a **`WantedTrees` bitmap** naming the trees it needs parents for (`0x00` for a HyParView membership sample), and its `StarvedTrees` bitmap if any tree has defeated it (Ch1 §1.1.5 §5.4).
2.  The guardian returns a **uniformly random sample** of up to 20 active `RELAY`-class registrations whose `AssignedTrees` intersects `WantedTrees`, stratified across the wanted trees, as Peer Records carrying NodeID, class, tree bitmap, reachability flags and observed address (§2.3.3). Leaf-class registrations are returned only for membership queries. The sample is drawn afresh for every query.
3.  The response also carries the publisher's current signed **Stream Record** (see [1. Publisher Genesis Key](1_publisher_genesis_key.md)), giving the joining client the live-edge segment ID, current swarm and relay counts, the sampling exponent and the slicing matrix in the same round-trip — no additional lookup is needed before live-edge synchronization (Ch4 §4.3) can begin.

Three properties of this design are load-bearing:

*   **Randomness is on the guardian, per query.** A deterministic selection (newest, longest-lived, XOR-closest) would hand every joiner in a launch the same 20 peers, each of which then absorbs the entire join burst and forces a HyParView eviction on every `JOIN` (Ch3 §3.1.2). The local-$J_{\text{new}}$ argument of Ch5 §5.1.3 — that joiners are spread across the peers their own lookups return — is true only because the sample is random.
*   **Retry means re-query.** `FAILURE_RETRY_BACKOFF` (Ch1 §1.2.2) re-issues `GET_PEERS` and unions the fresh sample with the peer's own per-tree pool; re-probing the same 20 records can never discover a relay that finished warming somewhere else.
*   **Yield is filtered before probing.** A joiner probes only records that relay the wanted tree and are reachable from it. With `WantedTrees` naming one tree, all 20 records are candidates for it, against $\approx 1.7$–$3.3$ of 20 under blind sampling.

## The Publisher's Write Path (`STORE_RECORD`)

The publisher stores its Stream Record on the guardians of $K_s$ once per segment with `STORE_RECORD` (§2.3.3), signed by $SK_{\text{Publisher}}$; a guardian accepts it iff the signature verifies against the key hashing to $K_s$ and the record is not older than the one it holds. The **`STORE_RECORD_ACK`** returns the guardian's raw counts — active registrations, relay registrations, relays per tree, distinct `GET_PEERS` senders in the last 10 s, and starved-tree indications from those queries. The publisher takes the median across responding guardians and from that single input drives:

| Consumer | Input |
| :--- | :--- |
| Forest ladder $M$ (Ch1 §1.2.1 §1.4) | `RelayCount` $\cdot 2^{s}$ |
| Adaptive JOINING threshold (Ch1 §1.3), PoW tier (Ch2 §2.2) | `ActiveCount` $\cdot 2^{s}$, published as `SwarmSize` |
| Layer fold (Ch1 §1.1.5 §5.6) and base-layer source reserve (§5.4) | $\hat{s}_m = (k/\alpha) \cdot \text{StarvedCount}[m] / (\text{ActiveCount} \cdot 2^{s})$ — **`StarvedCount` is not scaled**: queries are not sampled, registrations are |
| Coverage monitoring | `PerTreeRelayCount[m]` $\cdot 2^{s}$ — a persistently empty tree that grants have not covered is a signal to fold its layer (Ch1 §1.1.5 §5.6) |

Only the three registration counts are scaled by $2^{s}$. An earlier draft scaled every count, which at $N = 10^6$ read starvation $128\times$ too high.

Guardians know the publisher only by its key, so they never initiate contact; the counts ride on the acknowledgement of a write the publisher makes anyway. One round-trip, once per segment, resolves every "guardians report to the publisher" dependency in the specification.

## The Source Is Discoverable Like Any Peer

The source registers under $K_s$ as an ordinary `RELAY`-class peer with `Flags.SOURCE` set and `AssignedTrees` covering every tree, and is exempt from sampling. At $N = 2$ the first viewer's `GET_PEERS` returns exactly one record — the source — and the join proceeds with no special case. A source that is itself NAT-blocked binds an ingress relay, which registers with `Flags.SOURCE_INGRESS` on its behalf (Ch6 §6.3.3).
