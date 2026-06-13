# Chapter 4: Media Distribution and Deadline-Aware Swarming

### 4.1 Segment Serialization and Blake3 Merkle-Tree Encoding

The video stream is split into 1-second standalone segments (e.g., GOP-aligned H.264/AV1 fragments). To ensure zero-trust verification on the fly, segments are split into $16\text{ KB}$ block symbols and structured as a Merkle Tree:

```text
                      [ Signed Root Hash ]
                           /        \
                    [ Hash L ]    [ Hash R ]
                     /      \      /      \
                  H(B1)    H(B2)  H(B3)  H(B4)
                    |        |      |      |
                  [B1]     [B2]   [B3]   [B4]   <-- 16KB Data Blocks
```

*   **Signed Merkle Manifest:** The broadcaster signs the Merkle Root Hash along with the segment ID:
    $$\text{Sig}_{\text{Manifest}} = \text{Sign}_{SK_{\text{Source}}}(\text{SegmentID} \parallel \text{MerkleRoot})$$
*   **Progressive Verification:** When a peer downloads Block $B_i$, it also receives the sister-node hashes (the Merkle proof path). This allows the peer to verify the mathematical validity of $B_i$ immediately, before receiving the rest of the segment.

### 4.2 Forward Error Correction (FEC) Layer: RaptorQ Packetization

To eliminate retransmission overhead under high packet loss, we integrate a **RaptorQ FEC** layer:
*   A segment of $K$ source symbols is encoded into $K + E$ symbols, where $E$ represents redundant parity symbols.
*   The receiver can reconstruct the entire segment of $K$ symbols upon receiving *any* $K$ unique symbols out of the $K + E$ pool with a high mathematical probability ($>99\%$):
    $$\text{Required Symbols} \approx K + 2$$
*   **Symbol Dispersion:** Parity symbols are distributed across different parents, neutralizing single-connection packet drop.

### 4.3 Hybrid Push-Pull Scheduling Logic and Buffer State-Bitfield Formats

To combine the speed of tree structures with the resilience of meshes, we utilize a **Hybrid Push-Pull** scheduling architecture:

```text
               Live Edge (T = Now)                 Historical Buffer (T - 5s)
  <---------------------------------------------|----------------------------->
         Proactive PUSH Zone (Sub-stream Trees) | Reactive PULL Zone (Mesh)
```

1.  **PUSH Zone (Live Edge):** The newest segment symbols are actively pushed down the trees of the Multi-Forest overlay.
2.  **PULL Zone (Buffer Repair):** If a node detects a missing symbol block as the playback deadline approaches, it switches to pull-mode, querying its mesh neighbors using compressed bitfields.

```text
BUFFER_STATE_BITFIELD Frame:
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
| Segment ID (4 bytes)                                          |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
| Starting Block Sequence Number                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
| Bitfield Length (Bytes)       | Reserved                      |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                               |
|        Bitfield Bytes (1 = Block Present, 0 = Block Missing)  |
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```
