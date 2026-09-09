# SOLUTION-006: Small-Swarm Churn Recovery (Tiered Candidate Sources)

**Closes:** ISSUE-006 (Medium)
**Lives in:** `protocol/chapter3/3.3_churn_recovery/1_recovery_timeline.md`
**Class:** Cold-start resilience

---

## The problem in one line

The 250 ms recovery path promotes a standby from the Passive Set, but the Passive Set can hold at most $N - 1 - |\mathcal{A}|$ entries — zero at $N = 5$ — so at small $N$ every churn event fell through to a 1–3 second DHT walk and a visible stall.

## The decision

A thin Passive Set is **supplemented from three further sources, ordered by cost then freshness**, with the cheap tiers tried concurrently:

| Tier | Source | Cost | Freshness |
| :---: | :--- | :--- | :--- |
| 1 | Passive Set | 0 RTT | maintained by SHUFFLE |
| 2 | Cached DISCOVERY peer list | 0 RTT | up to 90 s stale |
| 3 | Urgent `SHUFFLE` to a surviving active-set peer | ~1 RTT | current |
| 4 | Fresh DHT walk | 1–3 s | current |

## Why the original fix was not enough

The resolution took Option A (cached DHT list) and skipped Option B (ask a live peer), noting Option B was "more architecturally correct." Verification showed Option A alone has two defects.

### It discarded live standbys

The rule was `if |P| < 3: candidates = cached_list` — an **assignment**, not an extension. A peer with two live, recently-verified standbys throws them away in favour of a cache that may be 90 seconds old. Union, not substitution.

### The cache is far staler than the claim

The spec asserted this turns the small-$N$ worst case "from a 1–3 s DHT walk into a ~200 ms reconnect." That holds only if the cached entries are alive. The cache refreshes on DHT re-query, i.e. every $\tau_{\text{ttl}}/2 = 90\text{ s}$ — and it was originally captured at DISCOVERY, so for a long-running viewer it is a list of who was present *when they joined*. In a small swarm with turnover, much of it is dead. As a zero-cost first guess it is worth keeping; as a guarantee it was overstated.

### The insight that fixes it: a peer has M parents

The reason tier 3 works is that **losing one parent does not isolate a peer**. It holds up to $M$ parents, one per tree, plus its active set. When the tree-$T_m$ parent dies, the other $M-1$ are almost certainly alive — so there is always somebody to ask, and asking costs one RTT (~40–80 ms) for a *current* answer versus 1–3 s for the same thing from the DHT. That is why tier 3 sits above the DHT walk rather than below it, and why it is dispatched concurrently: its latency overlaps the tier-1/2 attempts instead of following them.

No new frame was needed. `SHUFFLE` (0x06) already exists to "discover fresh candidate peers" — the fallback is a new *use* of an existing mechanism, not a new mechanism.

### The source that looks obvious and is wrong

Siblings from the child roster are tempting — the roster is only 1 second old, two orders of magnitude fresher than the cache. But they were children of the *same dead parent* and are themselves orphaned; they are the last peers in the swarm who can serve as a new parent. Freshness is not the only axis. This is exactly the case the Deputy election of Ch1 §1.2.3 exists to resolve, and the two mechanisms must not be confused.

## Also added: keep the cache warm where it matters

A peer with a persistently thin Passive Set ($|\mathcal{P}| < c_p/4$) re-queries `GET_PEERS` every 30 s rather than 90 s. This costs almost nothing in the only regime where it applies — small $N$ means few peers, few guardians, a small response — and it is the same regime where cache freshness is the difference between a 200 ms reconnect and a stall.

## The generalisable lesson

**Fallback chains should be unions ordered by cost, not a sequence of substitutions.** The instinct to write `if primary is inadequate: use secondary` throws away partial primary results. The correct shape is `candidates = primary; if insufficient: candidates += secondary; dispatch expensive-but-fresh probes concurrently`.

## Validation owed (Chapter 8)

* Recovery time distribution at $N = 5, 10, 20, 40$ — does the tiered path actually hold the 250 ms budget, or only improve on 1–3 s?
* Hit rate of the cached list as a function of its age and of swarm turnover; if it is very low, tier 2 is not worth its complexity and tier 3 should be promoted.
