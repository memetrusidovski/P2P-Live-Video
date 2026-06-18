# 1. Peer State Transition Model

To maintain the stability of the multi-forest overlay under high churn and network jitter, every node executes a standardized state machine. Each state restricts the packet types a peer can process and defines its current obligations to the swarm.

| State | Code | Description | Next State (Triggers) |
| :--- | :--- | :--- | :--- |
| **BOOTSTRAP** | `0x00` | Instantiating keypairs, validating Node ID, loading local candidate cache | `DISCOVERY` (Initial seed loaded) |
| **DISCOVERY** | `0x01` | Querying S/Kademlia DHT for active stream keys and stream publisher info | `JOINING` (Stream records fetched) |
| **JOINING** | `0x02` | Executing HyParView handshakes, populating Active and Passive sets | `CONNECTING` (Active set size $\ge 4$) |
| **CONNECTING**| `0x03` | Initiating Tree Join handshakes to establish parent connections | `ACTIVE` (Parents connected for all $M$ slices) |
| **ACTIVE** | `0x04` | Streaming active video; decoding, verifying, and uploading symbols | `CHURN_REPAIR` (Parent timeout / packet loss) <br> `TERMINATED` (Verified `STREAM_END` gossip received) |
| **CHURN_REPAIR**| `0x05` | Re-routing failed slices to standby candidate peers from Passive Set | `ACTIVE` (Re-route successful) <br> `DISCOVERY` (All parents lost, complete disconnect) <br> `TERMINATED` (Verified `STREAM_END` gossip received) |
| **TERMINATED**| `0x06` | Shutting down connections, releasing sockets, broadcasting disconnect | (None) |

## State Transition Diagram

```text
                  +-------------------+
                  |   0x00 BOOTSTRAP  |
                  +-------------------+
                            |  (Seed node loaded)
                            v
                  +-------------------+
                  |   0x01 DISCOVERY  | <--------------------+
                  +-------------------+                      |
                            |  (Stream records retrieved)    |
                            v                                |
                  +-------------------+                      |
                  |   0x02 JOINING    |                      |
                  +-------------------+                      |
                            |  (Active set populated)        |
                            v                                | (Complete
                  +-------------------+                      |  disconnect)
                  |  0x03 CONNECTING  |                      |
                  +-------------------+                      |
                            |  (Parents connected)           |
                            v                                |
+---------------> +-------------------+                      |
| (Re-route       |    0x04 ACTIVE    | ---------------------+
|  successful)    +-------------------+
|                      |          |
|   (Parent timeout)   |          | (Verified STREAM_END gossip)
|                      v          |
|                 +-------------------+         |
+---------------- |  0x05 CHURN_REPAIR| ---------+
                  +-------------------+          |
                       |        |                |
     (Graceful quit /  |        | (Verified      |
      App close)       v        |  STREAM_END)   v
                  +-------------------+    +-------------------+
                  |  0x06 TERMINATED  |<---|  0x06 TERMINATED  |
                  +-------------------+    +-------------------+
```

## Broadcaster Failure: STREAM_END Frame

When the broadcaster intends to end a stream, it gossips a signed `STREAM_END` frame to its immediate children before closing its socket. Children verify the signature against the pinned $PK_{\text{Source}}$ and forward it to their subtrees using the same HyParView epidemic gossip mechanism (Chapter 3). Any peer receiving a valid `STREAM_END` immediately transitions to `TERMINATED`.

```text
STREAM_END Frame (Type 0x04):
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|   Version     |  Type (0x04)  |          Payload Length       |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                               |
|                   StreamID Key K_s (32 bytes)                 |
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|              Final Segment Sequence Number (4 bytes)          |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|            Timestamp (8-byte microsecond integer)             |
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                               |
|         Ed25519 Source Signature (64 bytes)                   |
|         Sign_{SK_Source}(StreamID || FinalSeqNum || Timestamp)|
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

If the broadcaster disconnects abruptly without sending `STREAM_END`, peers will exhaust their CHURN_REPAIR attempts across all $M$ slices. Once all parents for all slices are lost simultaneously and no DHT re-query returns records (the stream's DHT entries expire after $\tau_{\text{ttl}} = 180\text{ s}$), the peer transitions to `TERMINATED`.
