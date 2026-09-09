# Appendix D: Canonical Frame-Type Registry

This appendix is the **single normative registry** of every frame in the protocol. The byte layouts in the chapters (and the new layouts below) are the canonical wire encoding; the protobuf schema in `schemas/p2p_live.proto` is a **non-normative logical field reference** for implementers who want typed message definitions.

## D.1 Common Frame Header

Every frame begins with the same 4-byte header:

```text
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|   Version     |     Type      |          Payload Length       |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

Protocol `Version` lives **only** here — payload structures do not repeat it. All multi-byte integers are big-endian. (Note: frame type codes and the peer lifecycle state codes of Ch1 §1.3 are separate namespaces; the numeric overlap of `0x00–0x06` is coincidental.)

## D.2 Transport Mapping

| Transport | Used for | Frames |
| :--- | :--- | :--- |
| **Plain UDP** (pre-session) | DHT discovery/registration and first contact, before any QUIC session exists. These frames carry the 152-byte S/Kademlia validation block (Ch2 §2.2.3). | PING, PONG, FIND_NODE, FIND_VALUE, REGISTER_PEER, GET_PEERS, JOIN, PROBE, PROBE_RESPONSE |
| **QUIC bidirectional stream** (control) | Post-handshake control traffic, protected by QUIC encryption + the per-session HMAC of Ch3 §3.2.2. No validation block repeated. | NEIGHBOR, SHUFFLE, DISCONNECT, ACCEPTED, FORWARD_JOIN, GOSSIP_EXCHANGE, MANIFEST, MANIFEST_UPDATE, BLOCK_PROOF, BLOCK_TRANSMISSION, BUFFER_STATE_BITFIELD, PULL_REQUEST, PROOF_OF_UPLOAD, CHOKE_STATE, REPUTATION_AUDIT_GOSSIP, RELAY_PROPOSAL, RELAY_BIND, STREAM_END |
| **QUIC datagram** (RFC 9221, unreliable) | High-rate media symbols where retransmission is replaced by FEC. | RAPTORQ_SYMBOL (optionally BUFFER_STATE_BITFIELD) |

Connection migration is **not a frame**: WiFi↔cellular handover uses QUIC-native path validation with 64-bit Connection IDs (Ch6 §6.2).

## D.3 Registry

| Code | Frame | Defined in | Notes |
| :---: | :--- | :--- | :--- |
| `0x01` | `PING` | §D.4.1 | Carries validation block; solicits PONG |
| `0x02` | `PONG` | §D.4.1 | Reflects observed source IP:port (STUN-lite) |
| `0x03` | `JOIN` | §D.4.2 | HyParView join (Ch3 §3.1) |
| `0x04` | `STREAM_END` | Ch1 §1.3.1 | Signed broadcaster shutdown |
| `0x05` | `NEIGHBOR` | Ch3 §3.2.1 + §D.4.3 | **Amended:** carries `TreeID`. `RELAY_JOIN_REQUEST` (Ch1 §1.2.3) ≡ NEIGHBOR with Priority=HIGH and TreeID set |
| `0x06` | `SHUFFLE` | Ch3 §3.2.1 | |
| `0x07` | `DISCONNECT` | Ch3 §3.2.1 | Reasons: 0x01 CHOKE (`DISCONNECT_CHOKE`), 0x02 QUIT, 0x03 EVICTION |
| `0x08` | `ACCEPTED` | §D.4.4 | Accepts JOIN / NEIGHBOR / tree-join requests |
| `0x09` | `FORWARD_JOIN` | §D.4.5 | HyParView random-walk propagation |
| `0x0A` | `REGISTER_PEER` | Ch2 §2.3.3 | Validation block is **152 bytes** |
| `0x0B` | `GET_PEERS` | Ch2 §2.3.3 | Response includes the Stream Record (Ch2 §2.3.1) |
| `0x0C` | `FIND_NODE` | §D.4.6 | S/Kademlia lookup (Ch2 §2.1) |
| `0x0D` | `FIND_VALUE` | §D.4.6 | |
| `0x0E` | `PROBE` | §D.4.7 | Parent-selection capacity probe (Ch1 §1.2 §2.2) |
| `0x0F` | `PROBE_RESPONSE` | §D.4.7 | K_avail, reliability, hop count, trees, node class |
| `0x10` | `BLOCK_TRANSMISSION` | Ch4 §4.1.3 | Pull-path block + Merkle proof |
| `0x11` | `MANIFEST` | §D.4.8 | Per-segment signed manifest (Ch7 §7.1) |
| `0x12` | `RAPTORQ_SYMBOL` | Ch4 §4.2.3 | SBN = Merkle block index (Ch4 §4.2) |
| `0x13` | `BLOCK_PROOF` | §D.4.9 | Push-path Merkle proof, precedes a block's symbols |
| `0x14` | `GOSSIP_EXCHANGE` | §D.4.10 | HMAC-authenticated neighbor gossip (Ch3 §3.2.2) |
| `0x15` | `BUFFER_STATE_BITFIELD` | Ch4 §4.3.2 | |
| `0x16` | `PULL_REQUEST` | §D.4.11 | |
| `0x17` | `MANIFEST_UPDATE` | §D.4.8 | Dynamic forest resize (Ch1 §1.2 §4.5); same body as MANIFEST plus new tree mapping |
| `0x20` | `PROOF_OF_UPLOAD` | Ch5 §5.2.3 + §D.4.12 | **Amended:** carries the downloader's NodeID for third-party audit |
| `0x21` | `CHOKE_STATE` | §D.4.13 | 0=CHOKE, 1=UNCHOKE, 2=UNCHOKE_OPTIMISTIC |
| `0x22` | `REPUTATION_AUDIT_GOSSIP` | Ch5 §5.3.3 | |
| `0x30` | `RELAY_PROPOSAL` | Ch6 §6.3.3 | |
| `0x31` | `RELAY_BIND` | Ch6 §6.3.3 | |

## D.4 Layouts for Previously Unspecified Frames

All layouts below follow the D.1 header. `NodeID` fields are 32 bytes; pre-session frames additionally carry the 152-byte validation block immediately after the header.

### D.4.1 PING (0x01) / PONG (0x02) — plain UDP
```text
PING:  [Header][Validation Block 152B][StreamID 32B (zero if generic)]
PONG:  [Header][Validation Block 152B][Reflected AddrFamily 1B][Reflected IP 4/16B][Reflected Port 2B]
```
PONG's reflected address gives the sender its external mapping without a separate STUN round-trip. The verifier checks the dynamic PoW (Ch2 §2.2) against the **observed UDP source address** of the packet — never against a self-declared field.

### D.4.2 JOIN (0x03) — plain UDP → QUIC upgrade
```text
[Header][Validation Block 152B][StreamID 32B][NodeClass 1B][Reserved 3B]
```

### D.4.3 NEIGHBOR (0x05) — amendment
The Ch3 §3.2.1 layout gains one byte after `Priority`:
```text
[... existing fields ...][Priority 1B][TreeID 1B]
```
`TreeID = 0` means "membership only" (classic HyParView semantics); `TreeID = m > 0` requests a parent slot in tree $T_m$ (`RELAY_JOIN_REQUEST` when Priority=HIGH).

### D.4.4 ACCEPTED (0x08)
```text
[Header][Sender NodeID 32B][AcceptedType 1B (0x03 JOIN | 0x05 NEIGHBOR)][TreeID 1B][HopDepth 1B][Reserved 1B]
```
`HopDepth` is the acceptor's hop distance from the source in `TreeID`, letting the joiner set its own depth to `HopDepth + 1` immediately.

### D.4.5 FORWARD_JOIN (0x09)
```text
[Header][Origin NodeID 32B][Origin AddrFamily 1B][Origin IP 4/16B][Origin Port 2B][TTL 1B][PR_gossip 1B]
```

### D.4.6 FIND_NODE (0x0C) / FIND_VALUE (0x0D) — plain UDP
```text
[Header][Validation Block 152B][Target Key 32B]
Response: [Header][Guardian NodeID 32B][Result Kind 1B (0=closer nodes, 1=value)][Count 1B][Records ...]
```
Records are PeerAddressRecords (Ch2 §2.3.3) for node results, or the value payload (e.g., a Stream Record) for value results.

### D.4.7 PROBE (0x0E) / PROBE_RESPONSE (0x0F) — plain UDP
```text
PROBE:          [Header][Validation Block 152B][StreamID 32B][TreeID 1B]
PROBE_RESPONSE: [Header][Sender NodeID 32B][K_avail 2B][Reliability 2B (fixed-point /65535)]
                [HopCount 1B][NodeClass 1B][AssignedTreeCount 1B][AssignedTreeIDs 1B × count]
                [LiveEdgeSegmentSeq 4B]
```
This is the response `send_udp_probe()` consumes in the tree-join algorithm (Ch1 §1.2 §2.2). Multi-tree super nodes (Ch1 §1.2 §1.3) list every assigned tree.

### D.4.8 MANIFEST (0x11) / MANIFEST_UPDATE (0x17)
```text
[Header][StreamID 32B][SegmentSeq 4B][Timestamp 8B µs][MerkleRoot 32B]
[BlockCount 2B][SegmentByteLength 4B][SlicingMatrixVersion 1B][Reserved 3B]
[Ed25519 Source Signature 64B]
```
MANIFEST_UPDATE appends the new slicing matrix (as in Ch1 §1.2 §4.4's JSON, serialized as: `NumTrees 1B` + per-tree `{TreeID 1B, Layer 1B, Priority 1B}`) before the signature; its `SlicingMatrixVersion` must be strictly greater than the last accepted version.

### D.4.9 BLOCK_PROOF (0x13)
```text
[Header][SegmentSeq 4B][BlockIndex 2B][ProofDepth 1B][Reserved 1B][Sibling Hashes 32B × depth]
```
Sent on the tree's QUIC stream immediately before the block's RAPTORQ_SYMBOL datagrams, so the receiver can Merkle-verify the reconstructed block against the MANIFEST root before forwarding (Ch4 §4.1–4.2).

### D.4.10 GOSSIP_EXCHANGE (0x14)
```text
[Header][Sender NodeID 32B][SequenceNumber 4B][NeighborCount 1B][Reserved 3B]
[PeerAddressRecords ...][HMAC-Blake3 32B]
```
The HMAC uses the DH-derived transient session key and monotonic sequence number of Ch3 §3.2.2.

### D.4.11 PULL_REQUEST (0x16)
```text
[Header][SegmentSeq 4B][BlockIndex 2B][MissingSymbolCount 1B][UrgencyMs 2B][Reserved 3B]
```
`MissingSymbolCount = 0` requests the whole block (served as BLOCK_TRANSMISSION); a non-zero count requests that many repair symbols (served as RAPTORQ_SYMBOL datagrams). `UrgencyMs` is the requester's time-to-playout-deadline, used by the responder's scheduler (Ch4 §4.3.3).

### D.4.12 PROOF_OF_UPLOAD (0x20) — amendment
The Ch5 §5.2.3 layout gains the signer's identity so receipts are third-party auditable (Ch5 §5.3):
```text
[... existing fields ...][Downloader NodeID 32B][Downloader Signature 64B]
```

### D.4.13 CHOKE_STATE (0x21)
```text
[Header][State 1B (0=CHOKE, 1=UNCHOKE, 2=UNCHOKE_OPTIMISTIC)][Reserved 3B]
```
