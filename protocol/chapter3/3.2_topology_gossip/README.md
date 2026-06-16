# 3.2 Gossip-based Topology Synchronization

This subchapter details the specific binary packets used to manage connections and shuffle candidate peers to maintain swarm health.

## Deep Dive Topics:
*   [1. Shuffle and Neighbor Frames](1_shuffle_neighbor_frames.md): Binary layouts for `NEIGHBOR`, `SHUFFLE`, `DISCONNECT`.
*   [2. HMAC Security](2_hmac_security.md): Transient session keys and replay-sequence tracking.
