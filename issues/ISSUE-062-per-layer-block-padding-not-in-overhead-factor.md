# ISSUE-062: Per-layer 16 KB block padding costs 10–15 % of the reference ladder at 250 ms chunks, and the overhead factor does not include it

**Status:** Open
**Priority:** Medium
**Component:** Ch4 §4.1.1 chunks and layer-major block numbering; Ch1 §1.2.1 §1.3 slot count and $\Omega$; Appendix B $f_{\text{frame}}$
**Affects:** Every relay's slot count and every tree's declared bitrate; worst for the smallest per-tree stripes ($0.75$ Mbps at $M = 6$)
**File:** `protocol/chapter4/4.1_segment_serialization/1_gop_serialization.md`, `protocol/chapter1/1.2_multi_forest_overlays/1_graph_theory_and_slicing.md`, `protocol/appendix_b_parameters.md`

---

## Summary

Blocks are numbered layer-major within a 250 ms chunk and every block is exactly 16 KB, so **each layer** in each chunk is zero-padded to a block boundary. At the reference ladder a chunk carries $\approx 47 / 47 / 94$ KB for $L_0 / L_1 / L_2$, i.e. $2.9 / 2.9 / 5.7$ blocks, which round up to $3 / 3 / 6$: $12$ blocks, $196{,}608$ bytes on the wire for $187{,}500$ bytes of media — $4.9\%$ at these exact sizes and, for arbitrary layer sizes, an expected half block ($8$ KB) per layer per chunk, $\approx 13\%$ of the reference chunk. The slot count $K_v(m) = \lfloor (1 - r_{\text{pull}})\,u_v / (t_v B_m \Omega_v) \rfloor$ uses $\Omega = (1 + \bar E/K)(1 + f_{\text{frame}})$ with $f_{\text{frame}} = 0.03$ and never sees this padding, so relays advertise more slots than their uplink carries. The simulator reproduced the effect directly: adding four bytes of framing per layer pushed every layer over a block boundary, block counts went from $12$ to $14$ per chunk, and the 100-relay baseline went from $0.02\%$ to $31\%$ starvation with no other change.

## Detailed Description

Spec text that fixes the geometry:

*   Ch4 §4.1.1: "Blocks within a chunk are numbered $j = 0, 1, \ldots$ layer-major (all of $L_0$'s blocks, then $L_1$'s, …)"; App D §D.4.8: "`LayerBlockCount` gives the number of 16 KB blocks of each layer in this chunk". A layer therefore owns whole blocks; its last block is padded.
*   Ch4 §4.2.3: "The final block of a segment is zero-padded to the full 16 KB for hashing and FEC" — one padded block per *segment* was the intent, written before chunks and layer striping existed (SOLUTION-019/033).
*   Ch1 §1.2.1 §1.3: $\Omega_v = (1 + \bar E_v/K)(1 + f_{\text{frame}})$; App B: $f_{\text{frame}} = 0.03$ "framing overhead fraction on media bytes".

Padding per layer per chunk is uniform on $[0, 16)$ KB unless the encoder targets block multiples, so the expected wire bytes for a layer of $b$ bytes are $b + 8$ KB. Per 250 ms chunk on the reference ladder:

| Layer | Media bytes | Expected wire bytes | Overhead |
| :--- | ---: | ---: | ---: |
| $L_0$ (1.5 Mbps) | 46,875 | 55,067 | 17.5 % |
| $L_1$ (1.5 Mbps) | 46,875 | 55,067 | 17.5 % |
| $L_2$ (3.0 Mbps) | 93,750 | 101,942 | 8.7 % |
| chunk | 187,500 | 212,076 | **13.1 %** |

At $M = 6$ the $L_0$ layer is striped over two trees but padding is per *layer*, not per stripe, so the relative cost is unchanged; a relay assigned to a $0.75$ Mbps tree computes $K_v$ against $B_m = 750$ kbps while its share of the padded layer is $\approx 880$ kbps. With $f_{\text{frame}} = 0.03$ the slot count is over by $\approx 10\%$ — one child in ten on every relay is unfunded, which the egress queue turns into loss and the loss into starvation, exactly the spiral Ch4 §4.2.2 warns about for parity.

Real encoders make it worse than the table: fragment sizes vary $\pm 30\%$ around the target bitrate, so the padded size varies block-by-block and the *declared* `BitrateKbps` in the slicing matrix has no stable relation to the bytes a tree actually carries.

## Impact

*   Slot counts are systematically optimistic by roughly the padding fraction; the capacity conditions of Ch1 §1.1.5 are evaluated on media bitrate but paid in wire bytes.
*   Small trees pay most: the same 8 KB expectation is $17\%$ of a $1.5$ Mbps layer and would be $35\%$ of a $0.75$ Mbps layer if layers were ever sized that small.
*   Any per-layer framing (the descriptor of ISSUE-060, a length prefix) adds bytes to every layer and can tip a layer over a boundary, as the simulator showed.

## Proposed Fix

Two independent parts.

1.  **Charge padding in $\Omega$.** Define the padding fraction from the mapping the source publishes: $p_m = \dfrac{8\,\text{KB} \cdot 4 \cdot L_m}{B_m \cdot 1\,\text{s}}$ where $L_m$ is the number of layers whose stripes tree $m$ carries (one for a striped layer, the bundle size for $M < L$), or simply have the source publish the *measured* mean wire bitrate per tree as `BitrateKbps` (it knows every block it emits). The second is simpler and self-correcting for real encoders: `BitrateKbps` becomes "bytes the tree carries per second including padding", and $K_v$ needs no new term. State which one in Ch1 §1.3 and App B.
2.  **Pack the padding once per chunk, not once per layer.** Allow a layer to *start mid-block*: number blocks over the concatenation of all layers' bytes with per-layer byte lengths in the manifest (`LayerByteLength 4B × LayerCount` alongside `LayerBlockCount`), and assign a block that straddles two layers to the tree of the *lower* layer. Padding drops to one partial block per chunk ($\approx 8$ KB, $4\%$). The cost is that a shed of the upper layer leaves one straddling block on the lower tree that is $\le 16$ KB larger than needed — negligible — and that the per-layer block count in `PROOF_OF_UPLOAD` bitmaps becomes a byte range. If that touches too much, keep layer-major whole blocks and do part 1 only; part 1 alone makes the accounting honest.

Either way, add `LayerByteLength` to the manifest: without it a consumer cannot strip a middle layer's padding (the implementation currently prefixes each layer with its length inside the payload, which is what triggered this ticket; see the ISSUE-060 addendum).

## Effort

Part 1: small (one formula or one sentence about `BitrateKbps`, one App B row). Part 2: medium (manifest layout, block-to-tree rule, PoU bitmap semantics).
