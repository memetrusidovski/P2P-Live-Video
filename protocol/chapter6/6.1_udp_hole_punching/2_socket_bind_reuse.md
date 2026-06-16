# 2. Socket Bind Reuse

For UDP hole punching to succeed, the local P2P application must utilize **Socket Bind Port Reuse**:

1.  **Shared Port Principle:** The socket used to query external STUN servers must be the *exact same socket descriptor* (with `SO_REUSEPORT` / `SO_REUSEADDR` enabled) used to establish P2P communication channels.
2.  **Mapping Consistency:** If a peer opens a separate socket for STUN queries, the NAT router will allocate a *different* external mapped port, rendering the resolved IP/Port coordinates completely useless for hole punching.
3.  **Active Port Prediction:** By querying at least two independent, geodistributed STUN servers, the node verifies if its NAT allocates port mappings sequentially (predictable Symmetric NAT) or randomly (unpredictable Symmetric NAT).
