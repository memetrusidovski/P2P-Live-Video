# ISSUE-081: The registration sample is a fixed identity-keyed 1/2^s of relays that absorbs every DHT-directed join; above N_0 the DHT entry point yields only saturated candidates, every first round fails, and the starved re-query feeds the fold trigger

**Status:** Open
**Priority:** High
**Component:** Ch2 §2.3.2 Sampled Registration and Stream Discovery; Ch1 §1.2.2 §2.2 step 1; Ch1 §1.1.5 §5.6; App D §D.4.6b, §D.4.7
**Affects:** Every join above $N_0 = 10^4$; the SJL target; the fold trigger during any join surge
**File:** `protocol/chapter2/2.3_stream_registration/2_store_get_rpcs.md`, `protocol/chapter1/1.2_multi_forest_overlays/2_parent_selection_algorithm.md`, `protocol/chapter1/1.1_scale_latency/5_capacity_adaptation.md`, `protocol/appendix_d_frame_registry.md`

---

## Summary

Only relays whose NodeID falls in the $2^{-s}$ sample register, and the sample is fixed by identity. Round 1 of every join is built from the passive pool plus `GET_PEERS` (Ch1 §1.2.2 §2.2 step 1); the pool is empty at `CONNECTING` because `ACCEPTED` carries no Peer Records (§D.4.4). Every DHT-directed `NEIGHBOR` in the swarm therefore lands on the same sampled relays, whose spare slots are gone seconds after the stream reaches $N_0$ and refill only as their children churn.

## Detailed Description

1.  $N = 10^6$, $\ell = 0.3$, $M = 6$, $K_v = 10$, $s = 8$: about 960 registered relays per tree holding about 9,600 slots.
2.  Spare slots about 1,630 per tree, consumed in 9.8 s at 166.7 joiners/s; refill 1.6 slots/s at 1 %/min churn, so about 99 % of joiners get `SERVING`, $K_{\text{avail}} = 0$ from all candidates in every tree in round 1.
3.  The $\ge 1$ s backoff (Ch1 §1.1.5 §5.3 item 2) then lands on every join: a usable list at 1,960 ms instead of 800 ms, so SJL exceeds 1.5 s at the reference RTT before any media moves.
4.  Every failed joiner re-queries with `StarvedTrees = 0x3F`; a surge of $10^4$ joins/s gives `StarvedCount` $\approx 15{,}000$ per guardian and $\hat{s} \approx 7.3\% > s_{\text{fold}}$ on the base-layer trees, folding the top layer after 10 s for a swarm that is not short of capacity.

## Impact

Above $N_0$ the first join round fails by construction, startup latency doubles, and a join surge reads as base-layer starvation and strips the top layer from everyone. The charitable reading (gossip is the steady state) does not help a joiner that has no gossip neighbours yet.

## Proposed Fix

*   Let a saturated candidate act as a directory: `PROBE_RESPONSE` with $K_{\text{avail}} = 0$ carries up to $n$ Peer Records from the responder's pool $\mathcal{P}_m$, so round 1 reaches un-sampled relays at $+1$ RTT.
*   Or rotate the sample by epoch: $\text{Blake3}(\text{NodeID} \parallel \lfloor \text{LiveEdgeSegmentSeq} / 135 \rfloor) \bmod 2^s$.
*   Exclude a joiner's first round from the `StarvedTrees` bit (it has not been starved of anything yet).
*   Touches Ch2 §2.3.2, App D §D.4.6b and §D.4.7, Ch1 §1.2.2 §2.2, Ch1 §1.1.5 §5.6.

## Effort

Medium.

Found by the DHT-at-scale walk (`audits/traces/dht-at-scale.md`); confirmed by an independent verifier. Cluster with ISSUE-080 and ISSUE-082.
