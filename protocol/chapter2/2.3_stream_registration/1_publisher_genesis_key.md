# 1. Publisher Genesis Key

Every stream must be cryptographically anchored to its publisher. It is critical that the DHT guardians reject any peer registrations that attempt to hijack the stream.

*   **Decoupled Stream IDs:** The `StreamID` is mathematically derived from the publisher's Ed25519 public key:
    $$\text{StreamID} = \text{Blake3}(PK_{\text{Publisher}})$$
*   **Access Control Verification:** When a peer registers for `StreamID`, the registration payload must carry a delegation certificate signed by $PK_{\text{Publisher}}$ or be signed by the peer itself with their own identity, indicating their role as a consumer/uploader. DHT guardians reject any updates attempting to modify the core stream metadata unless signed directly by the publisher's private key $SK_{\text{Publisher}}$.
