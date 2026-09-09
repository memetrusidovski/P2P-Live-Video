# 3. PoU Frame Layout

```text
PROOF_OF_UPLOAD Frame (Type 0x20) — one per segment per tree, downloader → uploader:
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|   Version     |  Type (0x20)  |          Payload Length       |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                       Segment Sequence Number                 |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|    TreeID     |    RxFlags    |   LossRate    |  BitmapLen(L) |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                               |
|          BlockBitmap (L bytes, L <= 64; bit = Chunk*128 + j)  |
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|     K_avail (16-bit)          | AssignedTrees |   NodeClass   |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                               |
|                  StreamID Key K_s (32 bytes)                  |
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                               |
|                  Uploader S/Kademlia NodeID                   |
|                            (32 bytes)                         |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                               |
|                 Downloader S/Kademlia NodeID                  |
|                            (32 bytes)                         |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                               |
|            Timestamp (8-byte microsecond integer)             |
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                               |
|              Ed25519 Downloader Signature (64 bytes)          |
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

*   **Size:** $184 + L$ bytes; $L$ is $\lceil (\text{blocks in the tree's stripe of this segment}) / 8 \rceil$ rounded to the chunk layout — typically $8$–$16$ bytes, at most $64$ ($4$ chunks $\times$ $128$ blocks, i.e. streams up to $\approx 64$ Mbps).
*   **`TreeID`:** the tree whose stripe this receipt covers; `0x00` with `RxFlags.PULL` for blocks served by `PULL_REQUEST` (Ch5 §5.2.1).
*   **`K_avail`, `AssignedTrees`, `NodeClass`:** the downloader's child advertisement (§5.2.1) — its free slots in `TreeID`, assignment bitmap and class — consumed by the uploader to build its `ROSTER` (Ch1 §1.2.3). Zero, `0x00` and the class code for a `PULL` receipt or a leaf-class downloader.
*   **`RxFlags`:** bit 0 `RELAYED` (issued to an emergent relay for bridged delivery, weighted $3\times$ — Ch6 §6.3.2); bit 1 `PULL`; bits 2–7 reserved, zero.
*   **`LossRate`:** $\lfloor 255 \rho \rfloor$, the downloader's symbol loss rate on this link over the trailing 2 s — the input to adaptive parity (Ch4 §4.2.2) and the uploader's overhead factor (Ch1 §1.2.1).
*   **Signature** covers every byte after the header. **Downloader NodeID** is embedded so that a receipt presented to a *third party* (a tree parent reading a `RANK_PROOF`, or an auditor, Ch5 §5.3) can be verified stand-alone: the verifier resolves the downloader's public key from its NodeID and checks the signature without having witnessed the transfer.
*   The `RANK_PROOF` (0x19) that presents a bundle of these receipts is laid out in Appendix D §D.4.17.
