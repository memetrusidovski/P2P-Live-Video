# ISSUE-076: The growth-gated unfold re-creates the base-layer starvation that caused the fold whenever relays and viewers grow together

**Status:** Open
**Priority:** High
**Component:** Ch1 §1.1.5 §5.6 Unfold; App B Unfold trigger; Ch1 §1.1.5 §5.1 per-tree condition
**Affects:** Every swarm below the per-tree threshold of its current mapping that keeps growing; 18 unfold cycles between $N_{\text{relay}} = 500$ and $10^6$
**File:** `protocol/chapter1/1.1_scale_latency/5_capacity_adaptation.md`, `protocol/appendix_b_parameters.md`

---

## Summary

§5.6 restores a folded layer "only when the relay population has grown by $\ge 50\%$ since the fold". The per-tree condition the fold was judged on, $\bar{u}_{\text{relay}} \ge M B_m \Omega / ((1-\ell)(1-r_{\text{pull}}))$, has no $N_{\text{relay}}$ term: when relays and viewers grow together, supply and demand grow alike and the mean upload that caused the fold is unchanged. The unfold raises the base-layer stripe bitrate, every relay's $K_v$ falls, and children are drained into a tree with no free slot until the re-fold, which the 30 s dwell blocks.

## Detailed Description

1.  Inputs: $\bar{u}_{\text{relay}} = 5$ Mbps, $\ell = 0$, $M = 6$, folded to $(3,3)$ at $N_{\text{relay}} = 500$. The $(2,2,2)$ mapping needs $6 \times 0.75 \times 1.15 / 0.9 = 5.75$ Mbps $> 5$.
2.  At $N_{\text{relay}} = 750$ the trigger fires. $L_0$ stripes go 500 → 750 kbps; $K_v = \lfloor 4.5 / 0.8625 \rfloor = 5$ (was 7); supply per $L_0$ tree $2 + 125 \times 5 = 627$ of 750 subscribers; 123 (16.4 %) are drained with `DRAIN_NOTICE(CAPACITY)` at `EffectiveSegmentSeq`.
3.  $\hat{s}_1 = (20/3) \cdot (123 \times 0.15) / 751 = 0.164 > 0.05$ crosses about 10 s later; the dwell ("a fold never pre-empts it") holds the re-fold until $t = 150$ s; fill-in at 155 s. Freeze: 30.3 s for 16.4 % of the base layer.
4.  The cycle repeats at every $1.5\times$ growth: $\ln(2000)/\ln(1.5) = 18.7$ times from 500 to $10^6$ relays.
5.  SOLUTION-058 records as residual only the opposite case (upload improved, count unchanged).

## Impact

A growing swarm parked below its per-tree threshold gets a 30 s base-layer freeze for about a sixth of its viewers every time its relay count grows by half. The guarantee "resolution loss, never playback" is broken on a schedule the swarm's own growth sets.

## Proposed Fix

*   Gate the unfold on a quantity the per-tree condition contains: growth of the relay fraction $\text{RelayCount}/\text{ActiveCount}$ (which lowers $1/(1-\ell)$) by enough to move the folded layer's threshold, never on `RelayCount` alone.
*   State that an unfold whose $K_v$ falls drains children, and let a fold that follows an unfold pre-empt the dwell.
*   Touches Ch1 §1.1.5 §5.6, App B (Unfold trigger), SOLUTION-058's residual list; `src/crates/bs-core/src/node/resize.rs` `forest_consider`.

## Effort

Small to medium.

Found by the capacity-shortfall walk (`audits/traces/capacity-shortfall.md`). Cluster with ISSUE-071 (fold timing).
