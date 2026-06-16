# 2.3 Stream Signature & Registration RPCs

This subchapter defines how peers actually publish their intent to share a specific live stream, acting as a ledgerless tracking system.

## Deep Dive Topics:
*   [1. Publisher Genesis Key](1_publisher_genesis_key.md): Decoupling StreamID from human-readable names.
*   [2. STORE and GET RPCs](2_store_get_rpcs.md): Registration and query timelines over the DHT.
*   [3. Registration Frame Layout](3_registration_frames.md): `REGISTER_PEER` and `GET_PEERS` byte structures.
