# ISSUE-004: Late-Joiner Live-Edge Sync Is Undefined in the Protocol

**Status:** Open  
**Priority:** High  
**Component:** Chapter 4 — Media Distribution / Chapter 2 — Stream Registration  
**Affects:** Every viewer that joins a stream after it has started  
**File:** `protocol/chapter4/4.3_hybrid_push_pull/1_buffer_sliding_timeline.md`, `protocol/chapter2/2.3_stream_registration/1_publisher_genesis_key.md`

---

## Summary

The protocol defines a 4-second sliding buffer window (push zone + pull zone) but does not specify how a newly joining peer determines where in the stream the live edge currently is. Without this, a new viewer has no reference point for the current segment sequence number and may start from segment 0 (watching from the beginning of the stream), miss the live edge entirely, or enter an undefined state in its buffer scheduler.

---

## Detailed Description

The buffer sliding timeline (`1_buffer_sliding_timeline.md`) defines the playback window:

```
[ Playout Deadline (T=0s) ] <--- PULL Zone ---> | <--- PUSH Zone ---> [ Live Edge (T=4.0s) ]
```

The PUSH zone assumes a peer already knows the current segment sequence number so that its parent can push the right chunks. The PULL zone assumes the peer knows which segment IDs it is missing.

A new viewer joining the stream has neither:
1. The current live-edge segment ID
2. The signed manifest for that segment (needed for Merkle verification per chapter 4.1.2)

**What the protocol currently provides:**

The stream registration (`1_publisher_genesis_key.md`) stores a `StreamID = Blake3(PK_Publisher)` in the DHT. The `REGISTER_PEER` and `GET_PEERS` RPCs (`2_store_get_rpcs.md`) return a list of active peers for a stream, but there is no field in the registration payload that carries the current segment sequence number or a pointer to the latest signed manifest.

**Failure modes without a fix:**

- **Start from segment 0:** Viewer begins watching from when the stream started. For a stream that has been live for 2 hours, this means the viewer is 2 hours behind. Unacceptable for live viewing.
- **No reference point:** The buffer scheduler (`3_rarest_first_heuristics.md`) calls `IdentifyMissingBlocks(LocalBufferState, SegmentID)` but SegmentID is undefined for a new joiner. The scheduler cannot produce valid PULL requests.
- **Stale manifest:** If the viewer contacts a peer and requests "the latest manifest," that peer may serve a segment that is already past the playout deadline by the time the network round-trips complete.

---

## Impact

- Every viewer joining a live stream is affected.
- Without a defined sync mechanism, viewer implementations will diverge — some start from 0, some guess the sequence number, producing inconsistent behaviour.
- The 4-second playout buffer is tight. A viewer that takes even 500ms to sync to the live edge is already 12.5% through the buffer before playback starts.

---

## Proposed Fix

Add a `live_edge` field to the stream registration DHT record, updated by the broadcaster on every new segment:

**DHT Stream Record (extended):**
```json
{
  "stream_id": "blake3_hash_of_pubkey",
  "publisher_pubkey": "<Ed25519 public key>",
  "slicing_mode": "SVC_SPATIAL",
  "num_trees": 6,
  "live_edge_segment_id": 3847,
  "live_edge_manifest_hash": "<blake3 root hash of segment 3847>",
  "live_edge_timestamp_utc": 1750123456,
  "tree_mapping": [...]
}
```

The broadcaster re-publishes this record to the DHT every 1 second (each new segment). The `τ_ttl = 180s` TTL from Appendix B gives plenty of margin.

**New viewer join sequence:**
1. Query DHT for stream → receive record including `live_edge_segment_id = X`.
2. Set local buffer head to segment X.
3. Receive signed manifest for segment X from parent (already required for Merkle verification).
4. Begin PUSH reception from segment X+1 onward.
5. PULL any blocks from segment X that haven't arrived yet.

The broadcaster must also maintain a short manifest cache (last 8–10 seconds of segments) so a new viewer's first PULL request for the current segment can be served by nearby peers who have already buffered it.

---

## Effort

Medium. Requires extending the DHT registration payload, adding a `live_edge_segment_id` field to the `REGISTER_PEER` frame, and defining the new-viewer bootstrap sequence in the state machine (add a sync step between CONNECTING and ACTIVE).
