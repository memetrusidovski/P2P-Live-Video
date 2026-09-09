# 1. Genesis Key Anchoring

To prevent stream hijacking or unauthorized video injection, the protocol mandates strict public key cryptography at the manifest layer. Every active stream is tied to a single **Genesis Public Key** $PK_{\text{Source}}$ (Ed25519 curve).

The stream's human-readable name is completely decoupled from routing. Instead, the `StreamID` itself is derived directly from $PK_{\text{Source}}$:
$$\text{StreamID} = \text{Blake3}(PK_{\text{Source}})$$

When a client joins a stream, it permanently pins $PK_{\text{Source}}$. This pinned key is used to validate all incoming chunk manifests, `MANIFEST_UPDATE` and `STREAM_DESCRIPTOR` frames (Appendix D §D.4.8, §D.4.20), and stream metadata updates. A frame whose **signature fails** against the pinned key is discarded, and the peer that delivered it is disconnected and **locally banned for $\tau_{\text{ban}} = 10$ minutes**, doubling on each repeat up to 24 h (Appendix B). The ban is local — it is never gossiped, since a bad signature proves only that *this* neighbour relayed garbage, and it is time-bounded because permanent bans on a swarm with cheap identities (Ch2 §2.2) punish only the honest. A **duplicate** manifest, or one outside the acceptance window, is *not* a verification failure (§2): every peer receives each manifest from up to $M$ parents, and treating a repeat as forgery would ban every parent within a second.

Every $250\text{ ms}$ chunk of a video segment (Ch4 §4.1.1) is announced via a signed `MANIFEST` frame (Appendix D §D.4.8). The manifest contains the segment sequence number, the chunk index, the source timestamp, the root of the chunk's Blake3 Merkle tree and the chunk's per-layer block counts; the signature covers the whole payload:

$$\text{ManifestPayload} = \text{StreamID} \parallel \text{SegmentSeq} \parallel \text{ChunkIndex} \parallel \text{ChunkCount} \parallel \text{Timestamp} \parallel H_{\text{MerkleRoot}} \parallel \text{BlockCount} \parallel \ldots \parallel \text{LayerBlockCounts}$$
$$\text{Signature} = \text{Sign}_{SK_{\text{Source}}}(\text{ManifestPayload})$$
