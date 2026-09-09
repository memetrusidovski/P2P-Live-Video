# 1. Shuffle and Neighbor Frames

The HyParView membership layer communicates over UDP using lightweight, binary-encoded control frames. This ensures minimal overhead, with gossip traffic consuming less than 1% of the total stream bandwidth.

```text
NEIGHBOR Request Frame (Type 0x05):
Used to request active connection promotion from the Passive Set.
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|   Version     |  Type (0x05)  |          Payload Length       |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                      S/Kademlia Sender NodeID                 |
|                            (32 bytes)                         |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|   Priority (0x01=High, 0x02=Low) | TreeID   | NodeClass | AssignedTrees |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+

SHUFFLE Frame (Type 0x06):
Used to shuffle the Passive Set to discover fresh candidate peers.
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|   Version     |  Type (0x06)  |          Payload Length       |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                      S/Kademlia Original Sender ID            |
|                            (32 bytes)                         |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|      TTL      |   Count (N)   |  WantedTrees  |  Reserved (0) |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                               |
|          N Peer Records (Ch2 §2.3.3 — 42 or 54 bytes each)    |
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+

DISCONNECT Frame (Type 0x07):
Used to gracefully terminate a peer connection and release socket resources.
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|   Version     |  Type (0x07)  |          Payload Length       |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                      S/Kademlia Sender NodeID                 |
|                            (32 bytes)                         |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|  Reason Code  |    TreeID     |          Reserved (0)         |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
  Reason: 0x01 CHOKE, 0x02 QUIT, 0x03 EVICTION, 0x04..0x09 drain reasons (shared with
  DRAIN_NOTICE), 0x0A REJECTED_SATURATED, 0x0B REJECTED_NOT_ASSIGNED, 0x0C REJECTED_DEPTH.
  TreeID: 0 = the whole connection; m > 0 = only the tree-m relationship or request.
```

*   **SHUFFLE records are full Peer Records**, not bare addresses: NodeID, `NodeClass`, `AssignedTrees` and reachability `Flags` travel with every address, so the receiver files each entry into the right per-tree candidate pool (Ch3 §3.1.1) without probing it. `WantedTrees` names the trees the sender is short of relays for. A `SHUFFLE` of 8 IPv6 records is $\approx 470$ bytes — well within the $\le 1\%$ gossip budget.
*   **Two forms of SHUFFLE.** With `TTL > 0` it is the classic HyParView random walk: each receiver integrates the records, decrements `TTL` and forwards to a random active neighbour; the walk has **no reply** — the return path for fresh records is the periodic `GOSSIP_EXCHANGE` every peer already runs with its active neighbours. With **`TTL = 0`, sent over an existing session**, it is a **direct shuffle request**: the receiver answers on the same session, within $\tau_{\text{sched}} = 100$ ms, with a `GOSSIP_EXCHANGE` (Appendix D §D.4.10) whose records are drawn first from its per-tree pools for the trees named in `WantedTrees`. This is the tier-3 repair source of §3.3.1; an earlier draft called it an "urgent `SHUFFLE`" with a priority the frame does not carry and a reply no frame defined, so three implementations could each have chosen a different answer and two of them would have timed out against the third.
*   **NEIGHBOR `TreeID`:** `0x00` requests plain HyParView membership promotion (the classic semantics); a value $m > 0$ requests a **parent slot in tree $T_m$** — this is the `RELAY_JOIN_REQUEST` used by topology healing (Ch1 §1.2.3) when sent with `Priority = High`. `NodeClass` and `AssignedTrees` are the requester's self-description (Appendix D §D.4.3), which the Rank Admission Rule reads. **Every `NEIGHBOR` is answered within $\tau_{\text{sched}}$**: a successful request with an `ACCEPTED` (0x08) frame carrying the acceptor's hop depth (Appendix D §D.4.4), a refused one with a `DISCONNECT` carrying the request's `TreeID` and one of `REJECTED_SATURATED`, `REJECTED_NOT_ASSIGNED` or `REJECTED_DEPTH` (Appendix D §D.4.3b). A refusal scoped to a tree leaves any other relationship on the connection intact. Silence is never an answer; a requester that hears nothing within $\tau_{\text{sched}}$ plus one RTT treats the candidate as unreachable, which is the only outcome that drops it from the passive set.
*   The complete frame-type registry, including `JOIN` (0x03), `FORWARD_JOIN` (0x09), `ACCEPTED` (0x08), and the HMAC-authenticated `GOSSIP_EXCHANGE` (0x14) carrying the Ch3 §3.2.2 session HMAC, is defined in [Appendix D](../../appendix_d_frame_registry.md).
