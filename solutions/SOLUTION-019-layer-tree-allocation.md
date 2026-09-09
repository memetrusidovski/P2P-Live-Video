# SOLUTION-019: Layer→Tree Allocation and Per-Tree Bitrate

**Closes:** ISSUE-019 (High)
**Lives in:** `protocol/chapter1/1.2_multi_forest_overlays/4_stream_slicing_architecture.md` §4.2.1–4.2.2, §4.4; `1_graph_theory_and_slicing.md` §1.2–1.3; `protocol/chapter1/1.1_scale_latency/5_capacity_adaptation.md` §5.1–5.3; `protocol/chapter1/1.3_peer_lifecycle/1_transition_model.md`; `appendix_d_frame_registry.md` §D.4.8; `appendix_b_parameters.md`
**Class:** Two ladders introduced by separate fixes, never reconciled (recurring pattern #2)

---

## The problem in one line

The forest ladder (SOLUTION-002) scales $M$ from 2 to 6 and the SVC ladder (SOLUTION-015) has 3 layers of unequal bitrate; the only `tree_mapping` the spec defined was the $M = 3$ case where they happen to coincide, so at every other rung — most real streams — no two implementations would build the same forest, the shed order was undefined, and every slot count was computed against a $B/M$ that no tree actually carried.

## The decision

1.  **The source constructs `tree_mapping` by a deterministic rule; peers only read it.** For $M \ge L$ each layer starts with one tree and the remaining $M - L$ trees are handed out one at a time to the layer with the *largest current per-tree bitrate*, ties to the lower layer (greedy minimax). Layers spanning several trees are **striped** by block index within the layer. For $M < L$ the ordered layers are cut into $M$ contiguous **bundles** minimising the largest bundle bitrate.
2.  **Per-tree bitrate is explicit on the wire.** The slicing-matrix entry is `{TreeID, Layer, StripeIndex, StripeCount, Priority, BitrateKbps}` (7 bytes). `BitrateKbps` is the value every capacity computation uses; $B/M$ is demoted to a nominal average for coarse sizing.
3.  **Subscription and shedding are by layer.** A peer holds a layer prefix $\mathcal{L}_{\text{sub}}$; $\mathcal{T}_{\text{sub}}$ is derived from it through the current matrix. Two failed rounds in *any* tree of layer $l$ shed $l$ and everything above it. The $L_0$ trees are never shed.
4.  **The per-segment `MANIFEST` carries per-layer block counts.** Without `LayerBlockCount` a peer cannot map a global block index to a tree, so it cannot know which tree a *missing* block belongs to. This gap existed at $M = 3$ too; it only became visible once stripes made the mapping non-trivial.

## Why this and not the alternatives

*   **Largest-remainder proportional allocation** (the obvious "trees ∝ bitrate" rule) was drafted first and rejected on arithmetic. On the reference ladder at $M = 6$ it gives $(2, 1, 3)$: $L_0$ stripes at $0.75$ Mbps, $L_1$ whole at $1.5$, $L_2$ stripes at $1.0$. Per-tree sustainability (§5.1) then requires $11.5$ Mbps mean relay upload for the $L_1$ tree but only $7.7$ for the $L_2$ trees — so **$L_1$ starves before $L_2$**, and because layers are dependent, a shortage that should have cost one layer costs two. The minimax rule gives $(2, 2, 2)$ — $0.75, 0.75, 0.75, 0.75, 1.5, 1.5$ — where the top layer's trees are the hardest to relay, which is the order the shed rule drops them in. Capacity-driven failure and the shed order now coincide by construction rather than by luck.
*   **Re-cutting the encoder ladder to divide evenly into 1 Mbps slices** was rejected: it makes the encoder serve the overlay, and it breaks the moment a publisher chooses different bitrates.
*   **Letting peers derive the mapping** from $(M, \text{ladder})$ locally would save 7 bytes per tree per resize. Rejected because the mapping is exactly the kind of derived value that drifts (principle 12), and because SOLUTION-002's invariant — the signed matrix is the only authority on what a tree contains — is what makes a resize safe.
*   **Bundling $\{L_0\}$, $\{L_1 + L_2\}$ at $M = 2$** (base layer alone in Tree 1) was rejected in favour of $\{L_0 + L_1\}$, $\{L_2\}$: the latter equalises per-tree bitrate ($3.0/3.0$ vs $1.5/4.5$) and shedding Tree 2 leaves 720p rather than 480p. The cost is that a base-layer-only subscription does not exist at $M = 2$; a leaf receives 720p for 3 Mbps.

## Defects found during verification

*   **$B_m = B/M$ was load-bearing in four places** it should not have been: the slot count, the per-tree sustainability condition, `PROBE_RESPONSE.K_avail`, and the source reserve. All are now stated against the tree's declared bitrate. At $M = 3$ the $L_2$ tree carries $3.0$ Mbps against a nominal $2.0$ — a relay sized on $B/M$ over-committed that tree by $50\%$.
*   **Shed state did not survive a resize.** The old rule removed tree *indices* from $\mathcal{T}_{\text{sub}}$; a resize renumbers which indices carry which layer, so a shed $L_2$ could silently become a shed $L_1$ stripe. Carrying shed state by layer fixes this.
*   **Striping multiplies a layer's failure surface.** Losing one of two $L_2$ stripes at $M = 6$ removes half the layer's blocks, which is not a decodable layer. This is stated explicitly as the price of finer capacity granularity; recovery is per tree and unchanged.

## The generalisable lesson

**When two independently-scaled ladders meet, write down the map between them for every rung, then check that the failure order the map induces matches the failure order the rest of the design assumes.** The $M = 3$ example looked complete because it was the one rung where nothing had to be decided. The first plausible rule for the other rungs was wrong in a way only the per-tree arithmetic exposed.

## Residual risk

*   Per-tree bitrates within one forest differ by $2\times$; relays on the $1.5$ Mbps trees hold half the children. The forest is no longer uniform, and Chapter 8 must confirm that the depth proof (which assumes fan-out 8) holds per tree rather than on average.
*   At $M = 2$ there is no base-layer-only mode. A $3$ Mbps floor is heavier than the $1.5$ Mbps the leaf QoS story assumed.

## Validation owed (Chapter 8)

*   Per-tree depth distribution at $M = 5, 6$ on real upload distributions — do the $1.5$ Mbps $L_2$ stripes deepen past $D_{\max}$ before the $0.75$ Mbps trees are anywhere near saturation?
*   Shed behaviour under stripe loss: how often does a single $L_2$ tree failure cause a shed that PULL repair would have covered within the hysteresis window?
*   Whether the minimax tie rule (spare trees to lower layers) or a variant weighted toward $L_0$ redundancy gives better base-layer availability.
