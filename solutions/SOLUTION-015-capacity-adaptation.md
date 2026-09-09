# SOLUTION-015: Capacity Adaptation and Degraded Operation

**Closes:** ISSUE-015 (Critical)
**Lives in:** `protocol/chapter1/1.1_scale_latency/5_capacity_adaptation.md` (new), `1_bandwidth_paradox.md`, `protocol/chapter1/1.2_multi_forest_overlays/2_parent_selection_algorithm.md`, `4_stream_slicing_architecture.md`, `protocol/chapter1/1.3_peer_lifecycle/1_transition_model.md`, `appendix_b_parameters.md`, `appendix_d_frame_registry.md`
**Class:** The protocol's most likely real-world failure mode

---

## The problem in one line

Every latency and depth proof assumed $\sum u_i \ge N \cdot B$ — a mean of 6 Mbps upload per viewer — and no document said what happens when that fails, which given real residential upload distributions is the common case, not the exception.

## The decision

A degraded-operation ladder in which **quality degrades and playback never stops**:

* **Two thresholds, separated.** $\bar{u} \ge 8$ Mbps ($\sigma \ge 1.33$) makes the $D \le 7$ depth proof hold; $\bar{u} \ge 6$ Mbps merely makes full quality *deliverable* with deeper trees. The spec had been conflating them.
* **Sustainability is per-tree, not global.** Tree $m$ is sustainable iff $\bar{u}_{\text{relay}(m)} \ge B$. A healthy global $\sigma$ can hide a starving tree, which is why adaptation is driven by per-tree signals rather than a gossiped global estimate.
* **Saturation is detected locally** as `FAILURE_RETRY_BACKOFF` — no global measurement, no consensus.
* **Shed, never deepen.** After 2 consecutive failed rounds, drop the tree and its SVC layer. Parents must reject joins beyond $D_{\text{max}} = 8$; $D_{\text{max}}$ overflow and capacity saturation are the same condition with the same response.
* **The base layer is protected by the source**, which reserves $3 \cdot B_1$ as a Tree-1 emergency pool.

## The two ideas worth carrying forward

**Degradation is resolution loss, never freeze.** This is the whole design intent, and it is why SVC slicing was chosen over MDC or round-robin chunking in the first place — a fact that only becomes visible when the sustainability failure case is written down.

**The source's obligation stays a small constant.** A swarm with $\sigma \ll 1$ degrades to "480p, partially source-fed" rather than collapsing, and the broadcaster pays $3 \cdot B_1 = 4.5$ Mbps — never the $O(N)$ CDN cost the protocol exists to avoid. Bounding the worst case *without* reintroducing the thing being replaced is the load-bearing property.

## Defect found: shedding a tree stranded the peer in CONNECTING

The lifecycle state machine gated `CONNECTING → ACTIVE` on `Size(Parents) == M` — parents for **all $M$ slices**. Shedding a tree means deliberately having fewer than $M$ parents. So:

> A peer that sheds an enhancement tree never satisfies the ACTIVE condition, sits in `CONNECTING` indefinitely, and renders no video at all.

The mechanism designed to keep playback alive under capacity shortage instead guaranteed playback never started — and by this document's own account, capacity shortage is the common case. `CHURN_REPAIR → ACTIVE` had the identical flaw via `AllSlicesRestored()`.

**Resolution:** both transitions are gated on the **subscribed** tree set $\mathcal{T}_{\text{sub}} = \{1 \ldots M\} \setminus \mathcal{T}_{\text{shed}}$, with $1 \in \mathcal{T}_{\text{sub}}$ always. Shedding removes a tree, re-join after hysteresis restores it, a `MANIFEST_UPDATE` recomputes the set.

Keeping the base layer permanently in $\mathcal{T}_{\text{sub}}$ has a useful consequence: a peer with no Tree-1 parent **cannot** enter ACTIVE, so "the base layer never fails" becomes a property of the state machine rather than only of the scheduler, and the source reserve is the mechanism that resolves it.

## Second defect: the source reserve had no trigger on the wire

§5.4 said the source serves base-layer slots "when it observes starvation signals: repeated `GET_PEERS` re-queries carrying a starved-tree indication." No such indication existed in any frame — `GET_PEERS` had no request layout at all, only a response. A peer starving in Tree 1 was indistinguishable from an ordinary joiner, so the emergency pool could never be aimed and the protocol's floor guarantee was unimplementable.

**Resolution:** a one-byte `StarvedTreeBitmap` in the `GET_PEERS` request (Appendix D §D.4.6b), bit $m-1$ meaning "I cannot get a parent in $T_m$". Guardians aggregate it per tree and report to the publisher alongside the swarm-size count they already collect. Costs one byte, and `0x00` — the ordinary case — carries no information.

## The generalisable lesson

**A fallback path must be traced through every state machine and every frame it touches, not just the subsystem that owns it.** Capacity adaptation was written as a Chapter 1.1 concern and was internally complete; it silently contradicted the Chapter 1.3 lifecycle and depended on a Chapter 2 frame field that did not exist. Both failures are invisible from inside the document that introduced them.

The concrete check worth repeating: for any new degraded mode, ask (a) which state-machine transitions assume the non-degraded invariant, and (b) which of its triggers correspond to actual bytes on the wire.

## Validation owed (Chapter 8)

* Shed/re-join oscillation across a peer cohort — do the 2-round threshold and 10 s hysteresis actually prevent lockstep quality flapping?
* Source egress under $\sigma \ll 1$ at large $N$, confirming the reserve stays near $3 \cdot B_1$ and does not creep toward $O(N)$.
* Playback continuity through a shed event: is the layer drop genuinely seamless, or is there a visible artefact at the transition?
* Whether $\sigma_{\text{target}} = 1.33$ matches real residential upload distributions, or whether the reference ladder bitrates should be lowered.
