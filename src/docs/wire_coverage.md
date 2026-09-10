# Wire coverage

All 35 frame codes from Appendix D sec. D.3 as known to `bs-wire`. "Typed" means the
payload has a struct with encode/decode and a golden vector; "Raw" means it
decodes to `Frame::Raw { frame_type, payload }` until its milestone.

| Code | Frame | Transport | bs-wire today | Needed in |
|---|---|---|---|---|
| 0x01 | PING | pre-session UDP | Typed | M1 |
| 0x02 | PONG | pre-session UDP | Typed | M1 |
| 0x03 | JOIN | pre-session UDP | Typed | M1 |
| 0x04 | STREAM_END | QUIC stream | Typed | M1 |
| 0x05 | NEIGHBOR | QUIC stream | Typed | M1 |
| 0x06 | SHUFFLE | QUIC stream | Raw | M3 |
| 0x07 | DISCONNECT | QUIC stream | Typed (ISSUE-059 rejection codes) | M1 |
| 0x08 | ACCEPTED | QUIC stream | Typed | M1 |
| 0x09 | FORWARD_JOIN | QUIC stream | Raw | M3 |
| 0x0A | REGISTER_PEER | pre-session UDP | Raw | M3 |
| 0x0B | GET_PEERS | pre-session UDP | Raw | M3 |
| 0x0C | FIND_NODE | pre-session UDP | Raw | M3 |
| 0x0D | FIND_VALUE | pre-session UDP | Raw | M3 |
| 0x0E | PROBE | pre-session UDP | Typed | M1 |
| 0x0F | PROBE_RESPONSE | pre-session UDP | Typed | M1 |
| 0x10 | BLOCK_TRANSMISSION | QUIC stream | Typed | M1 |
| 0x11 | MANIFEST | QUIC stream | Typed | M1 |
| 0x12 | RAPTORQ_SYMBOL | QUIC datagram | Typed | M1 |
| 0x13 | BLOCK_PROOF | QUIC stream | Typed | M1 |
| 0x14 | GOSSIP_EXCHANGE | QUIC stream | Raw | M3 |
| 0x15 | BUFFER_STATE_BITFIELD | QUIC stream | Raw | M3 |
| 0x16 | PULL_REQUEST | QUIC stream | Typed | M1 |
| 0x17 | MANIFEST_UPDATE | QUIC stream | Typed | M1 |
| 0x18 | ROSTER | QUIC stream | Raw | M3 |
| 0x19 | RANK_PROOF | QUIC stream | Raw | M3 |
| 0x1A | STORE_RECORD | pre-session UDP | Raw | M3 |
| 0x1B | STORE_RECORD_ACK | pre-session UDP | Raw | M3 |
| 0x1C | PUNCH_REQUEST | pre-session UDP | Raw | M4 |
| 0x1D | MANIFEST_REQUEST | QUIC stream | Typed | M1 |
| 0x1E | DRAIN_NOTICE | QUIC stream | Typed | M1 |
| 0x20 | PROOF_OF_UPLOAD | QUIC stream | Raw | M3 |
| 0x21 | CHOKE_STATE | QUIC stream | Typed | M1 |
| 0x22 | REPUTATION_AUDIT_GOSSIP | QUIC stream | Raw | M3 |
| 0x30 | RELAY_PROPOSAL | QUIC stream | Raw | M4 |
| 0x31 | RELAY_BIND | QUIC stream | Raw | M4 |

Typed: 19. Raw: 16. Records implemented: `PeerRecord`, `StreamRecord`,
`TreeMappingEntry`, `ValidationBlock`.

Pending spec decisions that will add wire structures: ISSUE-060 (stream
descriptor and a `MANIFEST_REQUEST` selector for it).
