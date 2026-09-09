# 1. Buffer Sliding Timeline

To balance propagation speed with network-loss resilience, a peer's playout buffer operates across a strict sliding timeline divided into two functional zones:

```text
                                  Buffer State Sliding Timeline
  <-------------------------------------------------------------------------------------------->
  [ Playout Deadline (T = 0s) ] <--- PULL Zone (Mesh) ---> | <--- PUSH Zone (Trees) ---> [ Live Edge (T = 4.0s) ]
  |                                                        |                                                    |
  v                                                        v                                                    v
  Video Decoder assembly                               Missing symbol blocks                        Proactive slice delivery
  must be finalized.                                   requested via bitfields.                     from parent trees.
```

1.  **PUSH Zone (Live Edge to $T_{\text{now}} - 1.5\text{ s}$):** Chunks are proactively pushed down the pre-configured Multi-Forest tree paths. No explicit request messages are sent, achieving propagation speeds close to IP multicast.
2.  **PULL Zone ($T_{\text{now}} - 1.5\text{ s}$ to Playout Deadline):** If a child detects missing blocks due to UDP packet drops, it switches to reactive mesh-pull mode, requesting missing symbols from its active neighbor set.

**Wire units per zone:** on the push path, each 16 KB block travels as a `BLOCK_PROOF` (0x13) frame on the tree's QUIC stream followed by the block's `RAPTORQ_SYMBOL` (0x12) datagrams. On the pull path the unit is the block: a `PULL_REQUEST` (0x16) with `MissingSymbolCount = 0` is answered by a `BLOCK_TRANSMISSION` (0x10) carrying the block plus its inline Merkle proof, while a non-zero count requests just that many repair symbols for a partially received block (see Ch4 §4.1–4.2 and Appendix D).

## Late-Joiner Live-Edge Synchronization

A peer joining mid-stream must anchor this timeline before either zone can operate: the PUSH zone assumes parents know which segment to push, and the PULL zone's `IdentifyMissingBlocks(LocalBufferState, SegmentID)` is undefined without a starting `SegmentID`. The anchor is the `live_edge_segment_id` from the publisher's signed Stream Record, which the joining peer already received in its `GET_PEERS` response during DISCOVERY (Ch2 §2.3).

**Join sequence (runs during the CONNECTING state, before ACTIVE):**

1.  Read `live_edge_segment_id = X` from the Stream Record (at most 1 segment stale, since the publisher republishes every second).
2.  Initialize the local buffer head to segment $X$; the playout deadline is set $4.0\text{ s}$ behind the live edge as usual.
3.  Obtain the signed manifest for segment $X$ from the first connected parent (required anyway for Merkle verification, Ch4 §4.1).
4.  Enter ACTIVE: receive PUSH delivery from segment $X{+}1$ onward through the tree parents.
5.  PULL any blocks of segment $X$ (and any gaps) from active-set neighbors to fill the initial buffer.

To guarantee step 5 can be served, every peer retains its most recent $8$–$10$ seconds of verified segments specifically for bootstrapping late joiners — this overlaps the normal 4-second playback window, so it costs at most ~6 extra seconds of buffered video (~4.5 MB at 6 Mbps).

A peer must **never** derive its start position from wall-clock time or begin at segment 0; segment sequence numbers from the signed Stream Record are the only synchronization authority (see Ch1 §1.3 lifecycle and the logical-clock rule in Ch7 §7.1).
