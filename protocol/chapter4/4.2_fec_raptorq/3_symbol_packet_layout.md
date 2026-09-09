# 3. RaptorQ Symbol Packet Layout

Every RaptorQ packet carries precise metadata indicating its block alignment to allow single-pass decoding:

```text
RAPTORQ_SYMBOL Frame (Type 0x12):
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|   Version     |  Type (0x12)  |          Payload Length       |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                       Segment Sequence Number                 |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|   Source Block Number (SBN)   |   Encoding Symbol ID (ESI)    |
|            (16-bit)           |           (16-bit)            |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                               |
|              RaptorQ Symbol Payload Data (1024 bytes)         |
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

*   **Source Block Number (SBN):** The **global block index** of Ch4 §4.1 — `ChunkIndex << 12 | j` — identifying the 16 KB Merkle block within the segment and, through its upper bits, the chunk whose `MANIFEST` root verifies it. One RaptorQ source block corresponds to exactly one Merkle block: $K = 16$ source symbols of 1024 bytes each ($K_{\text{block}}$, Appendix B). The segment is identified separately by the Segment Sequence Number field.
*   **Encoding Symbol ID (ESI):** Identifies the specific symbol index. Values of $\text{ESI} < 16$ correspond to original source symbols (the block's 16 raw 1 KB slices, in order), while values of $\text{ESI} \ge 16$ indicate generated parity symbols. Repair symbols served in response to a `PULL_REQUEST` start from a per-responder derived base rather than at $\text{ESI} = 16$, so that a peer repairing from several neighbours receives distinct symbols instead of duplicates (Ch4 §4.1.2).
*   **Symbol Size:** Fixed to 1024 bytes to prevent IP fragmentation over standard internet MTU limits ($1500$ bytes).
*   **Padding:** The final block of **each layer** in a chunk is zero-padded to the full 16 KB for hashing and FEC; the true byte length of each layer's payload travels in the chunk `MANIFEST`'s `LayerByteLength` field (Appendix D §D.4.8). An earlier draft named a `SegmentByteLength` field the manifest never had.
