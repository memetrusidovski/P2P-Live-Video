# 2. Replay Attack Prevention

The following pseudocode outlines how incoming manifests are validated and verified against replay attacks:

```python
import time
import cryptography  # Ed25519 library

# Rolling window of verified sequence numbers to prevent replay attacks
verified_segments_cache = {}

def validate_incoming_manifest(packet, pinned_pubkey, allowed_time_drift_seconds=2.0):
    # 1. Parse fields from binary frame
    stream_id = packet.read_bytes(32)
    seq_num = packet.read_uint32()
    timestamp = packet.read_uint64()
    merkle_root = packet.read_bytes(32)
    signature = packet.read_bytes(64)
    
    # 2. Prevent Replay: Check absolute timestamp drift
    current_time = int(time.time())
    if abs(current_time - timestamp) > allowed_time_drift_seconds:
        return False  # Stale manifest packet detected
        
    # 3. Prevent Replay: Check sequence number ordering
    if seq_num in verified_segments_cache:
        return False  # Duplicate/replayed manifest detected
        
    # 4. Cryptographic Validation
    payload = stream_id + seq_num.to_bytes(4, 'big') + timestamp.to_bytes(8, 'big') + merkle_root
    try:
        pinned_pubkey.verify(signature, payload)
    except InvalidSignature:
        return False  # Failed signature check - malicious source!
        
    # 5. Cache and validate
    verified_segments_cache[seq_num] = merkle_root
    return True
```
