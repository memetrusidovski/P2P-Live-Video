# 4. Stream Slicing Architecture (MDC & SVC)

## 4.1 The Consequence of Round-Robin Slicing

In a Multi-Forest overlay, the video stream must be split across $M$ trees. The most naive approach is **Round-Robin Chunking**:
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

The **reference layer ladder** used throughout this specification is $L_0 = 1.5$ Mbps (480p), $L_1 = 1.5$ Mbps (720p), $L_2 = 3.0$ Mbps (1080p), $B = 6$ Mbps. The ladder is a publisher choice, not a protocol constant; everything below is defined for an arbitrary ordered list of $L$ layers with bitrates $b_0 \ldots b_{L-1}$.

### 4.2.1 Layers Are Not Trees: The Allocation Rule

The forest size $M$ scales with swarm size (§1.4) while the layer count $L$ is fixed by the encoder, so the two coincide only by accident ($M = L = 3$ on the reference ladder). **The source** — never a peer — constructs the `tree_mapping` from $(M, b_0 \ldots b_{L-1})$ by the following deterministic rule, and publishes it signed in the `MANIFEST_UPDATE` (Appendix D §D.4.8). Every peer reads the mapping; no peer derives it.

**Case $M \ge L$ — layers are striped over trees.**

1.  $t_l \leftarrow 1$ for every layer $l$ (every layer gets at least one tree).
2.  Repeat $M - L$ times: give one more tree to the layer with the **largest current per-tree bitrate** $b_l / t_l$; ties go to the **lower** layer index.

The objective is minimax — the largest per-tree bitrate in the forest is as small as $M$ allows — and the tie rule sends spare trees to the base layer first. The consequence that matters is that **the trees carrying lower layers are never harder to relay than the trees carrying higher layers**: since a relay's slot count in a tree is inversely proportional to that tree's bitrate (§1.3), the tree that starves first under a capacity shortage is a top-layer tree, which is the layer the shed order (§4.2.2) drops first. An allocation that put the largest per-tree bitrate on $L_1$ would have $L_1$ starve before $L_2$ and force a two-layer shed for a one-layer shortage.

Layer $l$ is then carried by $t_l$ trees as **stripes**: the layer's blocks within a manifest chunk are numbered $j = 0, 1, \ldots$ in order, and block $j$ travels on the tree with `StripeIndex` $= j \bmod t_l$. Each of those trees carries $B_m = b_l / t_l$. Tree IDs are assigned in layer order, lowest layer first, so **Tree 1 always carries the base layer**.

**Case $M < L$ — trees carry bundles of layers.**

The ordered layer list is cut into $M$ **contiguous** bundles so as to minimise the largest bundle bitrate; ties go to the cut that gives Tree 1 the smaller bundle. Tree 1 carries the lowest bundle (which always contains $L_0$). Each tree carries $B_m = \sum_{l \in \text{bundle}(m)} b_l$. $M = 1$ is the degenerate single bundle carrying the whole stream; it is not on the forest ladder (§1.4) but the rule is defined for it so that a publisher forcing $M = 1$ produces a well-defined mapping.

**Reference ladder, every rung:**

| $M$ | Trees per layer $(t_0, t_1, t_2)$ | Per-tree bitrate $B_m$ (Mbps), Tree 1 → Tree $M$ | Shedding the top layer leaves |
| :---: | :---: | :--- | :--- |
| 2 | bundles $\{L_0{+}L_1\},\{L_2\}$ | $3.0,\ 3.0$ | 720p |
| 3 | $(1,1,1)$ | $1.5,\ 1.5,\ 3.0$ | 720p |
| 4 | $(1,1,2)$ | $1.5,\ 1.5,\ 1.5,\ 1.5$ | 720p |
| 5 | $(2,1,2)$ | $0.75,\ 0.75,\ 1.5,\ 1.5,\ 1.5$ | 720p |
| 6 | $(2,2,2)$ | $0.75,\ 0.75,\ 0.75,\ 0.75,\ 1.5,\ 1.5$ | 720p |

Two consequences are load-bearing for the rest of Chapter 1:

*   **$B_m = B/M$ is a nominal average, not a per-tree fact.** Per-tree bitrates differ by up to $2\times$ within one forest (Tree 5 vs Tree 1 at $M = 6$). Every quantity derived from a tree's bitrate — the slot count $K_v(m)$, the per-tree sustainability condition, `PROBE_RESPONSE.K_avail` — is computed against the tree's own `BitrateKbps` from the mapping (§1.3). Writing $B/M$ anywhere a per-tree value is needed over-commits the higher-bitrate trees.
*   **A striped layer is only as available as its least available tree.** Losing one of the two $L_2$ trees at $M = 6$ removes half of the layer's blocks, which is not a decodable layer. Striping trades a $t_l\times$ larger failure surface for the layer against $t_l\times$ finer capacity granularity for relays; the recovery mechanisms (sibling election, sibling PULL) operate per tree and are unchanged.

### 4.2.2 Graceful Degradation and the Shed Order

**The Benefit:** If an $L_2$ tree collapses due to massive churn, the peer loses Layer 2. Because the $L_0$ and $L_1$ trees are separate routing paths, the peer continues to receive them flawlessly. The viewer's video player smoothly drops from 1080p to 720p without a single frozen frame or buffering wheel. Once the tree heals (250 ms later), the video snaps back to 1080p.

**Normative shedding order — by layer, not by tree index.** A peer subscribes to a *layer prefix* $\mathcal{L}_{\text{sub}} = \{L_0 \ldots L_j\}$ and therefore to the tree set $\mathcal{T}_{\text{sub}}$ carrying those layers. When the shed rule of [Chapter 1 §1.1.5](../1.1_scale_latency/5_capacity_adaptation.md) fires for any tree carrying layer $l$, the peer sheds **layer $l$ and every layer above it** — every tree carrying those layers leaves $\mathcal{T}_{\text{sub}}$ together, since an SVC layer is undecodable without the layers beneath it and a stripe is undecodable without its siblings. The `priority` values in the slicing matrix encode this order (higher priority = shed later) and are equal for all trees of one layer. **Trees carrying $L_0$ are never shed.**

## 4.3 Solution 2: Multiple Description Coding (MDC)

If the broadcaster's encoder does not support SVC, the protocol utilizes **Multiple Description Coding (MDC)**.

MDC splits a single video feed into $D$ independent, equally important sub-streams (descriptions). A common implementation is Spatial Subsampling:
*   **Description 1:** Contains all the even-numbered scan lines of the video frame.
*   **Description 2:** Contains all the odd-numbered scan lines of the video frame.

### Slicing MDC to the Multi-Forest
The allocation rule of §4.2.1 applies unchanged with the descriptions in place of layers; since descriptions are independently decodable, their `priority` values are equal and the shed order among them is ascending `TreeID`, with Tree 1 never shed.

**The Benefit:** Both descriptions are independently decodable.
* If a peer receives both Tree 1 and Tree 2 data, the decoder interlaces them for a perfect, sharp 1080p image.
* If Tree 2 temporarily fails, the peer only receives Description 1 (the even lines). The decoder simply interpolates (blurs) the missing odd lines. The viewer perceives a momentary drop in visual sharpness, but the framerate remains a flawless 60fps with zero buffering.

## 4.4 Protocol Slicing Header

To allow clients to understand how the trees correspond to video layers, the initial connection manifest contains the Slicing Matrix mapping. The example below is the $M = 6$ rung of the reference ladder; the `tree_mapping` is exactly the output of the §4.2.1 rule.

```json
{
  "stream_id": "blake3_hash_of_pubkey",
  "manifest_version": 4,
  "slicing_mode": "SVC_SPATIAL",
  "num_trees": 6,
  "effective_segment_seq": 3852,
  "tree_mapping": [
    {"tree_id": 1, "layer": 0, "stripe": 0, "stripe_count": 2, "priority": 100, "bitrate_kbps":  750},
    {"tree_id": 2, "layer": 0, "stripe": 1, "stripe_count": 2, "priority": 100, "bitrate_kbps":  750},
    {"tree_id": 3, "layer": 1, "stripe": 0, "stripe_count": 2, "priority":  50, "bitrate_kbps":  750},
    {"tree_id": 4, "layer": 1, "stripe": 1, "stripe_count": 2, "priority":  50, "bitrate_kbps":  750},
    {"tree_id": 5, "layer": 2, "stripe": 0, "stripe_count": 2, "priority":  10, "bitrate_kbps": 1500},
    {"tree_id": 6, "layer": 2, "stripe": 1, "stripe_count": 2, "priority":  10, "bitrate_kbps": 1500}
  ]
}
```

The wire encoding is the 7-byte-per-tree matrix of Appendix D §D.4.8 (`{TreeID, Layer, StripeIndex, StripeCount, Priority, BitrateKbps}`); this JSON is illustrative. `bitrate_kbps` is the publisher's declared per-tree bitrate and is the value every capacity computation uses. Per-chunk block counts per layer, which vary with the encoder's actual output, travel in each `MANIFEST` (`LayerBlockCount`, §D.4.8), so a peer can compute exactly which global block indices belong to each tree.

A battery- or data-constrained mobile client declares itself `LEAF` class (see [5. Node Classes](5_node_classes.md)) and may subscribe to the base layer alone — the trees carrying $L_0$ — consuming less bandwidth while still receiving a stable 480p stream — a supported operating mode with defined entitlements, not an ad-hoc opt-out.

## 4.5 Dynamic Forest Resizing (`MANIFEST_UPDATE`)

The `num_trees` field is **dynamic**: it reflects the current forest size $M$ chosen by the source from the ladder defined in [1. Graph-Theoretic Foundations](1_graph_theory_and_slicing.md), §1.4. When the source changes $M$, the transition is coordinated as follows:

1.  **Announcement.** The source broadcasts a `MANIFEST_UPDATE` frame down all currently active trees and re-stores its DHT Stream Record. The frame carries the new `tree_mapping`, a strictly increasing `SlicingMatrixVersion`, an `EffectiveSegmentSeq` $= S_{\text{now}} + 5$ (the first segment emitted on the new layout — a **5-second migration window**), and the source's Ed25519 signature.
2.  **Relay reassignment is incremental.** Each `RELAY`-class peer recomputes its rendezvous assignment (§1.3) over $M_{\text{new}}$. Because rendezvous ranking is stable under a change of $M$, only $\approx 1/M_{\text{new}}$ of relays change tree on growth, and only the closed tree's relays change on shrink; every other relay keeps its tree, its parent, and its children. **A relay that does not move does nothing** — the content on its tree changes at `EffectiveSegmentSeq`, and its children keep receiving from it.
3.  **Moving relays drain, they do not drop.** A relay whose assignment changes announces the change to its old-tree children and keeps serving them for the drain window $\tau_{\text{drain}} = 5\text{ s}$ (§5.3) or until each has re-attached, releasing them through the standard sibling-election path (§3). It admits children in its new tree only as slots are released — its total children across old and new assignments never exceed $\sum_m K_v(m)$. Upload is therefore never double-committed.
4.  **Every peer acquires a parent in each new tree before `EffectiveSegmentSeq`**, using the standard parent-selection algorithm (§2.2). A tree that is genuinely new (growth) starts with the source's slots and the moving relays as they finish warm-up; the shed rule's warm-up exclusion (Ch1 §1.1.5) applies, so a slow-filling new tree is never mistaken for a capacity shortage during the window.
5.  **Switch.** At `EffectiveSegmentSeq` the source emits on the new layout only; peers close parent connections for trees that no longer exist. A peer without a parent in some new tree at the switch treats that tree as a failed join round and follows §1.1.5 (retry, then shed the layer) — it never stalls.

Peers ignore any `MANIFEST_UPDATE` whose signature does not verify against the pinned $PK_{\text{Source}}$ or whose `SlicingMatrixVersion` is not strictly greater than the last accepted version (replay protection). Per-tree bitrates generally change at a resize even for non-moving relays (Tree 3 carries $1.5$ Mbps at $M = 5$ and $0.75$ Mbps at $M = 6$), so every relay recomputes $K_v(m)$ against the new mapping; a slot count that *falls* is honoured by draining, never by dropping.
