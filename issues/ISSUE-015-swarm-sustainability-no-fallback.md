# ISSUE-015: Swarm Sustainability Condition Has No Fallback (Σu < N·B Undefined)

**Status:** Resolved  
**Priority:** Critical  
**Component:** Ch1 §1.1 (Scale/Latency) / Ch1 §1.2.4 (SVC Slicing)  
**Affects:** Any swarm whose mean upload < stream bitrate — i.e., most realistic residential audiences  
**File:** `protocol/chapter1/1.1_scale_latency/`, `protocol/chapter1/1.2_multi_forest_overlays/4_stream_slicing_architecture.md`

---

## Summary

The Swarm Sustainability Condition (Σuᵢ ≥ N·B) requires a mean of 6 Mbps upload per viewer, and the D ≤ 7 depth proof assumes fan-out k = 8 (8 Mbps upload in the assigned tree). Ch1.1.1 itself acknowledges asymmetric residential upload, but no document defines what happens when the condition fails, nor what a peer does when it cannot find a parent within D_max = 8 hops. The SVC layer priorities in ch1.2 §4.2 are the obvious degradation lever but are never connected to the sustainability math. This is the protocol's single most likely real-world failure mode.

## Proposed Fix

New doc **`protocol/chapter1/1.1_scale_latency/5_capacity_adaptation.md`**:

1. **The math:** capacity ratio σ = Σuᵢ/(N·B); per-tree sustainability (tree m sustainable iff mean relay upload in tree m ≥ M·B_m = B); operating thresholds ū ≥ 8 Mbps (full quality, depth proof holds), 6–8 Mbps (full quality, deeper trees), ū < 6 (σ < 1 ⇒ layer shedding). Sustainable-layer rule: retain SVC layers L₀..L_j with Σ bitrates ≤ ū/(1+FEC), with a worked table.
2. **Saturation signal:** local join failure (`FAILURE_RETRY_BACKOFF`, or all candidates at hop ≥ D_max) — no global measurement needed. Parents reject joins that would exceed D_max.
3. **SVC layer shedding:** after 2 failed retry rounds in tree m, shed trees in ascending manifest priority (1080p first), never Tree 1 (base); 10 s hysteresis before re-join via the existing 5 s upward-migration loop. D_max overflow = saturation = shed, don't deepen.
4. **Source seed top-up:** broadcaster reserves ≥ 3·B₁ of upload as a Tree-1 emergency pool serving base-layer slots on starvation signals. The base layer never fails before enhancement layers.
5. **TFT interaction:** under saturation, enhancement slots allocate by TFT/PoU rank; base-layer slots form a universal service floor (ties into ISSUE-016 node classes).

---

## Resolution

Applied the capacity-adaptation ladder to the spec:

- **Created `protocol/chapter1/1.1_scale_latency/5_capacity_adaptation.md`** — defines the capacity ratio σ, separates the two thresholds (ū ≥ 8 Mbps for the D≤7 depth proof vs ū ≥ 6 Mbps for sustainability), gives per-tree sustainability (tree m sustainable iff mean relay upload ≥ B), the sustainable-layer-set rule with a worked table for the 3-layer SVC ladder, saturation detection via local join failure, the 2-round shed rule in ascending manifest priority with 10 s hysteresis, D_max overflow = shed (never deepen), the source-side Tree-1 reserve of 3·B₁, and the TFT split (enhancement by rank, base layer as universal service floor).
- `protocol/chapter1/1.1_scale_latency/1_bandwidth_paradox.md` — closing section distinguishing sustainability from fan-out and pointing to §5.
- `protocol/chapter1/1.1_scale_latency/README.md`, `protocol/INDEX.md` — new doc listed.
- `protocol/chapter1/1.2_multi_forest_overlays/4_stream_slicing_architecture.md` — the `priority` field is now normatively the shed order; base layer never shed.
- `protocol/chapter1/1.2_multi_forest_overlays/2_parent_selection_algorithm.md` — new depth admission rule (parents must reject joins at h ≥ D_max) and a normative definition of what `FAILURE_RETRY_BACKOFF` triggers.
- `protocol/appendix_b_parameters.md` — added σ_target = 1.33, shed rounds = 2, shed hysteresis = 10 s, source reserve = 3·B₁.
