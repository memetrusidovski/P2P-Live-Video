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
| **Plain UDP** (pre-session) | DHT discovery/registration and first contact, before any QUIC session exists. These frames carry the 152-byte S/Kademlia validation block (Ch2 §2.2.3). | PING, PONG, FIND_NODE, FIND_VALUE, REGISTER_PEER, GET_PEERS, STORE_RECORD, STORE_RECORD_ACK, PUNCH_REQUEST, JOIN, PROBE, PROBE_RESPONSE |
| **QUIC bidirectional stream** (control) | Post-handshake control traffic, protected by QUIC encryption + the per-session HMAC of Ch3 §3.2.2. No validation block repeated. | NEIGHBOR, SHUFFLE, DISCONNECT, ACCEPTED, FORWARD_JOIN, GOSSIP_EXCHANGE, ROSTER, RANK_PROOF, MANIFEST, MANIFEST_UPDATE, MANIFEST_REQUEST, BLOCK_PROOF, BLOCK_TRANSMISSION, BUFFER_STATE_BITFIELD, PULL_REQUEST, PROOF_OF_UPLOAD, CHOKE_STATE, REPUTATION_AUDIT_GOSSIP, RELAY_PROPOSAL, RELAY_BIND, STREAM_END, PUNCH_REQUEST (forwarded form) |
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
| `0x07` | `DISCONNECT` | Ch3 §3.2.1 | Reasons: 0x01 CHOKE (`DISCONNECT_CHOKE`), 0x02 QUIT, 0x03 EVICTION, 0x04 PREEMPTED (rank admission, Ch1 §1.2.2) |
| `0x08` | `ACCEPTED` | §D.4.4 | Accepts JOIN / NEIGHBOR / tree-join requests |
| `0x09` | `FORWARD_JOIN` | §D.4.5 | HyParView random-walk propagation |
| `0x0A` | `REGISTER_PEER` | Ch2 §2.3.3 | Carries `NodeClass`, `AssignedTrees`, `Flags`; validation block is **152 bytes** |
| `0x0B` | `GET_PEERS` | Ch2 §2.3.3 | Request carries `StarvedTrees`, `WantedTrees`, `ReqFlags`; response is Peer Records plus the signed Stream Record |
| `0x0C` | `FIND_NODE` | §D.4.6 | S/Kademlia lookup (Ch2 §2.1) |
| `0x0D` | `FIND_VALUE` | §D.4.6 | |
| `0x0E` | `PROBE` | §D.4.7 | Parent-selection capacity probe (Ch1 §1.2 §2.2) |
| `0x0F` | `PROBE_RESPONSE` | §D.4.7 | K_avail, reliability, hop count, trees, node class, reachability flags |
| `0x10` | `BLOCK_TRANSMISSION` | Ch4 §4.1.3 | Pull-path block + Merkle proof |
| `0x11` | `MANIFEST` | §D.4.8 | Per-chunk (250 ms) signed manifest (Ch4 §4.1.1, Ch7 §7.1) |
| `0x12` | `RAPTORQ_SYMBOL` | Ch4 §4.2.3 | SBN = Merkle block index (Ch4 §4.2) |
| `0x13` | `BLOCK_PROOF` | §D.4.9 | Push-path Merkle proof, precedes a block's symbols |
| `0x14` | `GOSSIP_EXCHANGE` | §D.4.10 | HMAC-authenticated neighbor gossip (Ch3 §3.2.2) |
| `0x15` | `BUFFER_STATE_BITFIELD` | Ch4 §4.3.2 | |
| `0x16` | `PULL_REQUEST` | §D.4.11 | `Flags.BROADCAST_WANT` is the rarity signal |
| `0x17` | `MANIFEST_UPDATE` | §D.4.8 | Dynamic forest resize (Ch1 §1.2 §4.5); MANIFEST body plus `EffectiveSegmentSeq` and the 7-byte-per-tree slicing matrix |
| `0x18` | `ROSTER` | §D.4.15 | Per-tree child roster for Deputy election (Ch1 §1.2.3) |
| `0x19` | `RANK_PROOF` | §D.4.17 | Commit-then-sample presentation of a peer's receipts for rank admission (Ch5 §5.2.2) |
| `0x1A` | `STORE_RECORD` | Ch2 §2.3.3 | Publisher → guardian Stream Record write, once per segment |
| `0x1B` | `STORE_RECORD_ACK` | Ch2 §2.3.3 | Guardian → publisher: active/relay/per-tree/starved counts |
| `0x1C` | `PUNCH_REQUEST` | §D.4.14 | Hole-punch rendezvous via the referring guardian or neighbour (Ch6 §6.2) |
| `0x1D` | `MANIFEST_REQUEST` | §D.4.16 | Fetch past chunk manifests for late-join backfill (Ch4 §4.3.1) |
| `0x20` | `PROOF_OF_UPLOAD` | Ch5 §5.2.3 | One receipt per segment per tree: block bitmap, `LossRate`, `RxFlags`, both NodeIDs |
| `0x21` | `CHOKE_STATE` | §D.4.13 | 0=CHOKE, 1=UNCHOKE, 2=UNCHOKE_OPTIMISTIC |
| `0x22` | `REPUTATION_AUDIT_GOSSIP` | Ch5 §5.3.3 | Signed accusations carrying their evidence; at most 2 per frame |
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
[Header][Origin PeerRecord 42/54B][TTL 1B][PR_gossip 1B]
```
The origin is carried as a full Peer Record (Ch2 §2.3.3) — NodeID, class, tree bitmap, reachability and observed address — so the node that finally adds the joiner to its Active or Passive Set can file it in the right per-tree pool (Ch3 §3.1.1) without a probe.

### D.4.6 FIND_NODE (0x0C) / FIND_VALUE (0x0D) — plain UDP
```text
[Header][Validation Block 152B][Target Key 32B]
Response: [Header][Guardian NodeID 32B][Result Kind 1B (0=closer nodes, 1=value)][Count 1B][Records ...]
```
Records are Peer Records (Ch2 §2.3.3) for node results, or the value payload (e.g., a Stream Record in its canonical layout, Ch2 §2.3.3) for value results.

### D.4.6b GET_PEERS (0x0B) — plain UDP
```text
Request:  [Header][Validation Block 152B][StreamID Key K_s 32B][StarvedTrees 1B][WantedTrees 1B][ReqFlags 1B][Reserved 1B]
Response: [Header][Guardian NodeID 32B][Count 1B][RespFlags 1B][StreamRecordLength 2B][PeerRecord × Count][Stream Record]
```
Full field semantics are in Ch2 §2.3.3. `StarvedTrees` is the **starved-tree indication** the source's base-layer reserve depends on (Ch1 §1.1.5 §5.4): bit $m-1$ set means "I have failed to obtain a parent in tree $T_m$." Guardians count distinct querying NodeIDs per bit over a 10 s window and return the counts to the publisher in `STORE_RECORD_ACK`. `WantedTrees` selects relays of the named trees; `0x00` is a membership query. The guardian's sample is uniformly random per query.

### D.4.7 PROBE (0x0E) / PROBE_RESPONSE (0x0F) — plain UDP
```text
PROBE:          [Header][Validation Block 152B][StreamID 32B][TreeID 1B]
PROBE_RESPONSE: [Header][Sender NodeID 32B][K_avail 2B][Reliability 2B (fixed-point /65535)]
                [HopCount 1B][NodeClass 1B][Flags 1B][AssignedTrees 1B]
                [LiveEdgeSegmentSeq 4B]
```
This is the response `send_udp_probe()` consumes in the tree-join algorithm (Ch1 §1.2 §2.2). `K_avail` is the responder's free slots **in the tree named by the PROBE**, computed against that tree's declared bitrate (Ch1 §1.2 §1.3). `Flags` and `AssignedTrees` have the Peer Record semantics of Ch2 §2.3.3; multi-tree super nodes (Ch1 §1.2 §1.3) set every assigned tree's bit, including any coverage-granted tree. `K_avail = 0` with `LiveEdgeSegmentSeq = 0` means **warming up** (accepted into the tree, no verified segment yet); `K_avail = 0` with `LiveEdgeSegmentSeq > 0` means **saturated**. The two are treated differently by the shed rule (Ch1 §1.1.5).

### D.4.8 MANIFEST (0x11) / MANIFEST_UPDATE (0x17)
```text
MANIFEST:
[Header][StreamID 32B][SegmentSeq 4B][ChunkIndex 1B][ChunkCount 1B][Reserved 2B][Timestamp 8B µs][MerkleRoot 32B]
[BlockCount 2B][ChunkByteLength 4B][SlicingMatrixVersion 1B][LayerCount 1B][Reserved 2B]
[LayerBlockCount 2B × LayerCount]
[Ed25519 Source Signature 64B]

MANIFEST_UPDATE:
[Header][StreamID 32B][SegmentSeq 4B][ChunkIndex 1B][ChunkCount 1B][Reserved 2B][Timestamp 8B µs][MerkleRoot 32B]
[BlockCount 2B][ChunkByteLength 4B][SlicingMatrixVersion 1B][LayerCount 1B][Reserved 2B]
[LayerBlockCount 2B × LayerCount]
[EffectiveSegmentSeq 4B][NumTrees 1B]
[TreeID 1B][Layer 1B][StripeIndex 1B][StripeCount 1B][Priority 1B][BitrateKbps 2B]   × NumTrees
[Ed25519 Source Signature 64B]
```
The signature covers every preceding byte after the header.

*   **One `MANIFEST` per 250 ms chunk** (Ch4 §4.1.1): `ChunkIndex` $\in [0, \text{ChunkCount})$, `ChunkCount` $= 4$. The Merkle root and block counts are the chunk's. Manifests are admitted by sequence window and duplicates are never penalised (Ch7 §7.1.2).
*   **`LayerBlockCount`** gives the number of 16 KB blocks of each layer in this chunk, lowest layer first; `BlockCount` is their sum. Within the chunk, blocks are numbered $j$ across layers in that order, so block $j'$ of layer $l$ has $j = \sum_{l' < l} \text{LayerBlockCount}_{l'} + j'$, travels on the tree of layer $l$ with `StripeIndex` $= j' \bmod \text{StripeCount}$ (Ch1 §1.2 §4.2.1), and carries the global `BlockIndex = ChunkIndex << 12 | j` in every block-addressed frame. Without these counts a peer cannot tell which tree a missing block belongs to.
*   **Slicing matrix** entries are exactly the output of the allocation rule of Ch1 §1.2 §4.2.1. `BitrateKbps` is the tree's declared bitrate and is the value every slot computation uses (Ch1 §1.2 §1.3). All trees of one layer share a `Priority`; lower is shed first; the layer-0 trees are never shed.
*   **`EffectiveSegmentSeq`** is the first segment emitted on the new layout — the end of the 5-segment migration window (Ch1 §1.2 §4.5). A `MANIFEST_UPDATE` is accepted only if `SlicingMatrixVersion` is strictly greater than the last accepted version.

### D.4.9 BLOCK_PROOF (0x13)
```text
[Header][SegmentSeq 4B][BlockIndex 2B][ProofDepth 1B][SenderHopDepth 1B][Sibling Hashes 32B × depth]
```
Sent on the tree's QUIC stream immediately before the block's RAPTORQ_SYMBOL datagrams, so the receiver can Merkle-verify the reconstructed block against the chunk's MANIFEST root before forwarding (Ch4 §4.1–4.2). **`SenderHopDepth`** is the sender's current hop distance from the source in this tree; the receiver sets its own depth in the tree to `SenderHopDepth + 1` on every block (Ch1 §1.2.2 *Depth Propagation*). The byte was formerly reserved; carrying depth on the frame every parent already sends every child several times a second is what keeps the hop penalty and the $D_{\max}$ admission rule running on current data rather than join-time data.

### D.4.10 GOSSIP_EXCHANGE (0x14)
```text
[Header][Sender NodeID 32B][SequenceNumber 4B][NeighborCount 1B][WantedTrees 1B][Reserved 2B]
[PeerRecord × NeighborCount][HMAC-Blake3 32B]
```
The HMAC uses the DH-derived transient session key and monotonic sequence number of Ch3 §3.2.2. Records are full Peer Records (Ch2 §2.3.3). `WantedTrees` tells the receiver which trees the sender is short of relays for, so the reply can be drawn from those pools (Ch3 §3.1.1).

### D.4.11 PULL_REQUEST (0x16)
```text
[Header][SegmentSeq 4B][BlockIndex 2B][MissingSymbolCount 1B][UrgencyMs 2B][Flags 1B][Reserved 2B]
```
`MissingSymbolCount = 0` requests the whole block (served as BLOCK_TRANSMISSION); a non-zero count requests that many repair symbols (served as RAPTORQ_SYMBOL datagrams, drawn from the responder's derived ESI base so parallel repairs do not duplicate — Ch4 §4.1.2). `UrgencyMs` is the requester's time-to-playout-deadline, used by the responder's scheduler (Ch4 §4.3.3). `Flags` bit 0 **`BROADCAST_WANT`** is the **rarity signal** of Ch4 §4.3.3 (`TriggerRarityGossip`): the requester sends the same frame to *every* Active Set neighbour because no bitfield it holds shows the block; a neighbour that has the block answers, others stay silent. It is bounded to one broadcast per missing block per $\tau_{\text{sched}}$.

### D.4.12 PROOF_OF_UPLOAD (0x20)
```text
[Header][SegmentSeq 4B][TreeID 1B][RxFlags 1B][LossRate 1B][BitmapLen 1B][BlockBitmap ≤ 64B]
[K_s 32B][Uploader NodeID 32B][Downloader NodeID 32B][Timestamp 8B µs][Downloader Signature 64B]
```
The canonical layout and field semantics are in Ch5 §5.2.3. One receipt per **segment per tree** (not per block): the bitmap's bit $\text{ChunkIndex} \cdot 128 + j$ marks block $(\text{ChunkIndex}, j)$ as received from the uploader and Merkle-verified. `LossRate` is the child→parent loss report consumed by adaptive parity (Ch4 §4.2.2) and the slot-count overhead factor (Ch1 §1.2.1). `RxFlags`: bit 0 `RELAYED`, bit 1 `PULL` (then `TreeID = 0`).

### D.4.13 CHOKE_STATE (0x21)
```text
[Header][State 1B (0=CHOKE, 1=UNCHOKE, 2=UNCHOKE_OPTIMISTIC)][Reserved 3B]
```

### D.4.14 PUNCH_REQUEST (0x1C)
```text
Request form  (requester → referrer, plain UDP or the requester's QUIC control stream):
[Header][Validation Block 152B if plain UDP][Target NodeID 32B][StreamID 32B]

Forward form  (referrer → target, plain UDP with the referrer's validation block, or the referrer's QUIC control stream to the target):
[Header][Validation Block 152B if plain UDP][Requester PeerRecord 42/54B][StreamID 32B]
```
A joiner that wants to `PROBE` a `CONE`-reachability candidate (Ch2 §2.3.3 `Flags`) sends the request form to the **referrer** — the guardian whose `GET_PEERS` response named the candidate, or the gossip neighbour whose `SHUFFLE`/`GOSSIP_EXCHANGE` did. The referrer builds the requester's Peer Record from the **observed** source address of the request (never from a self-declared field) and forwards it to the target, which it can reach because it holds the target's registration or an open session. The target sends a `PING` to the requester's address, opening its NAT mapping; the requester's `PROBE` that follows within $1$ s then passes. Cost: one extra round-trip for `CONE` parents, none for `PUBLIC`. `SYMMETRIC` targets are never probed (Ch1 §1.2.5). This is the "gossip signaling path" of Ch6 §6.2.1, made concrete.

### D.4.16 MANIFEST_REQUEST (0x1D)
```text
[Header][SegmentSeq 4B][ChunkIndex 1B (0xFF = every chunk of the segment)][Reserved 3B]
```
Sent on the QUIC control stream to a parent or Active Set neighbour; answered with the corresponding `MANIFEST` frame(s) from the responder's retained window ($\tau_{\text{retain}}$), or nothing if it no longer holds them. The push path delivers only manifests signed after a peer connected, so late-join backfill (Ch4 §4.3.1 step 3) and any peer repairing a segment whose manifest it missed need this explicit fetch.

### D.4.17 RANK_PROOF (0x19)
```text
[Header][ReceiptCount 4B][TotalBytes 8B][CommitSegmentSeq 4B][ReceiptListRoot 32B][Prover NodeID 32B]
[Prover Signature 64B over (ReceiptCount ‖ TotalBytes ‖ CommitSegmentSeq ‖ ReceiptListRoot ‖ Prover NodeID)]
[NonceSegmentSeq 4B][NonceChunkIndex 1B][SampleCount 1B (= 8)][Reserved 2B]
[ per sample: [ReceiptLength 2B][PROOF_OF_UPLOAD body (Ch5 §5.2.3, without header)][MerklePath 32B × ⌈log2 ReceiptCount⌉] ] × SampleCount
```
Sent on the QUIC control stream immediately after a `NEIGHBOR(TreeID > 0)`, or to an auditor on request. Semantics — commitment, nonce rule (`NonceSegmentSeq > CommitSegmentSeq`, the nonce being that chunk's `MANIFEST.MerkleRoot`), sample index derivation and verifier checks — are in Ch5 §5.2.2. A proof with any failing sample is a verification failure (Ch7 §7.1.1). Size $\le 8$ KB at a super node's receipt volume.

### D.4.18 REPUTATION_AUDIT_GOSSIP (0x22)
```text
[Header][Accuser NodeID 32B][Count 1B (≤ 2)][Reserved 3B]
[ per accusation: [Suspect NodeID 32B][EvidenceType 1B][Reserved 1B][EvidenceLength 2B][Evidence ...][Timestamp 8B]
                  [Accuser Signature 64B over the accusation's preceding bytes] ] × Count
```
`EvidenceType` `0x01` `SYMMETRIC_TREE_PAIR` (evidence: two `PROOF_OF_UPLOAD` bodies), `0x02` `EQUIVOCATION` (evidence: two contradictory statements signed by the suspect). Receivers re-verify evidence before counting and never forward an accusation they could not verify (Ch5 §5.3.3).

### D.4.15 ROSTER (0x18)
```text
[Header][TreeID 1B][RosterSeq 4B][ChildCount 2B][EntryCount 1B]
[NodeID 32B][K_avail 2B][NodeClass 1B][Flags 1B][AddrFamily 1B][IP 4/16B][Port 2B]   × EntryCount
```
Sent by a relay parent to every child in tree `TreeID` on the tree's QUIC stream every $	au_{	ext{roster}}$. `ChildCount` is the parent's total children in this tree ($k$ in the election formulas). The entries are the parent's **top $R_{	ext{roster}} = 8$ children that are themselves relays of `TreeID`** (bit `TreeID`$-1$ of their `AssignedTrees` set), ordered by their advertised `K_avail` **in this tree** descending, ties by lowest NodeID; children that do not relay this tree are never listed, since they could not accept an orphan's `RELAY_JOIN_REQUEST(TreeID)`. Addresses are included because siblings are not generally in one another's Active Sets. Full election semantics: Ch1 §1.2.3.
