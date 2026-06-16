# 3. Connection Handover and Mobile Tolerance

When a node experiences a physical network layer change (such as a mobile phone switching from a WiFi access point to a cellular 5G network):

1.  **IP Invalidation:** The IP address changes, which would typically invalidate standard TCP connections and force a return to the `BOOTSTRAP` state.
2.  **QUIC Migration:** Our protocol bypasses this by utilizing **QUIC Session Migration**.
3.  **Migration Frame:** The peer sends a `CONNECTION_MIGRATION` frame to its active set. Since QUIC uses a persistent 64-bit connection identifier (`ConnectionID`) independent of the IP/Port 4-tuple, the active sockets migrate seamlessly.
4.  **State Preservation:** The state remains in `ACTIVE`, preventing any freeze in video playback during physical transit.
