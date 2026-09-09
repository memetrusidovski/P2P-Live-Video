# 3. Connection Handover and Mobile Tolerance

When a node experiences a physical network layer change (such as a mobile phone switching from a WiFi access point to a cellular 5G network):

1.  **IP Invalidation:** The IP address changes, which would typically invalidate standard TCP connections and force a return to the `BOOTSTRAP` state.
2.  **QUIC Migration:** Our protocol bypasses this by utilizing **QUIC Session Migration**.
3.  **QUIC-Native Path Migration:** No protocol frame is needed for the handover itself. QUIC uses a persistent 64-bit connection identifier (`ConnectionID`) independent of the IP/Port 4-tuple, so the active sockets migrate seamlessly via QUIC's built-in path validation (see Chapter 6 §6.2.2).
4.  **State Preservation:** The state remains in `ACTIVE`, preventing any freeze in video playback during physical transit.
