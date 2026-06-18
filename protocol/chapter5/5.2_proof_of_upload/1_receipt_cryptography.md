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

> **Known Limitation — Reveal-Before-Prove Asymmetry:** Because $A$ must deliver the block before $B$ can sign a receipt, a malicious $B$ can accept a block and refuse to sign, obtaining the data for free. This is bounded: $A$ chokes $B$ on the very next TFT cycle ($\le 500\text{ ms}$), so the attacker gains at most one 16 KB block per eviction-reconnect cycle. Reconstructing a full 750 KB segment requires $\approx 47$ such cycles, each requiring a fresh PoW puzzle solution and a new identity, making sustained free-riding computationally expensive. A full commit-reveal scheme (where $A$ sends a hash commitment first, waits for a signed challenge, then reveals the block) would eliminate this entirely at the cost of an additional RTT per block, which is incompatible with the sub-500 ms propagation budget. The current design accepts bounded exposure as the correct tradeoff.
