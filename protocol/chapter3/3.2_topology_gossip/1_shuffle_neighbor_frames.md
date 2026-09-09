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
|   Priority (0x01=High, 0x02=Low) | TreeID   | Reserved        |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+

SHUFFLE Frame (Type 0x06):
Used to shuffle the Passive Set to discover fresh candidate peers.
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|   Version     |  Type (0x06)  |          Payload Length       |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                      S/Kademlia Original Sender ID            |
|                            (32 bytes)                         |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|   Time-to-Live (TTL)          | Number of Shuffled Nodes (N)  |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                               |
|        List of N Shuffled Node ID Blocks (IP + Port)          |
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
|   Reason Code (0x01=Choke, 0x02=Quit, 0x03=Eviction)          |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

*   **NEIGHBOR `TreeID`:** `0x00` requests plain HyParView membership promotion (the classic semantics); a value $m > 0$ requests a **parent slot in tree $T_m$** — this is the `RELAY_JOIN_REQUEST` used by topology healing (Ch1 §1.2.3) when sent with `Priority = High`. Successful requests are answered with an `ACCEPTED` (0x08) frame carrying the acceptor's hop depth (Appendix D §D.4.4).
*   The complete frame-type registry, including `JOIN` (0x03), `FORWARD_JOIN` (0x09), `ACCEPTED` (0x08), and the HMAC-authenticated `GOSSIP_EXCHANGE` (0x14) carrying the Ch3 §3.2.2 session HMAC, is defined in [Appendix D](../../appendix_d_frame_registry.md).
