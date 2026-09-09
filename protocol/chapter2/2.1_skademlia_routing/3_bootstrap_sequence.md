# 3. The Step-by-Step Bootstrapping Sequence

When a node initializes for the very first time, it has an empty routing table and must connect to the global DHT space.

1.  **Seed Resolution:** The node resolves bootstrap contacts in priority order:
    *   **DNS-SRV (primary):** The node queries `_p2p._udp.p2p.live` for SRV records. This allows multiple independent operators to publish seed nodes under separate subdomains (e.g., `seed.operator-a.com`, `seed.operator-b.com`), eliminating any single point of failure. No single operator controls bootstrapping.
    *   **Hardcoded fallback:** If DNS is unavailable or returns no results, the node falls back to a compiled-in list:
    $$\mathcal{S}_{\text{bootstrap}} = \{\text{seed1.p2p.live:port}, \text{seed2.p2p.live:port}\}$$
    Implementations should ship with seeds from at least three independent operators to ensure no single party can block network entry.
2.  **Ping/Pong Validation and Reachability Classification:** The node sends a `PING` to **two** seed nodes from different operators. Each responds with a `PONG` reflecting the address and port it observed (Appendix D §D.4.1). From the two reflections the node classifies its own **`Reachability`** (Ch2 §2.3.3 `Flags`), the input that decides whether it may declare `RELAY` class (Ch1 §1.2.5):

    | Observation | `Reachability` |
    | :--- | :--- |
    | Both reflections equal the socket's own local address and port | `PUBLIC` |
    | Both reflections agree with each other but differ from the local address | `CONE` (mapping is endpoint-independent; a hole punch will work) |
    | The two reflections differ | `SYMMETRIC` (mapping is per-destination; inbound is unreachable) |

    The classification is repeated on every IP change (Ch1 §1.3.3). A node that solves the dynamic puzzle against a reflected address (Ch2 §2.2) already has both reflections in hand, so this costs no extra round-trip.
3.  **Self-Lookup:** The node inserts the seed $S$ into its routing table and immediately initiates a `FIND_NODE` query targeting its *own* $NodeID_{\text{local}}$.
4.  **Bucket Population:** As this self-lookup traverses the network, intermediate nodes learn of the new node's existence and — after confirming it answers an unsolicited `PING` — insert it into their k-buckets. Concurrently, the new node populates its own k-buckets with the contact info of the nodes returning results. Nodes that are leaf-class or `SYMMETRIC` do not answer DHT RPCs and are therefore never inserted: they use the DHT in **client mode** (Ch2 §2.3.2) and can never become guardians.
5.  **Stream Registration Discovery:** Once the routing table is populated, the node hashes the target `StreamID` to generate the 256-bit index $K_s$ and executes a parallel `FIND_VALUE` lookup to find the active viewer list.
