# 4. Stream Slicing Architecture (MDC & SVC)

## 4.1 The Consequence of Round-Robin Slicing

In a Multi-Forest overlay, the video stream must be split into $M$ slices. The most naive approach is **Round-Robin Chunking**:
*   Chunk 1 is sent down Tree 1.
*   Chunk 2 is sent down Tree 2.
*   Chunk 3 is sent down Tree 3.

**The Fatal Flaw:** Modern video codecs (like H.264, HEVC, AV1) use temporal compression (P-frames and B-frames) that rely heavily on the previous frame (the I-frame). If Tree 2 experiences a healing delay and Chunk 2 is dropped, Chunk 3 (which arrived perfectly on Tree 3) cannot be decoded because it lacks the reference data from Chunk 2. The entire video freezes until the swarm can PULL the missing Chunk 2, creating massive playback stutter.

## 4.2 Solution 1: Scalable Video Coding (SVC)

To decouple the dependency between trees, our protocol natively supports **Scalable Video Coding (SVC)** (e.g., H.264/SVC or AV1 Scalability).

SVC encodes the video into hierarchical spatial or temporal layers:
*   **Base Layer ($L_0$):** Contains a low-resolution, low-framerate version of the video (e.g., 480p @ 30fps). This layer is completely independent and decodable on its own.
*   **Enhancement Layer 1 ($L_1$):** Contains spatial enhancement data to upgrade the video to 720p. It requires $L_0$ to decode.
*   **Enhancement Layer 2 ($L_2$):** Contains spatial enhancement data to upgrade the video to 1080p60. It requires $L_0$ and $L_1$.

### Slicing SVC to the Multi-Forest
Instead of chunking the video sequentially, we assign actual video layers to specific Trees in the Multi-Forest:
*   **Tree 1 (The Backbone):** Exclusively distributes the Base Layer ($L_0$). This tree is prioritized and heavily protected with high RaptorQ FEC redundancy.
*   **Tree 2:** Distributes Enhancement Layer 1 ($L_1$).
*   **Tree 3:** Distributes Enhancement Layer 2 ($L_2$).

**The Benefit (Graceful Degradation):** If Tree 3 collapses due to massive churn, the peer loses Layer 2. However, because Tree 1 and Tree 2 are completely separate routing paths, the peer continues to receive $L_0$ and $L_1$ flawlessly. The viewer's video player smoothly drops from 1080p to 720p without a single frozen frame or buffering wheel. Once Tree 3 heals (250ms later), the video snaps back to 1080p.

**Normative shedding order:** the `priority` value in the slicing matrix (§4.4) is not advisory — it defines the order in which trees are abandoned when the swarm lacks the capacity to sustain them. A peer sheds trees in **ascending priority** (lowest number first: the 1080p enhancement before the 720p enhancement), and **never** sheds the base-layer tree. The full saturation-detection and shed procedure, including hysteresis and the source-side base-layer reserve, is defined in [Chapter 1 §1.1.5 Capacity Adaptation](../1.1_scale_latency/5_capacity_adaptation.md).

---

## 4.3 Solution 2: Multiple Description Coding (MDC)

If the broadcaster's encoder does not support SVC, the protocol utilizes **Multiple Description Coding (MDC)**. 

MDC splits a single video feed into $M$ independent, equally important sub-streams (descriptions). A common implementation is Spatial Subsampling:
*   **Description 1:** Contains all the even-numbered scan lines of the video frame.
*   **Description 2:** Contains all the odd-numbered scan lines of the video frame.

### Slicing MDC to the Multi-Forest
*   **Tree 1:** Routes Description 1.
*   **Tree 2:** Routes Description 2.

**The Benefit:** Both descriptions are independently decodable. 
* If a peer receives both Tree 1 and Tree 2 data, the decoder interlaces them for a perfect, sharp 1080p image. 
* If Tree 2 temporarily fails, the peer only receives Description 1 (the even lines). The decoder simply interpolates (blurs) the missing odd lines. The viewer perceives a momentary drop in visual sharpness, but the framerate remains a flawless 60fps with zero buffering.

## 4.4 Protocol Slicing Header

To allow clients to understand how the trees correspond to video layers, the initial connection manifest contains the Slicing Matrix mapping:

```json
{
  "stream_id": "blake3_hash_of_pubkey",
  "manifest_version": 4,
  "slicing_mode": "SVC_SPATIAL",
  "num_trees": 3,
  "tree_mapping": [
    {"tree_id": 1, "layer": "BASE", "resolution": "480p", "priority": 100},
    {"tree_id": 2, "layer": "ENHANCE_1", "resolution": "720p", "priority": 50},
    {"tree_id": 3, "layer": "ENHANCE_2", "resolution": "1080p", "priority": 10}
  ]
}
```
A battery- or data-constrained mobile client declares itself `LEAF` class (see [5. Node Classes](5_node_classes.md)) and may subscribe to Tree 1 alone, consuming less bandwidth while still receiving a stable 480p stream — a supported operating mode with defined entitlements, not an ad-hoc opt-out.

## 4.5 Dynamic Forest Resizing (`MANIFEST_UPDATE`)

The `num_trees` field is **dynamic**: it reflects the current forest size $M$ chosen by the source from the swarm-size ladder defined in [1. Graph-Theoretic Foundations](1_graph_theory_and_slicing.md), §1.3.1. When the source changes $M$ (typically growing it as viewers arrive), the transition is coordinated as follows:

1.  **Announcement:** The source broadcasts a `MANIFEST_UPDATE` frame down all currently active trees and updates its DHT stream record. The frame carries the new manifest (including `num_trees = M_new` and the new `tree_mapping`), a monotonically increasing `manifest_version`, and the source's Ed25519 signature.
2.  **Migration window (5 seconds):** On receipt, each peer re-computes its tree assignment $a = (\text{Blake3}(NodeID) \bmod M_{\text{new}}) + 1$ and joins its new tree(s) using the standard parent-selection algorithm. The *old* trees remain fully active during this window, so playback is uninterrupted.
3.  **Drain and close:** Once the new trees are seeded (relays advertising capacity), the source stops emitting segments on the old tree layout. Peers drop their obsolete parent connections at the end of the window.

Peers ignore any `MANIFEST_UPDATE` whose signature does not verify against the pinned $PK_{\text{Source}}$ or whose `manifest_version` is not strictly greater than the last accepted version (replay protection).
