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
| **QUIC bidirectional stream** (control) | Post-handshake control traffic, protected by QUIC encryption + the per-session HMAC of Ch3 §3.2.2. No validation block repeated. | NEIGHBOR, SHUFFLE, DISCONNECT, DRAIN_NOTICE, ACCEPTED, FORWARD_JOIN, GOSSIP_EXCHANGE, ROSTER, RANK_PROOF, MANIFEST, MANIFEST_UPDATE, MANIFEST_REQUEST, BLOCK_PROOF, BLOCK_TRANSMISSION, BUFFER_STATE_BITFIELD, PULL_REQUEST, PROOF_OF_UPLOAD, CHOKE_STATE, REPUTATION_AUDIT_GOSSIP, RELAY_PROPOSAL, RELAY_BIND, STREAM_END, PUNCH_REQUEST (forwarded form) |
| **QUIC datagram** (RFC 9221, unreliable) | High-rate media symbols where retransmission is replaced by FEC. | RAPTORQ_SYMBOL (optionally BUFFER_STATE_BITFIELD) |

Connection migration is **not a frame**: WiFi↔cellular handover uses QUIC-native path validation with 64-bit Connection IDs (Ch6 §6.2).

## D.3 Registry

| Code | Frame | Defined in | Notes |
| :---: | :--- | :--- | :--- |
| `0x01` | `PING` | §D.4.1 | Carries validation block; solicits PONG |
| `0x02` | `PONG` | §D.4.1 | Reflects observed source IP:port (STUN-lite) |
| `0x03` | `JOIN` | §D.4.2 | HyParView join (Ch3 §3.1) |
| `0x04` | `STREAM_END` | Ch1 §1.3.1 | Signed broadcaster shutdown |
| `0x05` | `NEIGHBOR` | Ch3 §3.2.1 + §D.4.3 | **Amended:** carries `TreeID`, `NodeClass`, `AssignedTrees`. `RELAY_JOIN_REQUEST` (Ch1 §1.2.3) ≡ NEIGHBOR with Priority=HIGH and TreeID set |
| `0x06` | `SHUFFLE` | Ch3 §3.2.1 | `TTL = 0` over an existing session is a direct request answered by `GOSSIP_EXCHANGE` (Ch3 §3.3.1 tier 3) |
| `0x07` | `DISCONNECT` | Ch3 §3.2.1 + §D.4.3b | Carries `TreeID` (0 = whole connection). Reasons (shared with `DRAIN_NOTICE`): 0x01 CHOKE (`DISCONNECT_CHOKE`), 0x02 QUIT, 0x03 EVICTION, 0x04 PREEMPTED, 0x05 DISPLACED, 0x06 REASSIGNED, 0x07 DEMOTED, 0x08 CAPACITY, 0x09 DEPTH (Ch1 §1.2.2 *The Drain Path*); **refusals** of a `NEIGHBOR`: 0x0A REJECTED_SATURATED, 0x0B REJECTED_NOT_ASSIGNED, 0x0C REJECTED_DEPTH |
| `0x08` | `ACCEPTED` | §D.4.4 | Accepts JOIN / NEIGHBOR / tree-join requests; `AcceptFlags.PENDING` for a sequential handover |
| `0x09` | `FORWARD_JOIN` | §D.4.5 | HyParView random-walk propagation |
| `0x0A` | `REGISTER_PEER` | Ch2 §2.3.3 | Carries `NodeClass`, `AssignedTrees`, `Flags`; validation block is **152 bytes** |
| `0x0B` | `GET_PEERS` | Ch2 §2.3.3 | Request carries `StarvedTrees`, `WantedTrees`, `ReqFlags`; response is Peer Records plus the signed Stream Record |
| `0x0C` | `FIND_NODE` | §D.4.6 | S/Kademlia lookup (Ch2 §2.1) |
| `0x0D` | `FIND_VALUE` | §D.4.6 | |
| `0x0E` | `PROBE` | §D.4.7 | Parent-selection capacity probe (Ch1 §1.2 §2.2) |
| `0x0F` | `PROBE_RESPONSE` | §D.4.7 | K_avail, reliability, hop count, trees, node class, reachability flags, per-tree `TreeState` and verified live edge |
| `0x10` | `BLOCK_TRANSMISSION` | Ch4 §4.1.3 | Pull-path block + Merkle proof |
| `0x11` | `MANIFEST` | §D.4.8 | Per-chunk (250 ms) signed manifest (Ch4 §4.1.1, Ch7 §7.1) |
| `0x12` | `RAPTORQ_SYMBOL` | Ch4 §4.2.3 | SBN = Merkle block index (Ch4 §4.2) |
| `0x13` | `BLOCK_PROOF` | §D.4.9 | Push-path Merkle proof, precedes a block's symbols |
| `0x14` | `GOSSIP_EXCHANGE` | §D.4.10 | HMAC-authenticated neighbor gossip (Ch3 §3.2.2) |
| `0x15` | `BUFFER_STATE_BITFIELD` | Ch4 §4.3.2 | |
| `0x16` | `PULL_REQUEST` | §D.4.11 | `Flags.BROADCAST_WANT` is the rarity signal |
| `0x17` | `MANIFEST_UPDATE` | §D.4.8 | Dynamic forest resize (Ch1 §1.2 §4.5); MANIFEST body plus `EffectiveSegmentSeq` and the 7-byte-per-tree slicing matrix |
| `0x18` | `ROSTER` | §D.4.15 | Per-tree child roster for Deputy election (Ch1 §1.2.3) |
| `0x19` | `RANK_PROOF` | §D.4.17 | Commit-then-sample presentation of a peer's receipts: sorted list, adjacent-pair samples (Ch5 §5.2.2) |
| `0x1A` | `STORE_RECORD` | Ch2 §2.3.3 | Publisher → guardian Stream Record write, once per segment |
| `0x1B` | `STORE_RECORD_ACK` | Ch2 §2.3.3 | Guardian → publisher: active/relay/per-tree/starved counts and the query count |
| `0x1C` | `PUNCH_REQUEST` | §D.4.14 | Hole-punch rendezvous via the referring guardian or neighbour (Ch6 §6.2) |
| `0x1D` | `MANIFEST_REQUEST` | §D.4.16 | Fetch past chunk manifests, or the pending `MANIFEST_UPDATE`, for late-join backfill (Ch4 §4.3.1) |
| `0x1E` | `DRAIN_NOTICE` | §D.4.19 | Parent → child: "re-select now, I keep serving until the deadline" (Ch1 §1.2.2 *The Drain Path*) |
| `0x1F` | `STREAM_DESCRIPTOR` | §D.4.20 | Source-signed codec / container / initialisation data per layer — what the decoder needs and the protocol never reads (Ch4 §4.1.1) |
| `0x20` | `PROOF_OF_UPLOAD` | Ch5 §5.2.3 | One receipt per segment per tree: block bitmap, `LossRate`, `RxFlags`, both NodeIDs, plus the child's `K_avail`/`AssignedTrees`/`NodeClass` advertisement |
| `0x21` | `CHOKE_STATE` | §D.4.13 | 0=CHOKE, 1=UNCHOKE, 2=UNCHOKE_OPTIMISTIC |
| `0x22` | `REPUTATION_AUDIT_GOSSIP` | Ch5 §5.3.3 + §D.4.18 | Signed accusations carrying their evidence and the accuser's `RANK_PROOF`; at most 2 accusations per frame |
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
The Ch3 §3.2.1 layout's two reserved bytes after `Priority` and `TreeID` carry the requester's self-description:
```text
[Header][Sender NodeID 32B][Priority 1B][TreeID 1B][NodeClass 1B][AssignedTrees 1B]
```
`TreeID = 0` means "membership only" (classic HyParView semantics); `TreeID = m > 0` requests a parent slot in tree $T_m$ (`RELAY_JOIN_REQUEST` when Priority=HIGH). `NodeClass` and `AssignedTrees` have the Peer Record semantics of Ch2 §2.3.3 and are what the Rank Admission Rule (Ch1 §1.2.2) reads to apply the leaf share and the displacement exclusion; without them a parent that did not see the requester's `JOIN` decided a class-dependent rule on a class the request did not carry. The parent records the requester's `Reachability` and address as *observed* on the session, never from the frame.

### D.4.3b DISCONNECT (0x07) — amendment
The Ch3 §3.2.1 layout's row after the NodeID is `[Reason 1B][TreeID 1B][Reserved 2B]`. `TreeID = 0` ends the whole connection (classic HyParView semantics, and the only form the earlier layout could express); `TreeID = m > 0` ends **only the tree-$m$ relationship** — or, with a refusal reason, declines a `NEIGHBOR(TreeID = m)` that never became one — and leaves any membership or other-tree relationship on the same connection intact.

**Every `NEIGHBOR` is answered.** A `NEIGHBOR` is answered within $\tau_{\text{sched}}$ with exactly one of `ACCEPTED` or `DISCONNECT` carrying the request's `TreeID` and one of three refusal reasons:

| Reason | Meaning | Requester's action |
| :---: | :--- | :--- |
| `0x0A` `REJECTED_SATURATED` | No free slot in `TreeID` and no admission rule applied — Rank Admission cases 3–5 (Ch1 §1.2.2); for `TreeID = 0`, no Active Set capacity | Counts the candidate toward the shed threshold (Ch1 §1.1.5 §5.3); **keeps the record** — the peer is full, not bad |
| `0x0B` `REJECTED_NOT_ASSIGNED` | The responder does not relay `TreeID` | Clears bit `TreeID`$-1$ of the record's `AssignedTrees` in its pools (Ch3 §3.1.1); keeps the peer |
| `0x0C` `REJECTED_DEPTH` | The responder's depth is $\ge D_{\max} - 1$, so the child would sit at $\ge D_{\max}$ (Ch1 §1.2.2 *Depth Admission Rule*) | Counts toward the shed threshold; prefers shallower candidates next round |

Only a **timeout** (no answer within $\tau_{\text{sched}}$ plus one RTT) or `EVICTION` removes a candidate from the passive set. An earlier draft consumed the values `REJECTED` and `REJECTED_NOT_ASSIGNED` in the tree-join and churn-recovery algorithms and defined no frame that produced either; under silence, a join round cost one probe timeout per refusing candidate, the stale-bitmap correction of SOLUTION-028 was unreachable, and the shed rule could not tell a saturated forest from a dead one.

### D.4.4 ACCEPTED (0x08)
```text
[Header][Sender NodeID 32B][AcceptedType 1B (0x03 JOIN | 0x05 NEIGHBOR)][TreeID 1B][HopDepth 1B][AcceptFlags 1B]
```
`HopDepth` is the acceptor's hop distance from the source in `TreeID`, letting the joiner set its own depth to `HopDepth + 1` immediately. `AcceptFlags` bit 0 **`PENDING`**: the join is granted but the slot is still held by a child being drained, and the acceptor lacks the reserve headroom to serve both (Ch1 §1.2.2 *The Drain Path*); pushing begins when the drained child releases, at the latest at that drain's deadline. The joiner keeps any parent it already holds in the tree until the first block arrives. Other bits reserved, zero.

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
This is the response `send_udp_probe()` consumes in the tree-join algorithm (Ch1 §1.2 §2.2). **Every per-tree field refers to the tree named by the PROBE.** `K_avail` is the responder's free slots in that tree, computed against that tree's declared bitrate (Ch1 §1.2 §1.3). `LiveEdgeSegmentSeq` is the highest segment the responder has **verified in that tree** — $0$ if none; segment numbering starts at $1$ (§D.4.8) — and is *not* the node-level live edge of Ch4 §4.3.1. `Flags` bits 0–4 have the Peer Record semantics of Ch2 §2.3.3; bits 5–6 are **`TreeState`** for the probed tree: `00` `SERVING` (has a parent and $\ge 1$ verified segment there), `01` `WARMING` (has a parent, no verified segment yet), `10` `UNPARENTED` (assigned to the tree but currently without a parent in it, Ch1 §1.1.5 §5.3 item 4), `11` reserved. `AssignedTrees` sets every assigned tree's bit, including any coverage-granted tree (Ch1 §1.2 §1.3). The joiner's use of `TreeState` is the table in Ch1 §1.2.2: only `WARMING` exempts a round from the shed rule. An earlier layout keyed warming on a node-level `LiveEdgeSegmentSeq = 0`, which misclassified every relay that was warming in one tree while serving another.

### D.4.8 MANIFEST (0x11) / MANIFEST_UPDATE (0x17)
```text
MANIFEST:
[Header][StreamID 32B][SegmentSeq 4B][ChunkIndex 1B][ChunkCount 1B][Reserved 2B][Timestamp 8B µs][MerkleRoot 32B]
[BlockCount 2B][ChunkByteLength 4B][SlicingMatrixVersion 1B][LayerCount 1B][Reserved 2B]
[ [LayerBlockCount 2B][LayerByteLength 4B] × LayerCount ]
[Ed25519 Source Signature 64B]

MANIFEST_UPDATE:
[Header][StreamID 32B][SegmentSeq 4B][ChunkIndex 1B][ChunkCount 1B][Reserved 2B][Timestamp 8B µs][MerkleRoot 32B]
[BlockCount 2B][ChunkByteLength 4B][SlicingMatrixVersion 1B][LayerCount 1B][Reserved 2B]
[ [LayerBlockCount 2B][LayerByteLength 4B] × LayerCount ]
[EffectiveSegmentSeq 4B][NumTrees 1B]
[TreeID 1B][Layer 1B][StripeIndex 1B][StripeCount 1B][Priority 1B][BitrateKbps 2B]   × NumTrees
[Ed25519 Source Signature 64B]
```
The signature covers every preceding byte after the header.

*   **One `MANIFEST` per 250 ms chunk** (Ch4 §4.1.1): `ChunkIndex` $\in [0, \text{ChunkCount})$, `ChunkCount` $= 4$. The Merkle root and block counts are the chunk's. Manifests are admitted by sequence window and duplicates are never penalised (Ch7 §7.1.2).
*   **`LayerBlockCount` / `LayerByteLength`** give, for each layer in this chunk, lowest layer first, the number of 16 KB blocks and the exact byte length of the layer's payload; the last block of each layer is zero-padded to 16 KB for hashing and FEC, and `LayerByteLength` is what strips the padding (Ch4 §4.1.1). `BlockCount` is the sum of the block counts and `ChunkByteLength` the sum of the byte lengths. An earlier layout carried only the chunk total, which cannot locate the padding of any layer but the last. Within the chunk, blocks are numbered $j$ across layers in that order, so block $j'$ of layer $l$ has $j = \sum_{l' < l} \text{LayerBlockCount}_{l'} + j'$, travels on the tree of layer $l$ with `StripeIndex` $= j' \bmod \text{StripeCount}$ (Ch1 §1.2 §4.2.1), and carries the global `BlockIndex = ChunkIndex << 12 | j` in every block-addressed frame. Without these counts a peer cannot tell which tree a missing block belongs to.
*   **Slicing matrix** entries are exactly the output of the allocation rule of Ch1 §1.2 §4.2.1. `BitrateKbps` is the tree's declared bitrate and is the value every slot computation uses (Ch1 §1.2 §1.3). All trees of one layer share a `Priority`; lower is shed first; the layer-0 trees are never shed.
*   **`EffectiveSegmentSeq`** is the first segment emitted on the new layout — the end of the 5-segment migration window (Ch1 §1.2 §4.5). A `MANIFEST_UPDATE` is accepted only if `SlicingMatrixVersion` is strictly greater than the last accepted version. During the window the source additionally emits, on every tree that exists only in the new layout, that tree's new-layout stripe (Ch1 §1.2 §4.5 step 4); those blocks are duplicates of blocks the old layout is also carrying and are identified by the same `BlockIndex`, which does not depend on the matrix.
*   **`SegmentSeq` starts at $1$.** Segment $0$ is never emitted, so a zero `LiveEdgeSegmentSeq` in `PROBE_RESPONSE` (§D.4.7) or a zero `EffectiveSegmentSeq` in the Stream Record (Ch2 §2.3.3) unambiguously means *none*.

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
[K_avail 2B][AssignedTrees 1B][NodeClass 1B]
[K_s 32B][Uploader NodeID 32B][Downloader NodeID 32B][Timestamp 8B µs][Downloader Signature 64B]
```
The canonical layout and field semantics are in Ch5 §5.2.3. One receipt per **segment per tree** (not per block): the bitmap's bit $\text{ChunkIndex} \cdot 128 + j$ marks block $(\text{ChunkIndex}, j)$ as delivered by the uploader on this tree and Merkle-verified. `LossRate` is the child→parent loss report consumed by adaptive parity (Ch4 §4.2.2) and the slot-count overhead factor (Ch1 §1.2.1). `RxFlags`: bit 0 `RELAYED`, bit 1 `PULL` (then `TreeID = 0`). **`K_avail`, `AssignedTrees`, `NodeClass`** are the downloader's **child advertisement**: its free slots in `TreeID` (against that tree's declared bitrate), its current assignment bitmap and class. This is the only child → parent frame that recurs at the roster cadence, and it is what the parent's `ROSTER` (§D.4.15) is built from; without it the parent had no path to a child's free slots at all. The three fields are inside the signature but carry no evidentiary meaning to a third party, which ignores them.

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
A joiner that wants to `PROBE` a `CONE`-reachability candidate (Ch2 §2.3.3 `Flags`) sends the request form to the **referrer** — the guardian whose `GET_PEERS` response named the candidate, or the gossip neighbour whose `SHUFFLE`/`GOSSIP_EXCHANGE` did. The referrer builds the requester's Peer Record from the **observed** source address of the request (never from a self-declared field) and forwards it to the target. A gossip referrer reaches the target over its open session. A guardian reaches it only through the UDP mapping the target's own traffic to the guardian opened, and **a `CONE` registrant must keep that mapping alive**: it sends a `PING` (§D.4.1) to every guardian it registered with every $\tau_{\text{nat}} = 25$ s (Ch2 §2.3.2). The target sends a `PING` to the requester's address, opening its NAT mapping; the requester's `PROBE` that follows within $1$ s then passes. Cost: one extra round-trip for `CONE` parents, none for `PUBLIC`. `SYMMETRIC` targets are never probed (Ch1 §1.2.5). A requester whose `PROBE` times out after a guardian-forwarded punch retries once through a gossip referrer if it has one for that target, and otherwise marks the record *unpunchable* locally for one registration period so the next round does not repeat the timeout. This is the "gossip signaling path" of Ch6 §6.2.1, made concrete.

### D.4.16 MANIFEST_REQUEST (0x1D)
```text
[Header][SegmentSeq 4B][ChunkIndex 1B (0xFF = every chunk of the segment; 0xFE = the pending MANIFEST_UPDATE; 0xFD = the current and any pending STREAM_DESCRIPTOR)][Reserved 3B]
```
Sent on the QUIC control stream to a parent or Active Set neighbour; answered with the corresponding `MANIFEST` frame(s) from the responder's retained window ($\tau_{\text{retain}}$), or nothing if it no longer holds them. The push path delivers only manifests signed after a peer connected, so late-join backfill (Ch4 §4.3.1 step 3) and any peer repairing a segment whose manifest it missed need this explicit fetch. `ChunkIndex = 0xFE` (with `SegmentSeq` ignored) asks for the most recent `MANIFEST_UPDATE` whose `EffectiveSegmentSeq` has not yet passed; a peer joining inside a migration window uses it to obtain the signed pending matrix (Ch2 §2.3.3). `ChunkIndex = 0xFD` asks for the `STREAM_DESCRIPTOR` (§D.4.20) in force and, if one is pending, the pending one too; a joiner sends it in the same request as its segment-$X$ manifests, so it costs no round-trip of its own.

### D.4.20 STREAM_DESCRIPTOR (0x1F)
```text
[Header][StreamID 32B][DescriptorVersion 4B][EffectiveSegmentSeq 4B][ContentType 1B][LayerMode 1B][LayerCount 1B][Reserved 1B]
[ per layer, lowest first:
  [CodecTag 4B (FourCC)][Width 2B][Height 2B][FrameRateMilli 4B][BitrateKbps 2B][InitLength 2B][InitData ≤ 4096B] ] × LayerCount
[Ed25519 Source Signature 64B]
```
Source-signed and pinned like a manifest (Ch7 §7.1.1). It carries **what the decoder needs and nothing the protocol reads**: the forwarding, verification, discovery and incentive layers are payload-agnostic and stay so. `ContentType`: `0x01` `VIDEO_CMAF` (fMP4/CMAF media segments; `InitData` is the `ftyp`+`moov` initialisation segment), `0x02` `VIDEO_ANNEXB` (`InitData` is the parameter sets, may be empty when repeated in-band), `0x03` `AUDIO`, `0x7F` `OPAQUE` (bytes with no protocol-known structure; `CodecTag` is the application's). `LayerMode`: `0x01` `SVC_SPATIAL`, `0x02` `SVC_TEMPORAL`, `0x03` `SIMULCAST` (independent renditions, the highest received is decoded), `0x04` `INDEPENDENT` (unrelated streams, e.g. video plus audio). Layer $l$ here is layer $l$ of the slicing matrix (§D.4.8); a layer whose `CodecTag` names a non-video codec is a separate track whatever the `LayerMode`. `DescriptorVersion` is strictly increasing; a new descriptor takes effect at `EffectiveSegmentSeq`, announced at least five segments ahead exactly as a `MANIFEST_UPDATE` is (Ch1 §1.2.4 §4.5), pushed down every tree once, and retained by every peer alongside the previous version. The Stream Record (Ch2 §2.3.3) carries the version and Blake3 hash of the descriptor in force, so a joiner learns which one it needs in the round-trip that anchors it and fetches the body from its first parent with `MANIFEST_REQUEST(0xFD)`. Size $\le 13$ KB at three layers with 4 KB of initialisation data each; typically under 3 KB. An earlier draft moved verified bytes to a million viewers and never said what they were.

### D.4.17 RANK_PROOF (0x19)
```text
[Header][ReceiptCount 4B][CommitSegmentSeq 4B][ReceiptListRoot 32B][Prover NodeID 32B]
[Prover Signature 64B over (ReceiptCount ‖ CommitSegmentSeq ‖ ReceiptListRoot ‖ Prover NodeID)]
[NonceSegmentSeq 4B][NonceChunkIndex 1B][PairCount 1B (= 8)][Reserved 2B]
[ per pair: [LeftLength 2B][PROOF_OF_UPLOAD body (left leaf)][RightLength 2B][PROOF_OF_UPLOAD body (right leaf; RightLength = 0 for the zero leaf)]
            [MerklePath 32B × (⌈log2 ReceiptCount⌉ − 1)] ] × PairCount
```
Sent on the QUIC control stream immediately after a `NEIGHBOR(TreeID > 0)`, or attached to a `REPUTATION_AUDIT_GOSSIP` (§D.4.18). The receipt list is **sorted by the canonical key** $(\text{SegmentSeq}, \text{TreeID}, \text{RxFlags}, \text{Downloader NodeID})$, strictly increasing, and its Merkle tree is over the leaf hashes $\text{Blake3}(\text{receipt body})$ padded with zero leaves to the next power of two. Each sample is the **adjacent pair** $(2p, 2p+1)$ selected by $p = \text{Blake3}(\text{ReceiptListRoot} \parallel \text{Nonce} \parallel i) \bmod \lceil \text{ReceiptCount}/2 \rceil$ with the path from the pair's parent node to the root; the verifier recomputes both leaf hashes and checks $\text{key}(\text{left}) < \text{key}(\text{right})$, which is what binds `ReceiptCount` to the number of *distinct* receipts. A zero right leaf is accepted only for the last pair when `ReceiptCount` is odd. There is no `TotalBytes`: the verifier computes the rank from `ReceiptCount` and the sampled receipts' own capped byte values (Ch5 §5.2.2). Semantics — commitment, nonce rule (`NonceSegmentSeq > CommitSegmentSeq`, the nonce being that chunk's `MANIFEST.MerkleRoot`), the per-receipt cap and the verifier checks — are in Ch5 §5.2.2. A proof with any failing check is a verification failure (Ch7 §7.1.1). Size $\le 10$ KB at a super node's receipt volume ($8 \times (2 \times 250 + 19 \times 32)$ bytes at $2^{20}$ receipts).

### D.4.18 REPUTATION_AUDIT_GOSSIP (0x22)
```text
[Header][Accuser NodeID 32B][Count 1B (≤ 2)][Reserved 1B][RankProofLength 2B]
[RANK_PROOF body (§D.4.17, without header) — RankProofLength bytes, 0 if omitted]
[ per accusation: [Suspect NodeID 32B][EvidenceType 1B][Reserved 1B][EvidenceLength 2B][Evidence ...][Timestamp 8B]
                  [Accuser Signature 64B over the accusation's preceding bytes] ] × Count
```
`EvidenceType` `0x01` `SYMMETRIC_TREE_PAIR` (evidence: two `PROOF_OF_UPLOAD` bodies), `0x02` `EQUIVOCATION` (evidence: two contradictory statements signed by the suspect). The attached `RANK_PROOF` is the **accuser's own**, with `Prover NodeID = Accuser NodeID`; it is what lets a receiver that is not the accuser's tree neighbour establish the accuser's standing (Ch5 §5.3.3). It is verifiable by anyone holding the nonce manifest, so a forwarder re-sends it unchanged. An accuser may omit it (`RankProofLength = 0`) only when sending to a current tree parent or child, which ranks it already. Receivers re-verify evidence before counting, count only accusers they can rank, and forward only accusations they counted (Ch5 §5.3.3).

### D.4.19 DRAIN_NOTICE (0x1E)
```text
[Header][TreeID 1B][Reason 1B][Scope 1B (0x00 = this child; 0x01 = every child in TreeID)][Reserved 1B][DeadlineSegmentSeq 4B]
```
Sent by a parent to a child on the tree's QUIC stream when the parent will release that child without dropping it: `Reason` is drawn from the `DISCONNECT` code space (§D.3) — `PREEMPTED` (rank admission), `DISPLACED` (leaf share), `REASSIGNED` (the parent moves at a forest resize), `DEMOTED` (`RELAY` → `LEAF`), `CAPACITY` ($K_v(m)$ fell), `DEPTH` ($h \ge D_{\max}$). The parent **keeps serving** until `DeadlineSegmentSeq` (current segment $+\ \tau_{\text{drain}}$, or `EffectiveSegmentSeq` at a resize) or until the child sends `DISCONNECT_CHOKE`, whichever is first, then sends `DISCONNECT(Reason)`. On receipt the child re-selects a parent for `TreeID` at once with its current parent still delivering; with `Scope = 0x01` the parent's children run the Deputy election of Ch1 §1.2.3 over their latest `ROSTER`, using the notice as the trigger. Full semantics: Ch1 §1.2.2 *The Drain Path*. An earlier draft had parents "announce" drains with no frame; the child then learned of its release only at `DISCONNECT`, after the window it was meant to use.

### D.4.15 ROSTER (0x18)
```text
[Header][TreeID 1B][RosterSeq 4B][ChildCount 2B][EntryCount 1B]
[NodeID 32B][K_avail 2B][NodeClass 1B][Flags 1B][AddrFamily 1B][IP 4/16B][Port 2B]   × EntryCount
```
Sent by a relay parent to every child in tree `TreeID` on the tree's QUIC stream every $	au_{	ext{roster}}$. `ChildCount` is the parent's total children in this tree ($k$ in the election formulas). The entries are the parent's **top $R_{	ext{roster}} = 8$ children that are themselves relays of `TreeID`** (bit `TreeID`$-1$ of their `AssignedTrees` set), ordered by their advertised `K_avail` **in this tree** descending, ties by lowest NodeID; children that do not relay this tree are never listed, since they could not accept an orphan's `RELAY_JOIN_REQUEST(TreeID)`. Addresses are included because siblings are not generally in one another's Active Sets. **Data sources:** `K_avail`, `AssignedTrees` and `NodeClass` come from each child's most recent `PROOF_OF_UPLOAD` for this tree (§D.4.12) — the child advertisement, refreshed once per segment — and from its `NEIGHBOR` at admission; `Flags` (reachability) and the address are the parent's **observed** values for the session, never the child's own claim. A child whose last advertisement is older than $2$ segments is dropped from the roster. Full election semantics: Ch1 §1.2.3.
