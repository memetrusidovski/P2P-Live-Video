# ISSUE-080: The sampling exponent s ratchets upward once N_est exceeds N_0, because 2^s is applied to registrations made under earlier s and no frame tells registrants that s changed

**Status:** Open
**Priority:** High
**Component:** Ch2 §2.3.2 Sampled Registration and Query Discipline; Ch2 §2.3.3 Stream Record; App B N_0
**Affects:** Every stream that passes $10^4$ peers; the counts that size the forest, the PoW tier, the fold trigger and the unfold
**File:** `protocol/chapter2/2.3_stream_registration/2_store_get_rpcs.md`, `protocol/chapter2/2.3_stream_registration/3_registration_frames.md`, `protocol/appendix_b_parameters.md`

---

## Summary

The publisher sets $s = \max(0, \lceil \log_2(N_{\text{est}}/N_0) \rceil)$ and estimates $N_{\text{est}} = \text{median}_g(\text{ActiveCount}) \cdot 2^s$. A peer reads $s$ from the Stream Record at join, and refresh is `REGISTER_PEER`, not `GET_PEERS` (Query Discipline), so a registrant never learns that $s$ rose and keeps refreshing. `ActiveCount` therefore does not halve when $s$ steps, the next estimate doubles, and $s$ climbs one step per evaluation without bound. No rule has a guardian apply the sample test to a refresh.

## Detailed Description

1.  $\text{ActiveCount} = 10{,}080$, $s = 0 \Rightarrow N_{\text{est}} = 10{,}080 > N_0 \Rightarrow s = 1$. Next segment, same count: $N_{\text{est}} = 20{,}160 \Rightarrow s = 2$; then 3, 4, 5.
2.  After 15 s: $s = 15$, `SwarmSize` $= 10{,}400 \times 32{,}768 \approx 3.4 \times 10^8$; registration probability $2^{-15}$; every joiner reads a swarm above $10^5$ and solves the 14-bit dynamic puzzle.
3.  `RelayCount` scaled the same way crosses every unfold threshold; $\hat{s}_m$ of Ch1 §1.1.5 §5.6 is divided by $\hat{N}$ and goes to zero, so the fold never fires.
4.  SOLUTION-037 assumes out-of-sample registrations "age out over 135 s"; no mechanism stops their refresh, since the refreshing peer does not re-read $s$.

## Impact

Above $N_0$ the publisher's only view of the swarm is wrong by orders of magnitude within seconds, and every control loop that reads it (forest ladder, PoW tier, fold, unfold) acts on fiction.

## Proposed Fix

*   Guardians record the $s$ a registration was made under and report $\sum_i \text{count}_i \cdot 2^{s_i}$ in `STORE_RECORD_ACK`; the publisher changes $s$ at most once per active window (135 s).
*   Return the current $s$ in the registration acknowledgement so a refreshing peer can drop out of the sample, and let a guardian refuse a refresh that fails the current sample test.
*   Touches Ch2 §2.3.2, §2.3.3 (`STORE_RECORD_ACK`, registration acknowledgement), App B; `src/crates/bs-core/src/node/dht.rs`.

## Effort

Medium: one field in two frames, one rule.

Found by the flash-crowd walk (`audits/traces/flash-crowd.md`). Cluster with ISSUE-081.
