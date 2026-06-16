# 1. Peer State Transition Model

To maintain the stability of the multi-forest overlay under high churn and network jitter, every node executes a standardized state machine. Each state restricts the packet types a peer can process and defines its current obligations to the swarm.

| State | Code | Description | Next State (Triggers) |
| :--- | :--- | :--- | :--- |
| **BOOTSTRAP** | `0x00` | Instantiating keypairs, validating Node ID, loading local candidate cache | `DISCOVERY` (Initial seed loaded) |
| **DISCOVERY** | `0x01` | Querying S/Kademlia DHT for active stream keys and stream publisher info | `JOINING` (Stream records fetched) |
| **JOINING** | `0x02` | Executing HyParView handshakes, populating Active and Passive sets | `CONNECTING` (Active set size $\ge 4$) |
| **CONNECTING**| `0x03` | Initiating Tree Join handshakes to establish parent connections | `ACTIVE` (Parents connected for all $M$ slices) |
| **ACTIVE** | `0x04` | Streaming active video; decoding, verifying, and uploading symbols | `CHURN_REPAIR` (Parent timeout / packet loss) |
| **CHURN_REPAIR**| `0x05` | Re-routing failed slices to standby candidate peers from Passive Set | `ACTIVE` (Re-route successful) <br> `DISCOVERY` (All parents lost, complete disconnect) |
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
|                           |  (Parent connection lost / Timeout > 200ms)
|                           v
|                 +-------------------+
+---------------- |  0x05 CHURN_REPAIR|
                  +-------------------+
                            |  (Graceful quit / App close)
                            v
                  +-------------------+
                  |  0x06 TERMINATED  |
                  +-------------------+
```
