# 1. Buffer Sliding Timeline

To balance propagation speed with network-loss resilience, a peer's playout buffer operates across a strict sliding timeline divided into two functional zones:

```text
                                  Buffer State Sliding Timeline
  <-------------------------------------------------------------------------------------------->
  [ Playout Deadline (T = 0s) ] <--- PULL Zone (Mesh) ---> | <--- PUSH Zone (Trees) ---> [ Live Edge (T = 3.0s) ]
  |                                                        |                                                    |
  v                                                        v                                                    v
  Video Decoder assembly                               Missing symbol blocks                        Proactive slice delivery
  must be finalized.                                   requested via bitfields.                     from parent trees.
```

The playout deadline sits $\Delta_{\text{buffer}} = 3.0\text{ s}$ behind the received live edge (Appendix B). This is the largest single term in the glass-to-glass budget (Ch1 §1.1.3), so it is set to the smallest value that leaves the PULL zone its full width; an earlier draft used $4.0$ s and, with the chunk barrier and encoder latency it omitted, overshot the 5 s objective.

1.  **PUSH Zone (Live Edge to $W_{\text{pull}}$ before the deadline):** Blocks are proactively pushed down the pre-configured Multi-Forest tree paths. No explicit request messages are sent, achieving propagation speeds close to IP multicast.
2.  **PULL Zone (the final $W_{\text{pull}}$ before the Playout Deadline):** If a child detects missing blocks due to UDP packet drops, it switches to reactive mesh-pull mode, requesting missing symbols from its active neighbor set. The zone must cover one parent repair — detection plus re-attachment, both RTT-scaled (Ch3 §3.3.1) — plus two PULL round-trips. It is therefore sized from the peer's own paths:

    $$W_{\text{pull}} = \text{clamp}\left(\tau_{\text{evict}}^{\max} + 6\,SRTT^{\max},\ 1.5\text{ s},\ 2.0\text{ s}\right)$$

    over the peer's current parents: $1.5$ s (the floor) for every path up to $\approx 150$ ms RTT, $2.0$ s on a $250$ ms path, where a repair alone takes $\approx 1.0$ s and two PULL round-trips another $0.5$. An earlier draft fixed the zone at $1.5$ s on the assumption that a repair took $\le 0.5$ s; at $250$ ms RTT the sum was $1.49$ s with the Deputy path succeeding and $\approx 1.9$ s when it failed first — no margin, then none at all, on exactly the paths the RTT-scaled deadline was introduced for. Widening the PULL zone does not change $\Delta_{\text{buffer}}$; it starts pulling a missing block earlier, at the cost of occasionally requesting a block the push path would still have delivered.

**Wire units per zone:** on the push path, each 16 KB block travels as a `BLOCK_PROOF` (0x13) frame on the tree's QUIC stream followed by the block's `RAPTORQ_SYMBOL` (0x12) datagrams. On the pull path the unit is the block: a `PULL_REQUEST` (0x16) with `MissingSymbolCount = 0` is answered by a `BLOCK_TRANSMISSION` (0x10) carrying the block plus its inline Merkle proof, while a non-zero count requests just that many repair symbols for a partially received block (see Ch4 §4.1–4.2 and Appendix D).

## Late-Joiner Live-Edge Synchronization

A peer joining mid-stream must anchor this timeline before either zone can operate: the PUSH zone assumes parents know which segment to push, and the PULL zone's `IdentifyMissingBlocks(LocalBufferState, SegmentID)` is undefined without a starting `SegmentID`. The anchor is the `live_edge_segment_id` from the publisher's signed Stream Record, which the joining peer already received in its `GET_PEERS` response during DISCOVERY (Ch2 §2.3).

**Join sequence (runs during the CONNECTING state, before ACTIVE):**

1.  Read `live_edge_segment_id = X` from the Stream Record (at most 1 segment stale, since the publisher republishes every second).
2.  Initialize the local buffer head to segment $X$; the playout deadline is set $\Delta_{\text{buffer}} = 3.0\text{ s}$ behind the live edge as usual.
3.  Obtain the signed manifests for the chunks of segment $X$ (and of any earlier segment the peer intends to backfill) **and the `STREAM_DESCRIPTOR`** whose version and hash the Stream Record named (Ch2 §2.3.3) from the first connected parent with `MANIFEST_REQUEST` (0x1D, Appendix D §D.4.16; `ChunkIndex = 0xFD` for the descriptor) — the push path delivers only manifests signed *after* the peer connected, so backfill needs an explicit fetch, and the decoder cannot consume a single verified block until it holds the descriptor's initialisation data (Ch4 §4.1.1). Both ride one request, so the descriptor adds no round-trip to Startup Join Latency.
4.  Enter ACTIVE: receive PUSH delivery from segment $X{+}1$ onward through the tree parents.
5.  PULL any blocks of segment $X$ (and any gaps) from active-set neighbors to fill the initial buffer.

To guarantee step 5 can be served, every peer retains its most recent $\tau_{\text{retain}} = 8$ seconds of verified segments and their manifests specifically for bootstrapping late joiners — the $3$ s playback window plus the up-to-$2$ s anchor lag below plus margin for the joiner's PULL round-trips. This overlaps the playback window, so it costs at most ~5 extra seconds of buffered video (~3.75 MB at 6 Mbps).

A peer must **never** derive its start position from wall-clock time or begin at segment 0; segment sequence numbers are the only synchronization authority (see Ch1 §1.3 lifecycle and the logical-clock rule in Ch7 §7.1).

### Anchor Staleness and Re-Synchronization

The Stream Record anchor is **coarse, and its error is one-directional** — it is always *behind* the true live edge, never ahead. Three delays stack up before the joining peer uses it:

| Source of lag | Magnitude |
| :--- | :--- |
| Record age at the guardian (1 s republish cadence) | $0$–$1.0\text{ s}$ |
| `GET_PEERS` lookup over $O(\log N)$ hops | $\sim 0.3$–$0.5\text{ s}$ at $N = 10^6$ |
| Parent selection and QUIC handshake | $\sim 0.2\text{ s}$ |

If the peer treated $X$ as ground truth it would set its playout $3.0\text{ s}$ behind an edge that is itself up to $\sim 1.7\text{ s}$ stale, and then **keep that offset forever** — it follows PUSH from $X{+}1$ onward and never re-measures. Two viewers in the same room whose lookups took different times would sit permanently seconds apart, which for the interactive use cases this protocol targets (sports, esports) is a product failure, not a rounding error.

The Stream Record is therefore a **bootstrap hint only**. The authoritative live edge is the highest segment sequence number actually arriving on the PUSH path:

$$X_{\text{edge}} = \max\left(\texttt{live\_edge\_segment\_id},\ \max_{p \in \text{parents}} \text{HighestPushedSeq}(p)\right)$$

1.  **Re-anchor on entry to ACTIVE.** Once parents are pushing, recompute $X_{\text{edge}}$ and set the playout deadline $3.0\text{ s}$ behind *that*, discarding the Stream Record estimate. All viewers converge on the same offset regardless of how slow their lookup was.
2.  **Correct drift continuously.** A peer whose playout position falls more than **one segment** beyond the $3.0\text{ s}$ target behind $X_{\text{edge}}$ has accumulated latency it will not recover by waiting. It re-syncs forward — skipping the gap, whose segments are past their deadline and would be dropped by the scheduler anyway (§4.3.3) — rather than letting the offset become permanent.
3.  **Never re-anchor backward.** $X_{\text{edge}}$ is monotonically non-decreasing. A parent that reports a *lower* sequence number than the peer has already seen is stale or hostile and must not move the anchor; this is the same monotonicity rule the manifest validator enforces in Ch7 §7.1.2.

### One Anchor for Every Class

Leaf-class peers (Ch1 §1.2.5) are tree children like every other subscriber and anchor at $X_{\text{edge}}$ with the same $\Delta_{\text{buffer}}$. An earlier draft had them join $5$–$10$ s behind the edge "drawing from well-replicated older segments" — which described a pull-based delivery path the protocol does not have: a tree parent pushes the current chunk and cannot push ten-second-old data, so the offset bought nothing but a longer retention window on every relay. The price of leaf class is paid elsewhere (quality ceiling and preemption order, Ch1 §1.2.5 §5.4), not in latency. The retention window above therefore needs to cover only the late-join backfill, and $\tau_{\text{retain}} = 8$ s does.
