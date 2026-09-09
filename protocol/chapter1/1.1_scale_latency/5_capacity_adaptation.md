# 5. Capacity Adaptation and Degraded Operation

The Swarm Sustainability Condition (§1) states when the swarm can carry itself:
$$\sum_{i=1}^{N} (1 - r_{\text{pull}})\, u_i \ \ge\ \Omega \cdot N \cdot B$$

where $\Omega \approx 1.15$ is the per-slice overhead factor (FEC parity plus framing) and $r_{\text{pull}} = 0.10$ the PULL-service reserve, both defined with the slot count in §1.2.1. That inequality is an *assumption* in the depth and latency proofs of §2–§3, not a guarantee the network provides. This document defines what the protocol does when the assumption fails — which, given real residential upload distributions, is the common case rather than the exception.

## 5.1 The Two Thresholds

Let $\bar{u} = \frac{1}{N}\sum u_i$ be the mean upload capacity of the swarm and define the **capacity ratio**:
$$\sigma = \frac{\bar{u}}{B}$$

Two distinct thresholds matter, and the spec has historically conflated them. Both include the overhead that every pushed slice actually costs:

| Condition | Meaning |
| :--- | :--- |
| $\bar{u} \ge \sigma_{\text{target}} B \approx 10.2\text{ Mbps}$ ($\sigma \ge 1.70$) | Mean fan-out $k = 8$ at the nominal $1$ Mbps slice is achievable ($8 \cdot 1\text{ Mbps} \cdot \Omega / (1 - r_{\text{pull}})$), so the $D \le 7$ depth proof of §3 holds **and** full quality is sustainable. This is the design target. |
| $\sigma_{\text{full}} B \approx 7.7 \le \bar{u} < 10.2\text{ Mbps}$ | Full quality is sustainable ($\sigma_{\text{full}} = \Omega / (1 - r_{\text{pull}}) = 1.28$), but mean fan-out drops below 8 and trees deepen toward $D_{\text{max}} = 8$. Still inside the playout budget, with less headroom. |
| $\bar{u} < 7.7\text{ Mbps}$ ($\sigma < 1.28$) | The swarm **cannot** carry full bitrate. Layer shedding (§5.3) applies. |

An overhead-blind reading of the same table ($6$ and $8$ Mbps) was $15$–$25\%$ optimistic; the earlier draft carried that error.

Sustainability is also a **per-tree** property, not only a global one. Tree $T_m$ carries its declared bitrate $B_m$ (§1.2.4) to every subscriber and draws its relays from the $\approx N_{\text{relay}}/M$ nodes assigned to it, each holding $K_v(m)$ slots. Tree $m$ is sustainable iff its relays' slots cover the swarm:
$$\frac{N_{\text{relay}}}{M} \cdot \frac{(1 - r_{\text{pull}})\, \bar{u}_{\text{relay}(m)}}{B_m\, \Omega} \ \ge\ N
\quad\Longleftrightarrow\quad
\bar{u}_{\text{relay}(m)} \ \ge\ \frac{1}{1 - \ell} \cdot \frac{M \cdot B_m \cdot \Omega}{1 - r_{\text{pull}}}$$

where $\ell = 1 - N_{\text{relay}}/N$ is the leaf fraction (§1.2.5). With no leaves and the nominal $B_m = B/M$ this collapses to the global condition; with real per-tree bitrates it does not. On the $M = 6$ reference mapping the $0.75$ Mbps $L_0$/$L_1$ stripes need $\bar{u}_{\text{relay}} \ge 5.75$ Mbps and the $1.5$ Mbps $L_2$ stripes need $11.5$ Mbps — the top layer's trees are the first to starve, which is the order the shed rule drops them in. That coincidence is deliberate: it is what the minimax allocation rule of §1.2.4 exists to produce. A single global $\sigma$ can look healthy while one tree starves, which is why adaptation is driven by per-tree signals, not by a gossiped global estimate.

## 5.2 Sustainable Layer Set

With SVC slicing (§1.2.4), the shed decision is which enhancement layers to keep. A swarm whose relays have mean usable upload $\bar{u}$ can sustain the layer prefix $L_0 \ldots L_j$ satisfying:
$$\sum_{l=0}^{j} b_l \ \le\ \frac{(1 - r_{\text{pull}})\, \bar{u}}{\Omega}$$

For the reference 3-layer ladder (480p base $= 1.5$ Mbps, 720p enhancement $= 1.5$ Mbps, 1080p enhancement $= 3.0$ Mbps; $B = 6$ Mbps) at the clean-link overhead $\Omega = 1.15$:

| Mean upload $\bar{u}$ | Sustainable layers | Delivered quality |
| :---: | :--- | :--- |
| $\ge 7.7$ Mbps | $L_0 + L_1 + L_2$ | 1080p |
| $3.8$–$7.7$ Mbps | $L_0 + L_1$ | 720p |
| $1.9$–$3.8$ Mbps | $L_0$ | 480p |
| $< 1.9$ Mbps | $L_0$ with source top-up (§5.4) | 480p, source-subsidized |

On a lossy swarm at the $30\%$ parity ceiling ($\Omega = 1.42$) every threshold rises by a further $23\%$. Degradation is therefore *resolution loss*, never playback freeze — the property SVC slicing exists to provide.

## 5.3 Saturation Detection and Layer Shedding

No global measurement is required. Capacity shortage manifests locally, as failure to acquire a parent in a specific tree:

1.  **Signals.** The tree-join algorithm (§1.2.2) returns `FAILURE_RETRY_BACKOFF` when every candidate rejects the join, or when every candidate advertises hop depth $h \ge D_{\text{max}}$. Parents **must** reject any join that would place a child deeper than $D_{\text{max}} = 8$; a saturated forest may not silently grow deeper.
2.  **Warm-up rounds do not count.** A candidate advertising $K_{\text{avail}} = 0$ **and** `LiveEdgeSegmentSeq = 0` is *warming up*, not saturated (§1.2.2): it has been accepted into the tree but has not yet verified its first segment, and will have capacity within one segment period. A retry round in which at least one candidate was warming is **not** a failed round for the purposes of the shed rule below. Without this exclusion a growth burst — many relays warming at once, which is exactly what a flash crowd or a forest resize produces — is indistinguishable from a capacity shortage, and whole cohorts of peers would shed a quality layer for a condition that resolves in under a second. For the same reason the retry backoff interval **must** be at least one segment period ($1.0\text{ s}$), so that no shed decision is ever taken on a sample shorter than a single warm-up window.
3.  **Shed rule — by layer.** After **2 consecutive failed retry rounds** in any tree $T_m$ carrying layer $l$, the peer sheds **layer $l$ and every layer above it**: it stops attempting to join every tree that carries those layers, removes them from its subscribed set $\mathcal{T}_{\text{sub}}$ (§1.3), marks the layers unavailable, and instructs its decoder to render at layer $l - 1$. Shedding is by layer because an SVC layer is undecodable without the layers beneath it, and a striped layer is undecodable without all of its stripes (§1.2.4). The manifest `priority` values encode the order — all trees of one layer share a priority, and lower priority is shed first. **The trees carrying the base layer $L_0$ are never shed.**
4.  **Hysteresis.** A shed layer is not retried for $10$ seconds, after which the existing 5-second upward-migration loop (§1.2.2) attempts re-join of its trees naturally; the layer is restored only when *all* of its trees have parents again. This prevents whole cohorts of peers from oscillating between quality levels in lockstep.

$D_{\text{max}}$ overflow and capacity saturation are thus the *same condition* with the same response: shed a layer rather than deepen the tree past its latency budget.

## 5.4 Source-Side Base-Layer Reserve

The base layer is the protocol's floor and is protected by the broadcaster directly. The source reserves at least $3 \cdot b_0 \cdot \Omega$ of its own upload capacity as a **base-layer emergency pool**, held back from ordinary child allocation in the $L_0$ trees. It serves base-layer slots directly when it observes starvation signals: `GET_PEERS` re-queries whose `StarvedTreeBitmap` (Appendix D §D.4.6b) marks an $L_0$ tree, aggregated per tree by the DHT guardians and returned to the publisher on every Stream Record store (Ch2 §2.3.2). The reserve therefore has a concrete, counted trigger rather than an inferred one.

This bounds the worst case. A swarm with $\sigma \ll 1$ degrades to "480p, partially source-fed" rather than collapsing — and the source's obligation stays a small constant ($3 \cdot 1.5 \cdot 1.15 \approx 5.2$ Mbps at the reference ladder), never the $O(N)$ CDN cost the protocol exists to avoid.

## 5.5 Interaction with Incentives

Under saturation, the two layer classes are allocated differently:

*   **Enhancement-layer child slots** are allocated by contribution rank (Chapter 5): a saturated enhancement-tree parent may preempt its lowest-ranked child for a better-ranked joiner. Scarcity makes contribution the deciding factor for quality, exactly as the protocol's core axiom requires.
*   **Base-layer slots** form a **universal service floor**: every verified peer is entitled to $L_0$ regardless of rank. Base-layer slots are never rank-preempted, a bounded share of them is reserved for leaf-class peers, and at the margin the source reserve of §5.4 serves them.

This is what keeps leaf-class devices (§1.2.5) from being free-riders: they receive the floor by right and pay for it in quality ceiling, not in stalled playback.
