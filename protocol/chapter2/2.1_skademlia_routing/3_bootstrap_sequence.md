# 3. The Step-by-Step Bootstrapping Sequence

When a node initializes for the very first time, it has an empty routing table and must connect to the global DHT space.

1.  **Hardcoded Seed Query:** The node reads a list of immutable, fallback bootstrap nodes from its configuration (or DNS seeds):
    $$\mathcal{S}_{\text{bootstrap}} = \{\text{seed1.p2p.live:port}, \text{seed2.p2p.live:port}\}$$
2.  **Ping/Pong Validation:** The node sends a `PING` RPC to a seed node $S$. $S$ responds with a `PONG` containing the joining node's verified public external IP address.
3.  **Self-Lookup:** The node inserts the seed $S$ into its routing table and immediately initiates a `FIND_NODE` query targeting its *own* $NodeID_{\text{local}}$.
4.  **Bucket Population:** As this self-lookup traverses the network, all intermediate nodes learn of the new node's existence and insert it into their k-buckets. Concurrently, the new node populates its own k-buckets with the contact info of the nodes returning results.
5.  **Stream Registration Discovery:** Once the routing table is populated, the node hashes the target `StreamID` to generate the 256-bit index $K_s$ and executes a parallel `FIND_VALUE` lookup to find the active viewer list.
