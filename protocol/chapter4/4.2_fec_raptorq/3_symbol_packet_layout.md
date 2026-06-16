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

*   **Source Block Number (SBN):** Maps the symbol to a specific video segment.
*   **Encoding Symbol ID (ESI):** Identifies the specific symbol index. Values of $\text{ESI} < K$ correspond to original source symbols, while values of $\text{ESI} \ge K$ indicate generated parity symbols.
*   **Symbol Size:** Fixed to 1024 bytes to prevent IP fragmentation over standard internet MTU limits ($1500$ bytes).
