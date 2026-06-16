# 1. Receipt Cryptography (Non-Repudiable Ledger)

To prevent cheating, collusion, or Sybil reputation pumping (where fake nodes claim to upload to each other), we enforce a zero-trust cryptographic accounting ledger based on **Proof-of-Upload (PoU) Receipts**.

When peer $A$ delivers a valid, Merkle-verified video symbol block $B_i$ of segment $S$ to peer $B$, $B$ is cryptographically obligated to issue a signed receipt back to $A$ immediately. The receipt is non-repudiable because it is signed with $B$'s private key $SK_B$, which is bound to $B$'s verified S/Kademlia identity.

```text
       Peer A (Uploader)                     Peer B (Downloader)
              |                                       |
              | ----- 16KB Video Block (Verified) --->|
              |                                       | (Merkle passes,
              |                                       |  signs receipt)
              |                                       |
              | <---- Signed PoU Receipt -------------|
```

The PoU receipt payload contains:
$$\text{PoU} = \text{Sign}_{SK_B}(\text{StreamID} \parallel \text{SegmentID} \parallel \text{BlockIndex} \parallel \text{UploaderNodeID} \parallel \text{Timestamp})$$

If $B$ refuses to return a valid PoU receipt within $50\text{ ms}$ of block delivery, $A$ immediately chokes $B$ and evicts it from its Active Set, preventing buffer depletion by freeloaders.
