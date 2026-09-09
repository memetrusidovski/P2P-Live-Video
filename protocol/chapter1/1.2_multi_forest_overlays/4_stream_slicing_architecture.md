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

**Minimax is not base-layer-first, and the ladder is not monotone for the base layer.** A layer's requirement on relay upload is $\propto M \cdot b_l / t_l$ (Ch1 §1.1.5 §5.1), so the base layer is easiest when it holds the most trees; minimax gives it extra trees only on ties. On the reference ladder the base layer's requirement at $\Omega = 1.15$, $\ell = 0$ runs $7.67, 5.75, 7.67, 4.79, 5.75$ Mbps for $M = 2 \ldots 6$: the $3 \to 4$ step, where $(1,1,2)$ puts every layer at $1.5$ Mbps and $L_0$ on one tree in four, *raises* it. A base-first alternative — minimise the largest per-tree bitrate subject to $b_0/t_0$ being strictly the smallest where $M$ allows — gives $(2,1,1)$ at $M = 4$: $L_0$ at $3.83$, $L_1$ at $7.67$, $L_2$ at $15.3$.

| $M = 4$ | 1080p needs | $L_1$ needs | $L_0$ needs | after folding $L_2$ |
| :--- | :---: | :---: | :---: | :---: |
| minimax $(1,1,2)$ | $7.67$ | $7.67$ | $7.67$ | $(2,2)$: all $3.83$ |
| base-first $(2,1,1)$ | $15.3$ | $7.67$ | $3.83$ | $(2,2)$: all $3.83$ |

Minimax is kept. With the fold rule of Ch1 §1.1.5 §5.6 in place the two objectives reach the **same steady state** wherever they differ: for $3.83 \le \bar{u}_{\text{relay}} < 7.67$ Mbps both end at $(2,2)$ after one fold (minimax because $L_0$ starves, base-first because $L_1$ does); for $7.67 \le \bar{u} < 15.3$ minimax delivers 1080p to everyone and base-first only to those who win $L_2$ slots. Minimax is never worse in steady state and strictly better across the band that matters most. What it costs is the **transient**: in the lower band the layer that starves until the fold fires is the *base* layer — a freeze for the starved fraction, not a resolution loss — which is why the fold rule detects base-layer starvation on a shorter window ($\tau_{\text{fold}} = 10$ s for $L_0$, $30$ s otherwise) and why unfolding is gated on relay growth rather than a timer, so that a swarm parked at this rung is folded once and left there.

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
*   **The mapping, not the peer, decides which layers the relay population can carry.** A relay's slots live in its assigned tree and serve only the layer that tree carries, so a shed at the peers frees nothing for the layers below (Ch1 §1.1.5 §5.2). When a lower layer is starved the source **folds** the top layer: it re-runs the rule above over $L_0 \ldots L_{\text{top}-1}$ at the same $M$ and publishes the result as a `MANIFEST_UPDATE` (§4.5; the trigger is Ch1 §1.1.5 §5.6). Folding never raises any remaining tree's bitrate — a layer removed from the greedy allocation only hands its trees to the layers that remain — so no relay's slot count falls at a fold. The folded mappings for the reference ladder are: $M{=}2$: $(1,1)$ then $(2)$; $M{=}3$: $(2,1)$ then $(3)$; $M{=}4$: $(2,2)$ then $(4)$; $M{=}5$: $(3,2)$ then $(5)$; $M{=}6$: $(3,3)$ then $(6)$.

### 4.2.2 Graceful Degradation and the Shed Order

**The Benefit:** If an $L_2$ tree collapses due to massive churn, the peer loses Layer 2. Because the $L_0$ and $L_1$ trees are separate routing paths, the peer continues to receive them flawlessly. The viewer's video player smoothly drops from 1080p to 720p without a single frozen frame or buffering wheel. Once the tree heals (250 ms later), the video snaps back to 1080p.

**Normative shedding order — by layer, not by tree index.** A peer subscribes to a *layer prefix* $\mathcal{L}_{\text{sub}} = \{L_0 \ldots L_j\}$ and therefore to the tree set $\mathcal{T}_{\text{sub}}$ carrying those layers. When the shed rule of [Chapter 1 §1.1.5](../1.1_scale_latency/5_capacity_adaptation.md) fires for any tree carrying layer $l$, the peer sheds **layer $l$ and every layer above it** — every tree carrying those layers leaves $\mathcal{T}_{\text{sub}}$ together, since an SVC layer is undecodable without the layers beneath it and a stripe is undecodable without its siblings. The `priority` values in the slicing matrix encode this order (higher priority = shed later) and are equal for all trees of one layer. **Trees carrying $L_0$ are never shed.**

**What a shed means at the decoder** depends on the `LayerMode` in the `STREAM_DESCRIPTOR` (Ch4 §4.1.1, Appendix D §D.4.20), which is why that record exists; "drop $L_2$" is three different operations:

| `LayerMode` | Layers are | Shedding $L_l$ means | Alignment needed |
| :--- | :--- | :--- | :--- |
| `SVC_SPATIAL` / `SVC_TEMPORAL` | one bitstream's layers, additive | stop feeding layer $l$'s byte stream; the decoder renders the prefix | none — SVC layers are droppable at any frame |
| `SIMULCAST` | independent renditions | switch the decoder to the highest rendition still received | at a segment boundary — every segment is a closed GOP (§4.1.1), so the switch waits at most one segment |
| `INDEPENDENT` | unrelated streams (e.g. video, audio) | the stream stops; nothing else is affected | none |

The forest treats every mode identically — trees, stripes, priorities and the fold rule do not change — and the descriptor's initialisation data is per layer so that a `SIMULCAST` switch has the target rendition's `moov` already in hand. An earlier draft assumed additive SVC everywhere and named no mode, leaving the decoder side of a shed undefined.

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

1.  **Announcement.** The source broadcasts a `MANIFEST_UPDATE` frame down all currently active trees and re-stores its DHT Stream Record — which now carries `EffectiveSegmentSeq` and the pending matrix alongside the current one (Ch2 §2.3.3), so a peer joining inside the window learns both. The frame carries the new `tree_mapping`, a strictly increasing `SlicingMatrixVersion`, an `EffectiveSegmentSeq` $= S_{\text{now}} + 5$ (the first segment emitted on the new layout — a **5-second migration window**), and the source's Ed25519 signature.
2.  **Relay reassignment is incremental.** Each `RELAY`-class peer recomputes its rendezvous assignment (§1.3) over $M_{\text{new}}$. Because rendezvous ranking is stable under a change of $M$, only $\approx 1/M_{\text{new}}$ of relays change tree on growth, and only the closed tree's relays change on shrink; every other relay keeps its tree, its parent, and its children. **A relay that does not move does nothing** — the content on its tree changes at `EffectiveSegmentSeq`, and its children keep receiving from it.
3.  **Moving relays drain, they do not drop.** A relay whose assignment changes sends `DRAIN_NOTICE` (reason `REASSIGNED`, scope *all children*, deadline `EffectiveSegmentSeq`; §2.2 *The Drain Path*) on its old tree and keeps serving those children until each has re-attached or the switch, whichever is first. Its children re-select while it is still delivering, using the sibling election of §3 over the roster they already hold. It admits children in its new tree only as slots are released — its total children across old and new assignments never exceed $\sum_m K_v(m)$. Upload is therefore never double-committed.
4.  **The source pre-emits every new tree during the window.** For each tree that exists only in the new layout, the source emits that tree's *new-layout* stripe from the announcement onward, in parallel with the old layout. The blocks it carries are always also carried by the old layout (a new tree's stripe is a subset of blocks the old trees already deliver), so to any receiver that already holds them they are duplicates, dropped by the ordinary `BlockIndex` check — `BlockIndex` does not depend on the matrix (Appendix D §D.4.8). What pre-emission buys is **warm-up before the switch**: without it no relay could verify a segment in the new tree before `EffectiveSegmentSeq`, every relay in it would advertise `WARMING` for the whole window, the only joinable parent would be the source, and the fill-in — one warm-up window per level — would begin only *after* the switch. The source's pre-emit slots in a new tree are bounded by its headroom: with $K_S(m)$ its ordinary slot count (Ch1 §1.1.5 §5.4), it admits $\lfloor (u_S - R_{\text{src}} - E_{\text{old}}) / (B_m \Omega) \rfloor$ children in each new tree during the window, where $E_{\text{old}}$ is its committed egress on the old layout, and the remainder at the switch. Moving relays join the new tree as its first generation, warm up on the pre-emitted stripe, and accept children as their old-tree children leave. Every peer that lacks a parent in a new tree runs the standard parent-selection algorithm (§2.2) for it from the announcement onward; the warm-up exclusion (Ch1 §1.1.5 §5.3 item 2) applies during the window.
5.  **Switch.** At `EffectiveSegmentSeq` the source emits on the new layout only; peers close parent connections for trees that no longer exist. A peer without a parent in some tree whose entry changed renders without that layer, repairs what it can from the PULL zone, and keeps joining; under the **grace rule** (Ch1 §1.1.5 §5.3 item 6) its failed rounds against such a tree do not count toward the shed threshold until `EffectiveSegmentSeq` $+ D_{\max}$ segments. It never stalls, and it never sheds a layer for a tree that is still filling in.

An earlier version of step 4 read "every peer acquires a parent in each new tree before `EffectiveSegmentSeq`". That was unsatisfiable: nothing was emitted on a new tree before the switch, so nothing could warm up in it, and the shed rule then fired on the post-switch fill-in — at the reference $5 \to 6$ rung, every peer lost $L_2$ for about two seconds and then shed it for ten more, at each of the four rungs a growing stream climbs. Pre-emission makes the fill-in happen inside the window; the grace rule makes whatever remains of it harmless.

**Content changes are not confined to new trees.** At $5 \to 6$ on the reference ladder, trees 3, 4 and 5 change from $(L_1, L_2, L_2)$ stripes to $(L_1, L_1, L_2)$, and their subscriber sets change with them (tree 4 goes from 1080p viewers to everyone at 720p and above). Their relays do not move and are already warm; their slot counts change at the switch, when $B_m$ does. Peers that newly need such a tree join it from the announcement onward as slots allow and from the switch as the new $K_v(m)$ opens slots; the grace rule covers them too. The same choreography, with no new trees and no movers, is what a **layer fold** (Ch1 §1.1.5 §5.6) runs.

**Flash-crowd jumps.** A pre-empted ladder step ($2 \to 6$ in one transition, §1.4) creates four new trees at once; the source pre-emits all four, $\approx 4.5$ Mbps $\cdot \Omega$ per child across them, so its headroom — not the window — bounds the first generation, and the grace rule carries the rest. The source's egress during the window is the old layout plus whatever new-tree children fit; it is never asked to exceed $u_S$.

Peers ignore any `MANIFEST_UPDATE` whose signature does not verify against the pinned $PK_{\text{Source}}$ or whose `SlicingMatrixVersion` is not strictly greater than the last accepted version (replay protection). Per-tree bitrates generally change at a resize even for non-moving relays (Tree 3 carries $1.5$ Mbps at $M = 5$ and $0.75$ Mbps at $M = 6$), so every relay recomputes $K_v(m)$ against the new mapping at the switch; a slot count that *falls* is honoured by draining (`DRAIN_NOTICE`, reason `CAPACITY`), never by dropping.
