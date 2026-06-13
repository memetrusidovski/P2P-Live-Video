# Chapter 2: Peer Discovery and Stream Registration (The Ledgerless Directory)

### 2.1 Bootstrapping and Stream Discovery over S/Kademlia DHT

When a client wants to subscribe to a stream with identity hash $H_S$, it must discover active peers without relying on a centralized tracker. The protocol utilizes a distributed directory implemented via a secure, decentralized **S/Kademlia DHT**:
1.  The client hashes the target `StreamID` to generate a 256-bit key:
    $$K = \text{Blake3}(\text{StreamID})$$
2.  The client performs a standard Kademlia `FIND_NODE` RPC using $K$ to locate the $k$ closest nodes to the target key in the 160-bit or 256-bit XOR metric space:
    $$d(x, y) = x \oplus y$$
3.  Nodes residing near the key $K$ act as registry guardians, maintaining short-lived, signed registration lists of active viewers.

### 2.2 S/Kademlia Cryptographic ID Generation & Proof-of-Work

To protect the DHT and overlay networks from Sybil identities, node IDs are bound to physical IPs and verified via Proof-of-Work (PoW). A node ID is valid if and only if it satisfies a dual-puzzle requirement:
1.  **Static Puzzle:** The node generates a public/private keypair $(PK_{\text{node}}, SK_{\text{node}})$ and a nonce $N_{\text{static}}$ satisfying:
    $$\text{Blake3}(PK_{\text{node}} \parallel N_{\text{static}}) < \text{Difficulty\_Threshold}_{\text{static}}$$
2.  **Dynamic Puzzle:** The node binds its ID to its current external IPv4/IPv6 address, generating a dynamic nonce $N_{\text{dynamic}}$ satisfying:
    $$\text{Blake3}(NodeID \parallel IP_{\text{external}} \parallel N_{\text{dynamic}}) < \text{Difficulty\_Threshold}_{\text{dynamic}}$$

```text
S/Kademlia ID Validation Frame:
+-----------------------------------------------------------------------+
| S/Kademlia NodeID (32 bytes)                                          |
+-----------------------------------------------------------------------+
| Public Key PK_node (32 bytes Ed25519)                                 |
+-----------------------------------------------------------------------+
| Static Nonce N_static (8 bytes)   | Dynamic Nonce N_dynamic (8 bytes) |
+-----------------------------------------------------------------------+
| External IP Address (16 bytes IPv6 or 4 bytes IPv4 padded)            |
+-----------------------------------------------------------------------+
```

Any node whose IP address changes must regenerate the dynamic nonce, preventing IP spoofing and Sybil clustering.

### 2.3 Store/Retrieve Protocols for Stream Signatures

*   **Registration (STORE):** To register as an active uploader for a stream, a peer sends a signed `REGISTER_PEER` RPC to the $k$ closest guardian nodes. The frame includes its NodeID, IP, Port, and a timestamp, signed with its validated private key.
*   **Querying (GET):** Subscribing nodes execute a `GET_PEERS` RPC. The DHT guardians return a subset of the registered peers, filtered by latency coordinates if available.
