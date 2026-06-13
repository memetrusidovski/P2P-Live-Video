# Chapter 7: Security Hardening, Cryptography, and Threat Mitigation

### 7.1 Signed Manifests and Source Identity Pinning (Ed25519)

*   **Genesis Public Key:** Every stream is uniquely identified by the hash of its broadcaster's Ed25519 public key.
*   **Source Identity Pinning:** Upon joining the stream, clients permanently pin this key. Every manifest, configuration update, and chunk signature is validated against this pinned key. This ensures absolute protection against man-in-the-middle attacks and stream hijacking.

### 7.2 DDoS Shielding: Relay Clusters and Token Bucket Rate-Limiting

*   **Shielding Relays:** High-tier supernodes closest to the source communicate only through dynamic, rotating public relays, hiding their physical IPs from the general public.
*   **Token-Bucket Filters:** Nodes implement eBPF-based Token Bucket filters in the OS kernel. This rate-limits incoming UDP traffic before it reaches user space:

```text
  [ Incoming UDP Packets ] ---> [ eBPF Token Bucket Filter ] ---> [ Application Code ]
                                             |
                                    (Exceeds limit? Drop!)
```

This ensures that flooding attacks (DDoS) are dropped at the network interface layer, saving CPU and RAM.

### 7.3 Privacy Routing: Multi-Hop Onion-Style Layering (Optional)

For users residing in regions with strict censorship or surveillance, the protocol provides an optional onion-routing privacy layer:
*   **Multi-Hop Encrypted Tunnels:** A privacy-conscious client builds an encrypted tunnel through three cooperative relays:
    $$\text{Client} \xrightarrow{\text{Encrypted}} \text{Relay}_1 \xrightarrow{\text{Encrypted}} \text{Relay}_2 \longleftrightarrow \text{Swarm}$$
*   The actual viewer's IP address is hidden from the swarm, as only the exit relay's IP is visible. This trades some playback latency for complete anonymity, protecting viewers from network surveillance and censorship targeting.
