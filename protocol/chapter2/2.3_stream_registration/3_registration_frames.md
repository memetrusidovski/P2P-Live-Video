# 3. Registration Frame Layouts

## The Peer Record

Every place the protocol hands one peer a description of another — `GET_PEERS` responses, `FIND_NODE` results, `SHUFFLE`, `GOSSIP_EXCHANGE`, the `FORWARD_JOIN` origin — uses the same **Peer Record**. An address alone is not a usable description of a peer in a multi-forest: a joiner needs to know *before probing* whether a peer relays the tree it wants, whether it is leaf-class, and whether it can be reached at all. Without those three facts every candidate list must be probed blind, and the fraction of useful candidates is $(1 - \ell)/M$ — one in twelve at $M = 6$ with half the swarm on phones.

```text
PeerRecord (IPv4: 42 bytes / IPv6: 54 bytes)
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                               |
|                 S/Kademlia NodeID (32 bytes)                  |
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|   NodeClass   | AssignedTrees |     Flags     |  AddrFamily   |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                 IP Address (4 bytes if 0x04, 16 if 0x06)      |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|            Port (2 bytes)     |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

*   **`NodeClass`:** `0x00` `RELAY`, `0x01` `LEAF`, `0x02` `LEAF_PRIVATE` (Ch1 §1.2.5).
*   **`AssignedTrees`:** bit $m - 1$ set means the peer currently relays tree $T_m$ — its rendezvous assignment plus any coverage grant (Ch1 §1.2.1). `0x00` for leaf classes. With the NodeID also present, a reader can *verify* the ranked part of the bitmap and detect a node claiming trees it was not assigned.
*   **`Flags`:**

    | Bits | Field | Meaning |
    | :---: | :--- | :--- |
    | 0–1 | `Reachability` | `00` `PUBLIC` (no NAT / full cone), `01` `CONE` (restricted or port-restricted: reachable after a hole punch, §D.4.14), `10` `SYMMETRIC` (symmetric / CGNAT: not reachable inbound), `11` reserved |
    | 2 | `RELAY_CAPABLE` | Meets the emergent-relay criteria of Ch6 §6.3.1 and accepts `RELAY_BIND` |
    | 3 | `SOURCE` | This peer is the broadcaster |
    | 4 | `SOURCE_INGRESS` | This peer is an ingress relay bound by a NAT-blocked broadcaster (Ch6 §6.3.3) |
    | 5–7 | reserved | Must be zero |

*   **`AddrFamily`:** `0x04` = IPv4 (record is 42 bytes total), `0x06` = IPv6 (54 bytes). Receivers skip unknown families using the enclosing frame's Payload Length.

The record is what its *sender* knows. A guardian fills it from the peer's own `REGISTER_PEER` (below), the address being the **observed** UDP source of that registration; a gossiping peer fills it from the `JOIN`/`NEIGHBOR` handshake and the observed address of its session. No peer ever writes its own address into a record (Ch2 §2.2.3).

## `REGISTER_PEER` (0x0A)

```text
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|   Version     |  Type (0x0A)  |          Payload Length       |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                               |
|                   StreamID Key K_s (32 bytes)                 |
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                               |
|                     S/Kademlia ID Validation Block            |
|                    (152 bytes — see Ch2 §2.2.3)               |
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|   Port Number (16-bit)        | Protocol Type | NodeClass     |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
| AssignedTrees |     Flags     |          Reserved (0)         |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                               |
|               Ed25519 Registration Signature (64 bytes)       |
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

*   **Registration Signature** covers $K_s \parallel \text{NodeID} \parallel \text{Port} \parallel \text{NodeClass} \parallel \text{AssignedTrees} \parallel \text{Flags} \parallel \text{Timestamp}$, with `Timestamp` taken from the validation block. It is the durable statement the guardian re-serves inside Peer Records (see [2. STORE and GET RPCs](2_store_get_rpcs.md) for why the frame carries two signatures).
*   **`NodeClass`, `AssignedTrees`, `Flags`** are the peer's self-description and are copied verbatim into the Peer Record guardians serve. A peer **must** re-register whenever any of them changes (class upgrade, a coverage grant, multi-tree standing earned or lost, reachability re-evaluated); a stale bitmap costs other peers a wasted probe.
*   **Protocol Type:** `0x01` = UDP.

## `GET_PEERS` (0x0B)

```text
Request (plain UDP):
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|   Version     |  Type (0x0B)  |          Payload Length       |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|            S/Kademlia ID Validation Block (152 bytes)         |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                   StreamID Key K_s (32 bytes)                 |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
| StarvedTrees  | WantedTrees   |   ReqFlags    |  Reserved (0) |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+

Response (plain UDP):
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|   Version     |  Type (0x0B)  |          Payload Length       |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                      S/Kademlia Guardian ID                   |
|                            (32 bytes)                         |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|  Count (N)    |  RespFlags    |    StreamRecordLength (L)     |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                               |
|                 N Peer Records (42 or 54 bytes each)          |
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                               |
|      Publisher Stream Record (L bytes, 0 if unavailable)      |
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

*   **`StarvedTrees`:** bit $m - 1$ set means "I have failed to obtain a parent in $T_m$" — the starved-tree indication the source's base-layer reserve depends on (Ch1 §1.1.5 §5.4). Guardians count distinct querying NodeIDs per bit over a sliding 10 s window and report the counts to the publisher (`STORE_RECORD_ACK`, below).
*   **`WantedTrees`:** bit $m - 1$ set requests relays of $T_m$. The guardian returns a **uniformly random sample of active `RELAY`-class registrations whose `AssignedTrees` intersects `WantedTrees`**, stratified so that each wanted tree receives $\lceil 20 / \text{popcount}(\text{WantedTrees}) \rceil$ records where it can. `WantedTrees = 0x00` is a *membership* query: a uniformly random sample of active registrations of any class, used by `JOINING` to seed HyParView. Every query draws a fresh sample, so a retry after `FAILURE_RETRY_BACKOFF` sees different peers.
*   **`ReqFlags`:** bit 0 `WANT_RELAY_CAPABLE` — restrict the sample to records with `Flags.RELAY_CAPABLE` (used by NAT-blocked peers seeking an emergent relay, Ch6 §6.3). Other bits reserved.
*   **`Count`** $\le 20$. **`RespFlags`:** bit 0 `SAMPLED` — the guardian holds only a registration sample (see *Sampled Registration* in §2.3.2) and `SwarmSize` in the record is an estimate.
*   **`StreamRecordLength (L)`:** the publisher's signed Stream Record (layout below) is appended to every response, so a joining peer obtains the live-edge anchor, `swarm_size`, `relay_count` and the current slicing matrix in the same round-trip (Ch2 §2.3.1, Ch4 §4.3.1). $L = 0$ means the guardian holds no current record — the joiner must then treat swarm-global values as unknown rather than assuming defaults.

At 20 IPv6 records plus a 6-tree Stream Record carrying a pending 6-tree matrix and the descriptor hash the response is $\approx 1{,}400$ bytes, inside a 1500-byte MTU.

## `STORE_RECORD` (0x1A) / `STORE_RECORD_ACK` (0x1B)

The publisher writes its Stream Record to the guardians once per segment; the acknowledgement is where the guardians return the counts the publisher's decisions run on — this single round-trip is the **only** guardian → publisher path in the protocol.

```text
STORE_RECORD (plain UDP, publisher → each guardian of K_s):
[Header][Validation Block 152B][Stream Record (variable, self-signed — layout below)]

STORE_RECORD_ACK (plain UDP, guardian → publisher):
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|   Version     |  Type (0x1B)  |          Payload Length       |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                      S/Kademlia Guardian ID (32 bytes)        |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|   Accepted    |  RespFlags    |      QueryCount (2 bytes)     |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                     ActiveCount (4 bytes)                     |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                     RelayCount (4 bytes)                      |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|      PerTreeRelayCount[1]     |      PerTreeRelayCount[2]     |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|              ...  PerTreeRelayCount[3..6] (2 bytes each) ...  |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|      StarvedCount[1]          |      StarvedCount[2]          |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|              ...  StarvedCount[3..6] (2 bytes each) ...       |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

*   A guardian accepts a `STORE_RECORD` only if the record's signature verifies against the key that hashes to $K_s$ and its `ManifestVersion`/`LiveEdgeSegmentId` are not lower than the record it holds (`Accepted = 0x01`); otherwise `0x00`.
*   **`ActiveCount` / `RelayCount`** are the guardian's raw counts of *active* registrations (refreshed within 135 s, Appendix B) and of those with `NodeClass = RELAY`. **`PerTreeRelayCount[m]`** is the number of active relay registrations with bit $m - 1$ set. **`QueryCount`** is the number of distinct NodeIDs that sent this guardian a `GET_PEERS` in the last 10 s, and **`StarvedCount[m]`** the number of those whose request set `StarvedTrees` bit $m - 1$. All counts are raw. When registration is sampled (§2.3.2) the publisher scales the three **registration** counts by $2^{s}$; `QueryCount` and `StarvedCount` count queries, which every peer sends regardless of the sample, and are **never** scaled.
*   The publisher takes the **median** over the guardians that answer, so one stale or hostile guardian cannot move the forest ladder, the JOINING threshold or the PoW tier.

## The Stream Record — Canonical Layout

The Stream Record is signed and re-served by third parties, so it needs one byte-exact encoding. The JSON in [1. Publisher Genesis Key](1_publisher_genesis_key.md) is illustrative; **this is the specification.**

```text
[PublisherPubKey 32B][ManifestVersion 4B][SlicingMode 1B][NumTrees 1B]
[RegisterSampleLog2 1B][NumTreesNext 1B][SwarmSize 4B][RelayCount 4B]
[LiveEdgeSegmentId 4B][LiveEdgeManifestHash 32B][LiveEdgeTimestamp 8B µs][EffectiveSegmentSeq 4B]
[DescriptorVersion 4B][DescriptorHash 32B]
[TreeID 1B][Layer 1B][StripeIndex 1B][StripeCount 1B][Priority 1B][BitrateKbps 2B]  × NumTrees      (matrix in force)
[TreeID 1B][Layer 1B][StripeIndex 1B][StripeCount 1B][Priority 1B][BitrateKbps 2B]  × NumTreesNext  (pending matrix)
[Ed25519 Publisher Signature 64B]
```

*   Fixed part 132 bytes; $\le 280$ bytes at $M = 6$ with a pending 6-tree matrix, $\le 238$ with none. `stream_id` is not carried — it is $\text{Blake3}(\text{PublisherPubKey})$, and a verifier derives it.
*   **`DescriptorVersion` / `DescriptorHash`:** the version and Blake3 hash of the `STREAM_DESCRIPTOR` (Appendix D §D.4.20) in force — the codec, container and initialisation data a decoder needs before it can use a single verified block. The joiner learns *which* descriptor it needs in the round-trip that anchors it and fetches the body from its first parent with `MANIFEST_REQUEST(0xFD)`, in the same request as its segment-$X$ manifests (Ch4 §4.3.1). A descriptor change is announced five segments ahead like a matrix change; the record points at the version in force and peers hold the pending one too.
*   **`SlicingMode`:** `0x01` `SVC_SPATIAL`, `0x02` `MDC`. The first per-tree block is the slicing matrix **in force** — the one every `MANIFEST` currently on the wire refers to.
*   **`EffectiveSegmentSeq` / `NumTreesNext`:** while a `MANIFEST_UPDATE` is pending (Ch1 §1.2.4 §4.5), `EffectiveSegmentSeq` is its switch segment and the second per-tree block is the matrix that takes effect there; otherwise both are $0$ and the second block is empty. A peer joining inside the migration window thereby holds *both* matrices — without the one in force it could not map a missing block of the manifests it is receiving to a tree, and without the pending one it would be surprised at the switch. The signed frame itself is available on request as `MANIFEST_REQUEST(ChunkIndex = 0xFE)` (Appendix D §D.4.16).
*   **`RegisterSampleLog2` ($s$):** peers register only if their NodeID falls in a $2^{-s}$ sample (§2.3.2). `SwarmSize` and `RelayCount` are already scaled by the publisher.
*   The signature covers every preceding byte. Guardians and clients reject a record whose signature does not verify against `PublisherPubKey`, or whose `PublisherPubKey` does not hash to the $K_s$ it was stored under.

## Notes

*   **Two signatures in `REGISTER_PEER`:** the validation block's packet signature and the registration signature serve different purposes and both are required — see [2. STORE and GET RPCs](2_store_get_rpcs.md).
*   **No self-declared IP:** no frame here carries a sender IP field. Guardians bind registrations to the observed UDP source address (Ch2 §2.2.3), and that is the address they place in Peer Records.
