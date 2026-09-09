# ISSUE-012: Wire Format Split-Brain — gRPC Schema Contradicts the Binary Frame Diagrams

**Status:** Resolved  
**Priority:** High  
**Component:** Cross-cutting — schemas / Ch2 / Ch3 / Ch4 / Ch5 / Ch6  
**Affects:** Any implementation attempt; interoperability  
**File:** `protocol/schemas/p2p_live.proto`, frame diagrams across chapters

---

## Summary

The spec defines two incompatible wire formats. The chapter ASCII diagrams use a fixed binary framing (`Version u8 | Type u8 | PayloadLength u16` + typed payload) over UDP/QUIC with hex type codes. The protobuf schema defines the DHT as a **gRPC service** — which implies HTTP/2 over TCP, contradicting `protocol_type = UDP` in the same file — and duplicates a `version` field per message. Two implementers reading different halves of the spec would build incompatible nodes.

## Detailed Description

- `StreamDirectoryService` (gRPC) cannot be the DHT transport: DHT RPCs precede any QUIC session and must run over plain UDP.
- The S/Kademlia validation block is **152 bytes** in ch2.2.3 but **80 bytes** in ch2.3.3's `REGISTER_PEER` diagram.
- `SKademliaIdentity.external_ip` exists in the proto but not in the ch2.2.3 frame diagram.
- No central frame-type registry exists; type codes are scattered across chapters.
- Frames referenced but never specified: `PING`/`PONG`, `JOIN`, `FORWARD_JOIN`, `ACCEPTED` (0x08), `FIND_NODE`/`FIND_VALUE`, `MANIFEST`, `PULL_REQUEST` (0x16), `CHOKE`/`UNCHOKE`/`UNCHOKE_OPTIMISTIC`, `GOSSIP_EXCHANGE`, capacity probe/response (parent selection depends on it), `RELAY_JOIN_REQUEST`, `CONNECTION_MIGRATION`.
- `NEIGHBOR` (0x05) lacks the TreeID/Slice field that Appendix A.2 uses.
- `PROOF_OF_UPLOAD` (0x20) lacks the downloader's NodeID, yet ch5.3 audits receipts third-party.

## Proposed Fix

Make the binary frames canonical; demote the proto to a non-normative logical field reference.

1. Delete the gRPC `service` block; add a header comment declaring the proto non-normative.
2. Create **`protocol/appendix_d_frame_registry.md`**: the single normative frame-type registry with transport mapping (plain UDP pre-session; QUIC streams for control; QUIC datagrams for symbols) and byte layouts for every missing frame, including `PROBE`/`PROBE_RESPONSE` (0x0E/0x0F) carrying `K_avail`, reliability, hop count, assigned trees, and node class.
3. Canonical validation block = **152 bytes**; fix ch2.3.3. Drop `external_ip` from the wire — verifiers check the dynamic PoW against the **observed UDP source address**. Reserve one byte for the C2 difficulty tier (ISSUE-011).
4. Scope the 152-byte block to pre-session plain-UDP frames only; post-handshake frames ride authenticated QUIC + HMAC (ch3.2).
5. Amend `NEIGHBOR` with a TreeID field; define `RELAY_JOIN_REQUEST` ≡ `NEIGHBOR(Priority=HIGH, TreeID=m)`; resolve `CONNECTION_MIGRATION` as QUIC-native path migration (not a frame); add `downloader_node_id` to `PROOF_OF_UPLOAD`.

---

## Resolution

Binary frames are now canonical; the proto is demoted to a non-normative field reference.

- **Created `protocol/appendix_d_frame_registry.md`** — the single normative registry: common 4-byte header (version lives only there), transport mapping (plain UDP pre-session / QUIC streams control / QUIC datagrams symbols), full type-code table, and byte layouts for all previously missing frames: PING/PONG (0x01/0x02, PONG reflects observed address), JOIN (0x03), ACCEPTED (0x08, carries hop depth), FORWARD_JOIN (0x09), FIND_NODE/FIND_VALUE (0x0C/0x0D), PROBE/PROBE_RESPONSE (0x0E/0x0F — the capacity advertisement parent selection depends on), MANIFEST (0x11), BLOCK_PROOF (0x13), GOSSIP_EXCHANGE (0x14), PULL_REQUEST (0x16), MANIFEST_UPDATE (0x17), CHOKE_STATE (0x21).
- `protocol/schemas/p2p_live.proto` — rewritten: gRPC service deleted, non-normative banner added, per-message `version` fields removed, `external_ip` dropped (dynamic PoW verified against the observed UDP source address), new messages added to mirror the registry, `downloader_node_id` added to ProofOfUpload.
- `protocol/chapter2/2.3_stream_registration/3_registration_frames.md` — validation block corrected 80 → 152 bytes.
- `protocol/chapter2/2.2_crypto_node_id/3_validation_frame.md` — block scoped to pre-session plain-UDP frames only; observed-source-address rule; threshold-based adaptive-C2 verification (no wire field needed).
- `protocol/chapter3/3.2_topology_gossip/1_shuffle_neighbor_frames.md` — NEIGHBOR gains TreeID (RELAY_JOIN_REQUEST ≡ NEIGHBOR(HIGH, TreeID=m)); registry cross-refs added.
- `protocol/chapter5/5.2_proof_of_upload/3_pou_frame.md` — Downloader NodeID added for stand-alone third-party audit.
- `protocol/chapter1/1.3_peer_lifecycle/3_connection_handover.md` — CONNECTION_MIGRATION is QUIC-native path migration, not a frame.
- `protocol/chapter2/2.3_stream_registration/2_store_get_rpcs.md` — "RPCs" clarified as plain-UDP frames.
- `protocol/INDEX.md` — Appendix D listed.
