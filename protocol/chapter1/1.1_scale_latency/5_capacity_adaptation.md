# 5. Capacity Adaptation and Degraded Operation

The Swarm Sustainability Condition (§1) states when the swarm can carry itself:
$$\sum_{i=1}^{N} u_i \ge N \cdot B$$

That inequality is an *assumption* in the depth and latency proofs of §2–§3, not a guarantee the network provides. This document defines what the protocol does when the assumption fails — which, given real residential upload distributions, is the common case rather than the exception.

## 5.1 The Two Thresholds

Let $\bar{u} = \frac{1}{N}\sum u_i$ be the mean upload capacity of the swarm and define the **capacity ratio**:
$$\sigma = \frac{\sum_{i=1}^{N} u_i}{N \cdot B} = \frac{\bar{u}}{B}$$

Two distinct thresholds matter, and the spec has historically conflated them:

| Condition | Meaning |
| :--- | :--- |
| $\bar{u} \ge 8\text{ Mbps}$ ($\sigma \ge 1.33$) | Mean fan-out $k = 8$ is achievable, so the $D \le 7$ depth proof of §3 holds **and** full quality is sustainable. This is the design target ($\sigma_{\text{target}}$). |
| $6 \le \bar{u} < 8\text{ Mbps}$ ($1 \le \sigma < 1.33$) | Full quality is sustainable, but mean fan-out drops below 8 and trees deepen toward $D_{\text{max}} = 8$. Still inside the playout budget, with less headroom. |
| $\bar{u} < 6\text{ Mbps}$ ($\sigma < 1$) | The swarm **cannot** carry full bitrate. Layer shedding (§5.3) applies. |

Sustainability is also a **per-tree** property, not only a global one. Under the placement rules of §1.2, tree $T_m$ carries $B_m = B/M$ to every peer and draws its relays from the nodes assigned to it. Tree $m$ is sustainable iff the mean upload of its relay population satisfies:
$$\bar{u}_{\text{relay}(m)} \ge M \cdot B_m = B$$

A single global $\sigma$ can therefore look healthy while one tree starves — which is why adaptation is driven by per-tree signals, not by a gossiped global estimate.

## 5.2 Sustainable Layer Set

With SVC slicing (§1.2.4), the shed decision is which enhancement layers to keep. A peer with usable upload $\bar{u}$ can sustain the layer prefix $L_0 \ldots L_j$ satisfying:
$$\sum_{l=0}^{j} B_{L_l} \le \frac{\bar{u}}{1 + \text{Overhead}_{\text{fec}}}$$

For the reference 3-layer ladder (480p base $= 1.5$ Mbps, 720p enhancement $= 1.5$ Mbps, 1080p enhancement $= 3.0$ Mbps; $B = 6$ Mbps) at a 10% FEC allowance:

| Mean upload $\bar{u}$ | Sustainable layers | Delivered quality |
| :---: | :--- | :--- |
| $\ge 6.6$ Mbps | $L_0 + L_1 + L_2$ | 1080p |
| $3.3$–$6.6$ Mbps | $L_0 + L_1$ | 720p |
| $1.65$–$3.3$ Mbps | $L_0$ | 480p |
| $< 1.65$ Mbps | $L_0$ with source top-up (§5.4) | 480p, source-subsidized |

Degradation is therefore *resolution loss*, never playback freeze — the property SVC slicing exists to provide.

## 5.3 Saturation Detection and Layer Shedding

No global measurement is required. Capacity shortage manifests locally, as failure to acquire a parent in a specific tree:

1.  **Signals.** The tree-join algorithm (§1.2.2) returns `FAILURE_RETRY_BACKOFF` when every candidate rejects the join, or when every candidate advertises hop depth $h \ge D_{\text{max}}$. Parents **must** reject any join that would place a child deeper than $D_{\text{max}} = 8$; a saturated forest may not silently grow deeper.
2.  **Shed rule.** After **2 consecutive failed retry rounds** in tree $T_m$, the peer sheds that tree: it stops attempting to join, marks the corresponding SVC layer unavailable, and instructs its decoder to render at the next lower layer. Trees are shed in **ascending manifest `priority` order** — the 1080p enhancement (`priority: 10`) goes first, then 720p (`priority: 50`). Tree 1 (the base layer, `priority: 100`) is **never** shed.
3.  **Hysteresis.** A shed tree is not retried for $10$ seconds, after which the existing 5-second upward-migration loop (§1.2.2) attempts re-join naturally. This prevents whole cohorts of peers from oscillating between quality levels in lockstep.

$D_{\text{max}}$ overflow and capacity saturation are thus the *same condition* with the same response: shed a layer rather than deepen the tree past its latency budget.

## 5.4 Source-Side Base-Layer Reserve

The base layer is the protocol's floor and is protected by the broadcaster directly. The source reserves at least $3 \cdot B_1$ of its own upload capacity as a **Tree-1 emergency pool**, held back from ordinary Layer-1 child allocation. It serves base-layer slots directly when it observes starvation signals: repeated `GET_PEERS` re-queries carrying a starved-tree indication, or a sustained rise in rarity gossip for base-layer blocks.

This bounds the worst case. A swarm with $\sigma \ll 1$ degrades to "480p, partially source-fed" rather than collapsing — and the source's obligation stays a small constant ($3 \cdot B_1 = 4.5$ Mbps at the reference ladder), never the $O(N)$ CDN cost the protocol exists to avoid.

## 5.5 Interaction with Incentives

Under saturation, the two layer classes are allocated differently:

*   **Enhancement-layer child slots** are allocated strictly by Tit-for-Tat / PoU rank (Chapter 5). Scarcity makes contribution the deciding factor for quality, exactly as the protocol's core axiom requires.
*   **Base-layer slots** form a **universal service floor**: every verified peer is entitled to $L_0$ regardless of rank, served partly through the optimistic-unchoke quota and, at the margin, the source reserve of §5.4.

This is what keeps leaf-class devices (§1.2.5) from being free-riders: they receive the floor by right and pay for it in quality ceiling and live-edge offset, not in stalled playback.
