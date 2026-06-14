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
  "slicing_mode": "SVC_SPATIAL",
  "num_trees": 3,
  "tree_mapping": [
    {"tree_id": 1, "layer": "BASE", "resolution": "480p", "priority": 100},
    {"tree_id": 2, "layer": "ENHANCE_1", "resolution": "720p", "priority": 50},
    {"tree_id": 3, "layer": "ENHANCE_2", "resolution": "1080p", "priority": 10}
  ]
}
```
If a mobile client connects to the swarm and is battery-constrained, it can intentionally choose to only join Tree 1, consuming less bandwidth while still receiving a stable 480p stream.
