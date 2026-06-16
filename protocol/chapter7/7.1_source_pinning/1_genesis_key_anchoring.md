# 1. Genesis Key Anchoring

To prevent stream hijacking or unauthorized video injection, the protocol mandates strict public key cryptography at the manifest layer. Every active stream is tied to a single **Genesis Public Key** $PK_{\text{Source}}$ (Ed25519 curve).

The stream's human-readable name is completely decoupled from routing. Instead, the `StreamID` itself is derived directly from $PK_{\text{Source}}$:
$$\text{StreamID} = \text{Blake3}(PK_{\text{Source}})$$

When a client joins a stream, it permanently pins $PK_{\text{Source}}$. This pinned key is used to validate all incoming segment manifests, layout configuration frames, and stream metadata updates. Any packet that fails verification against this pinned key is discarded at the transport layer, and the transmitting peer is permanently banned.

Every $1.0\text{ second}$ video segment is announced via a signed manifest packet. The manifest contains the segment's sequence index, timestamp, and the cryptographic root of the segment's Blake3 Merkle tree:

$$\text{ManifestPayload} = \text{StreamID} \parallel \text{SegmentSequenceNumber} \parallel \text{Timestamp} \parallel H_{\text{MerkleRoot}}$$
$$\text{Signature} = \text{Sign}_{SK_{\text{Source}}}(\text{ManifestPayload})$$
