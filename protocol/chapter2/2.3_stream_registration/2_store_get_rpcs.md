# 2. STORE and GET RPCs

The Distributed Hash Table (DHT) acts as the decentralized ledger for stream lookup. Unlike standard static torrent files, live streaming streams require a dynamic peer lookup system to track active uploaders as viewers continuously join and leave.

```text
      [ Joining Peer ]                      [ DHT Guardian Node ]
             |                                        |
             | ---- REGISTER_PEER(StreamID, Signed) ->|
             |                                        | (Verifies Signature,
             |                                        |  Updates Bucket)
             |                                        |
             | <--- REGISTER_RESPONSE(200 OK) --------|
```

### Stream Registration Protocol (`REGISTER_PEER`)
To register as an active peer for a specific stream:
1.  The peer calculates the target 256-bit stream index:
    $$K_s = \text{Blake3}(\text{StreamID})$$
2.  The peer sends a `REGISTER_PEER` RPC containing its NodeID, IP, Port, and a signed statement:
    $$\text{Payload} = \text{NodeID} \parallel IP \parallel \text{Port} \parallel \text{Timestamp}$$
    $$\text{Sig} = \text{Sign}_{SK_{\text{node}}}(\text{Payload})$$
3.  The DHT guardian nodes closest to $K_s$ receive the packet, verify the S/Kademlia Proof-of-Work, and validate the signature before adding the peer to the stream's active bucket list.

### Stream Discovery Protocol (`GET_PEERS`)
To fetch a list of active uploaders:
1.  The client executes a `GET_PEERS` RPC targeting $K_s$.
2.  The DHT guardians return a compacted list of up to 20 highly active peers, prioritized by RTT coordinates if the optional latency extension is active.
