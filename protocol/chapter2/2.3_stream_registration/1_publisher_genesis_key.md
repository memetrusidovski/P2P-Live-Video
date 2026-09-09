# 1. Publisher Genesis Key

Every stream must be cryptographically anchored to its publisher. It is critical that the DHT guardians reject any peer registrations that attempt to hijack the stream.

*   **Decoupled Stream IDs:** The `StreamID` is mathematically derived from the publisher's Ed25519 public key:
    $$\text{StreamID} = \text{Blake3}(PK_{\text{Publisher}})$$
*   **Access Control Verification:** When a peer registers for `StreamID`, the registration payload must carry a delegation certificate signed by $PK_{\text{Publisher}}$ or be signed by the peer itself with their own identity, indicating their role as a consumer/uploader. DHT guardians reject any updates attempting to modify the core stream metadata unless signed directly by the publisher's private key $SK_{\text{Publisher}}$.

## The Publisher Stream Record (Live-Edge Anchor)

Alongside the per-peer registrations, the publisher maintains a single authoritative **Stream Record** at the key $K_s = \text{Blake3}(\text{StreamID})$. This record is what allows a viewer joining mid-stream to synchronize to the live edge — without it, a late joiner has no way to know the current segment sequence number and could start playback from segment 0.

```json
{
  "stream_id": "blake3_hash_of_pubkey",
  "publisher_pubkey": "<Ed25519 public key>",
  "manifest_version": 4,
  "slicing_mode": "SVC_SPATIAL",
  "num_trees": 6,
  "register_sample_log2": 0,
  "swarm_size": 1834,
  "relay_count": 1102,
  "live_edge_segment_id": 3847,
  "live_edge_manifest_hash": "<Blake3 Merkle root of segment 3847>",
  "live_edge_timestamp": 1750123456000000,
  "tree_mapping": [ {"tree_id": 1, "layer": 0, "stripe": 0, "stripe_count": 2, "priority": 100, "bitrate_kbps": 750}, ... ]
}
```

*   **Republish cadence:** The publisher re-signs and re-publishes this record every $1.0\text{ s}$ — once per segment — so `live_edge_segment_id` is never more than one segment stale. The $\tau_{\text{ttl}} = 180\text{ s}$ record TTL (Appendix B) provides ample margin.
*   **`swarm_size` / `relay_count`:** The publisher's current count of **active** registered peers, and of those the `RELAY`-class ones, as reported by the DHT guardians (§2.3.2; a registration is active if refreshed within 135 s, Appendix B). `swarm_size` drives the adaptive JOINING threshold (Ch1 §1.3) and the adaptive Proof-of-Work difficulty (Ch2 §2.2); `relay_count` drives the dynamic forest ladder (Ch1 §1.2.1 §1.4), which must not be keyed on a count that includes peers unable to relay.
*   **`live_edge_timestamp`:** Microsecond UTC timestamp of the live-edge segment's creation, used only for coarse staleness checks — playback synchronization is driven by segment sequence numbers, never wall-clock time.
*   **`register_sample_log2`:** the registration sampling exponent $s$ (§2.3.2): a peer registers only if its NodeID falls in a $2^{-s}$ sample. $0$ until the swarm passes $10^4$ peers.
*   **Authenticity:** The record is signed by $SK_{\text{Publisher}}$; guardians and clients reject any record whose signature does not verify against the public key that hashes to `stream_id`. The byte-exact encoding that the signature covers is given in §2.3.3 — this JSON is illustrative only.
