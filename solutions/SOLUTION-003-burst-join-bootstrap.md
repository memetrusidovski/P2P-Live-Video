# SOLUTION-003: Burst-Join Bootstrap (Optimistic Unchoke Slots)

**Closes:** ISSUE-003 (Medium)
**Lives in:** `protocol/chapter5/5.1_tit_for_tat/3_optimistic_exploration.md`, `2_sliding_window_unchoker.md`
**Class:** Startup latency / incentive-layer capacity accounting

---

## The problem in one line

A new viewer has an empty buffer and cannot reciprocate, so the only way it gets bootstrapped is the single optimistic unchoke slot rotating every 2 s — which serialises every simultaneous joiner into a queue, at a stream launch precisely when everyone arrives at once.

## The decision

Three changes, of which only the first was in the original issue:

1. **Scale the slots with the local join burst**, capped at 4 and at half the node's capacity:
   $$\text{optimistic\_slots} = \min\left(\left\lceil J_{\text{new}}/3 \right\rceil,\ 4,\ \lfloor K_v/2 \rfloor\right)$$
2. **Fill them FIFO** from a bootstrap queue ordered by first-seen time, not at random.
3. **Bound the whole slot budget by $K_v$** — optimistic + service floor + regular TFT $\le K_v$.

## Why scaling alone was not enough

The issue offered three options and the resolution took Option A (scale the slots), which is correct but incomplete. Verifying it against real behaviour turned up two ways it still fails.

### Random selection leaves an unbounded tail

Option A fixes the *mean* wait; it does nothing to the *tail*. Selection was still `SelectRandomPeer(NewJoiners)`, re-drawn every round with no memory, so a joiner's probability of still waiting after $k$ rounds is $(1 - s/J)^k$ — decaying, but never zero. Some viewer always draws the short straw, and that viewer is exactly what the $\text{SJL} \le 1.5\text{ s}$ KPI measures. The issue's own text flagged this ("some viewers may wait substantially longer than average") and then the applied fix didn't address it.

FIFO makes the bound deterministic: $\lceil \text{position} / \text{slots} \rceil \times 2\text{ s}$.

**The cost of FIFO is that it is cheaper to attack than a lottery** — "arrive first" beats "win a draw" — so it needs three guards that random selection did not:

* Failure to reciprocate sends a peer to the **tail** with a 30 s cooldown, so an attacker cannot re-enter at the head by cycling.
* Re-entry is keyed on NodeID, so a fresh identity costs a fresh static PoW puzzle.
* Success promotes the peer out of the queue immediately, freeing the slot mid-window rather than at its end.

Random selection is kept for its *other* job — probing unknown capacity — which only applies once the bootstrap queue is empty. The two purposes were conflated in one mechanism; separating them is most of the fix.

### The slot budget was not bounded by capacity — and broke worst at cold start

This is the serious one, and it is invisible until the two ladders are read together.

The spec said optimistic slots never displace regular TFT slots. With up to 4 of each, a node commits to 8 concurrent uploads. But a node has only $K_v = \lfloor u_v / B_m \rfloor$ slots, and $B_m = B/M$ — so $K_v$ depends on the **forest size**, which the ISSUE-002 ladder makes small exactly at cold start:

| Regime | $M$ | $B_m$ | $K_v$ for a 10 Mbps uploader |
| :--- | :---: | :---: | :---: |
| Steady state | 6 | 1 Mbps | 10 |
| Cold start | 1 | 6 Mbps | **1** |

At $M = 1$ the unchoker would reserve eight slots on a node that has one. Every peer it serves gets a fraction of the rate it needs — and this happens in the burst-join scenario the fix exists to solve, so the fix would have been at its least effective precisely where it was aimed.

Resolution: `UnchokeSlotsCount` is derived, not constant — the remainder of $K_v$ after the optimistic and floor reservations. A node with $K_v \le 1$ runs no optimistic unchoking at all and leaves bootstrapping to higher-capacity peers and the source's base-layer reserve.

## The generalisable lesson

**Any count of concurrent uploads must be expressed as a share of $K_v$, never as an absolute.** $K_v$ moves with $M$, which moves with $N$. Three separate constants in the incentive layer (4 regular slots, 4 optimistic slots, the 20% floor) were each independently reasonable and jointly unpayable. This is worth checking anywhere else the spec names a fixed number of peers to serve.

## Clarification that prevents a misreading

$J_{\text{new}}$ is **local** — new peers visible in this node's own Active and Passive sets, not the swarm-wide join rate. Read as a global count, the formula implies a 10,000-viewer launch produces a 500-second tail at 4 slots. It does not: joiners are dispersed across the peers each of them found via its own `GET_PEERS`, so any one existing peer sees a handful. Stated explicitly in the spec because the formula invites the wrong reading.

## Validation owed (Chapter 8)

* SJL distribution — specifically the 99th percentile, not the mean — under a synchronised launch of 10 / 100 / 1,000 viewers.
* Whether the 30 s cooldown is sufficient against a Sybil wave that cycles identities to hold queue-head positions.
* Whether $\lfloor K_v/2 \rfloor$ is the right ceiling, or whether bootstrap deserves a larger share of a high-capacity node's budget.
