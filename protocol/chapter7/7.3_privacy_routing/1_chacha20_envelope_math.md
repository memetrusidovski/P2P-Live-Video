# 1. ChaCha20 Envelope Math

To protect viewers in hostile regions with aggressive network surveillance or ISP censors, our protocol integrates an optional, low-latency **Onion-Style Privacy Layer**. 

Instead of exchanging direct socket connections with peers in the active set, a privacy-seeking client structures its transport data through three intermediate cooperative relays. Let $M_1$ be the Entry Relay, $M_2$ the Middle Relay, and $M_3$ the Exit Relay.

The client negotiates symmetric keys $K_1, K_2, K_3$ with each respective relay using ECDH. When sending a payload $P$, the client recursively wraps it in three nested cryptographic envelopes using ChaCha20-Poly1305 symmetric authenticated encryption:

$$\text{OnionEnvelope} = \text{ChaCha}_{\text{K1}} \left( M_2 \parallel \text{ChaCha}_{\text{K2}} \left( M_3 \parallel \text{ChaCha}_{\text{K3}} \left( \text{DestPeer} \parallel P \right) \right) \right)$$

```text
  [ Client ] ---> [ ChaCha_K1 (Entry M1) ] ---> [ ChaCha_K2 (Middle M2) ] ---> [ ChaCha_K3 (Exit M3) ] ---> [ Swarm Destination ]
```

*   **Entry Relay $M_1$** decrypts the outer layer using $K_1$, learns the identity of $M_2$, and forwards the packet. $M_1$ knows the client's IP but not the destination peer or payload.
*   **Middle Relay $M_2$** decrypts the middle layer using $K_2$, learns the identity of $M_3$, and forwards the packet. $M_2$ knows neither the client's IP nor the destination peer's IP.
*   **Exit Relay $M_3$** decrypts the final layer using $K_3$, extracts the raw payload $P$, and delivers it to $\text{DestPeer}$. $M_3$ knows the destination peer's IP but has no knowledge of the client's IP.

This multi-hop architecture decouples the viewer's identity (IP address) from their viewing habits, preventing ISP-level traffic analysis.
