# 3. Validation Frame Layout

Every **pre-session plain-UDP frame** (PING/PONG, FIND_NODE/FIND_VALUE, REGISTER_PEER/GET_PEERS, JOIN, PROBE — see Appendix D §D.2) must carry the cryptographic ID validation block to ensure immediate, zero-trust verification at the network socket layer. Post-handshake frames ride authenticated QUIC (plus the per-session HMAC of Ch3 §3.2.2) and do **not** repeat this block — repeating 152 bytes on every packet would be pure overhead once a verified session exists.

This 152-byte header is prepended to payload structures.

```text
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                               |
|                 S/Kademlia NodeID (32 bytes)                  |
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                               |
|                 Public Key PK_node (32 bytes)                 |
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                     Static Nonce (8 bytes)                    |
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                     Dynamic Nonce (8 bytes)                   |
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                               |
|            Timestamp (8-byte microsecond integer)             |
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                               |
|              Ed25519 Packet Signature (64 bytes)              |
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

*   **Ed25519 Packet Signature:** Signer uses their private key $SK_{\text{node}}$ to sign the hash of the packet payload and timestamp, protecting against IP spoofing and replay attacks.
*   **No self-declared IP:** the block deliberately carries no IP field. The verifier checks the dynamic Proof-of-Work against the **observed UDP source address** of the packet — stronger than trusting anything the sender claims about itself, and it keeps the block layout independent of address family.
*   **No explicit difficulty field:** the adaptive $C_2$ tier (Ch2 §2.2.1) needs no wire encoding. A proof solved at a higher difficulty automatically satisfies every lower threshold, so the verifier simply checks the hash against the tier required for the swarm size *it* observes, accepting one tier below to tolerate stale swarm-size reads during rapid growth.
