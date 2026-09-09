# 2. Replay Attack Prevention

The following pseudocode outlines how incoming manifests are validated and verified against replay attacks:

All protocol timestamps are **microseconds** (unsigned 64-bit), matching the validation, PoU, MANIFEST, and STREAM_END frame layouts. The drift bound is therefore expressed in microseconds too.

```python
import time
import cryptography  # Ed25519 library

ALLOWED_DRIFT_US = 2_000_000        # ±2.0 seconds, expressed in microseconds
BUFFER_RETENTION_SEGMENTS = 120     # bounded replay window (~120 s at 1 segment/s)

# Rolling window of verified sequence numbers to prevent replay attacks.
# Bounded: entries older than the retention window are evicted, so the cache
# cannot grow without limit over a long broadcast.
verified_segments_cache = {}        # seq_num -> merkle_root
highest_verified_seq = -1

def validate_incoming_manifest(packet, pinned_pubkey):
    global highest_verified_seq

    # 1. Parse fields from binary frame (Appendix D 4.8)
    stream_id   = packet.read_bytes(32)
    seq_num     = packet.read_uint32()
    timestamp   = packet.read_uint64()      # microseconds
    merkle_root = packet.read_bytes(32)
    signature   = packet.read_bytes(64)

    # 2. Prevent Replay: absolute timestamp drift (microsecond units on both sides)
    now_us = int(time.time() * 1_000_000)
    if abs(now_us - timestamp) > ALLOWED_DRIFT_US:
        return False  # Stale manifest packet detected

    # 3. Prevent Replay: reject duplicates AND out-of-order rewinds.
    #    A membership check alone would accept an old segment that had already
    #    been evicted from the cache, so ordering is checked explicitly.
    if seq_num in verified_segments_cache:
        return False  # Duplicate/replayed manifest detected
    if seq_num <= highest_verified_seq - BUFFER_RETENTION_SEGMENTS:
        return False  # Too old to be legitimate — outside the replay window

    # 4. Cryptographic Validation
    payload = stream_id + seq_num.to_bytes(4, 'big') + timestamp.to_bytes(8, 'big') + merkle_root
    try:
        pinned_pubkey.verify(signature, payload)
    except InvalidSignature:
        return False  # Failed signature check - malicious source!

    # 5. Cache, advance the ordering watermark, and evict beyond the window
    verified_segments_cache[seq_num] = merkle_root
    highest_verified_seq = max(highest_verified_seq, seq_num)
    cutoff = highest_verified_seq - BUFFER_RETENTION_SEGMENTS
    for old in [s for s in verified_segments_cache if s < cutoff]:
        del verified_segments_cache[old]
    return True
```

Note that the timestamp check is a coarse staleness guard only. Playback position and live-edge tracking are driven **exclusively by segment sequence numbers** (Ch4 §4.3), never by wall-clock time, so a peer with a drifted system clock still plays correctly — it simply cannot admit manifests that appear more than two seconds out of date.
