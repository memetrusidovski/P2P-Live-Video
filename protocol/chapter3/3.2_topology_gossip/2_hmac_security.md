# 2. HMAC Security and Sequence Tracking

To protect the gossip layer from metadata manipulation or unauthorized insertion of falsified peer listings:

*   **HMAC Signing:** Every gossip frame carries a 32-byte Hash-based Message Authentication Code (HMAC) generated using a shared transient symmetric session key derived via Diffie-Hellman during the initial QUIC handshake.
*   **Replay Sequence Check:** Nodes track incoming 32-bit monotonically increasing Sequence Numbers for each active peer. Any packet received with a sequence number less than or equal to the highest recorded value for that sender is instantly dropped.
