# 3. Optimistic Exploration

Optimistic unchoking applies to **PULL service** (§2). If nodes served PULLs only to peers that had already served them, a new joiner — whose buffer is empty and who therefore needs late-joiner backfill (Ch4 §4.3.1) before it can reciprocate anything — could never start. To solve this, the protocol allocates a small pool of **Optimistic Unchoke Slots** out of the PULL budget:

*   Every $2\text{ seconds}$ (4 TFT cycles), the node selects candidate peers from its Passive Set $\mathcal{P}$ or Active Set and unchokes them regardless of their current upload rate.
*   This allows a candidate $C$ to bootstrap its buffer and start uploading back.
*   If $C$ reciprocates within the next $2\text{ seconds}$ by uploading at an egress rate exceeding $\text{Threshold}_{\text{min}}$, it is promoted into the active unchoked pool; otherwise, it is re-choked, and a new candidate is explored.

## Adaptive Slot Count (Burst-Join Handling)

A single optimistic slot creates a serial bootstrap queue when several viewers join at once (a stream launch or a shared link): with $J$ simultaneous new joiners and one slot rotating every 2 s, the last joiner waits $\approx 2J$ seconds before receiving its first data — and longer in expectation, since selection is random.

The number of optimistic slots therefore scales with the observed number of *new* peers — those whose contribution score over the last 2 s is zero or near-zero — bounded both by an absolute cap and by the node's own capacity:

$$\text{optimistic\_slots} = \min\left(\left\lceil \frac{J_{\text{new}}}{3} \right\rceil,\ 4,\ \left\lfloor \frac{\text{PullSlots}}{2} \right\rfloor\right)$$

*   $J_{\text{new}} = 0$–$3$: 1 slot (identical to the classic single-slot behaviour in steady state).
*   Larger join bursts open up to 4 concurrent bootstrap slots.
*   The cap of 4 prevents a flood of new peers (or a Sybil wave) from consuming the unchoker and starving established contributors.

**$J_{\text{new}}$ is local, not global.** It counts new peers *this node* can see in its own Active and Passive sets — never the swarm-wide join rate. A 10,000-viewer launch does not give one node $J_{\text{new}} = 10{,}000$: joiners are spread across the peers returned by their own `GET_PEERS` lookups, so each existing peer sees a handful. The formula is sized for that local view, and reading it as a global count wildly overstates the tail.

### The Slot Budget Is Bounded by the PULL Reserve

Unchoke slots are not free-floating: every unchoked neighbour may draw $\beta_{\text{pull}} = 0.5$ Mbps, and a node has only $\text{PullSlots} = \lfloor r_{\text{pull}} u_v / \beta_{\text{pull}} \rfloor$ of them (§2). The two consumers of that budget **may never exceed it**:

$$\underbrace{\text{optimistic\_slots}}_{\le \lfloor \text{PullSlots}/2 \rfloor} + \underbrace{\text{regular TFT slots}}_{\text{the remainder}} \le \text{PullSlots}$$

The earlier draft counted these slots against the node's *tree* slot count $K_v$ and added a 20% service-floor reservation to the same sum. That conflated two budgets: tree slots are consumed by pushed stripes and allocated by rank at admission (Ch1 §1.2.2), and the base-layer floor is a rule on *tree* admission in the $L_0$ trees — neither is a PULL-side quantity. Keeping PULL inside $r_{\text{pull}} u_v$ is also what makes the slot count of Ch1 §1.2.1 honest: tree pushes and PULL service are drawn from disjoint shares of the same uplink.

A 10 Mbps uploader has two PULL slots; it runs one optimistic slot only when a joiner is actually waiting, and otherwise gives both to reciprocators. A node with a single PULL slot runs **no** optimistic unchoking and leaves bootstrapping to higher-capacity peers.

### Bootstrap Order Is FIFO, Not Random

Scaling the slot count fixes the *mean* wait but not the *tail*. With random selection repeated each round, a joiner's probability of still waiting after $k$ rounds decays geometrically but never reaches zero — some viewer always draws the short straw, which is what the startup-latency KPI ($\text{SJL} \le 1.5\text{ s}$, Ch8 §8.2) actually measures.

New joiners are therefore held in a **bootstrap queue ordered by first-seen timestamp**, and optimistic slots are filled from its head. Worst-case wait becomes deterministic:

$$t_{\text{wait}} \le \left\lceil \frac{\text{queue position}}{\text{optimistic\_slots}} \right\rceil \times 2\text{ s}$$

Three rules keep FIFO from being gamed, since "arrive first" is cheaper to attack than "win a lottery":

1.  A peer that consumes an optimistic slot and **fails** to reciprocate above $\text{Threshold}_{\text{min}}$ within its 2 s window goes to the **tail** with a $30\text{ s}$ cooldown before it is eligible again. An attacker cannot re-enter the queue head by cycling.
2.  Re-entry is keyed on **NodeID and on the `/24` (or `/48`) prefix**. A fresh identity is cheap (Ch2 §2.2.2 — about 10 ms of CPU), so keying on NodeID alone would let an attacker re-enter the head of the queue at will; the prefix cooldown is what actually holds.
3.  A peer that reciprocates is promoted to a regular TFT slot and leaves the queue entirely, freeing its slot immediately rather than at the end of the window.

Random selection is retained for its *other* purpose — discovering unknown capacity — and applies only when the bootstrap queue is empty.

```text
// Appended to the Sliding-Window Unchoker execution loop:

// Optimistic Unchoke: scale slots with the local join burst, bounded by PullSlots
BootstrapQueue <- { p in (PassiveSet ∪ ActiveSet) : ContributionScore_2s(p) ≈ 0
                                                   and not InCooldown(p) }
SortByFirstSeenAscending(BootstrapQueue)            // FIFO: longest wait first

PullSlots <- Floor(R_PULL * u_v / BETA_PULL)        // the PULL reserve in 0.5 Mbps slots (§2)
If PullSlots <= 1 then:
    SlotCount <- 0                                  // cannot afford exploration
Else:
    SlotCount <- Min(Ceil(Size(BootstrapQueue) / 3), 4, Floor(PullSlots / 2))

// Regular TFT slots are the remainder of the PULL budget, not a constant
RegularSlots <- PullSlots - SlotCount

For i in 1..SlotCount:
    If BootstrapQueue is not empty then:
        Candidate <- PopHead(BootstrapQueue)        // oldest waiter first
    Else:
        Candidate <- SelectRandomPeer(PassiveSet)   // classic capacity exploration
    SendControlPacket(Candidate, UNCHOKE_OPTIMISTIC)

// At the end of the 2 s window:
For each p optimistically unchoked this window do:
    If EgressRate(p) >= Threshold_min then:
        PromoteToRegularSlot(p)                     // leaves the queue
    Else:
        MoveToTail(p); SetCooldown(p, 30s)          // no head-of-queue cycling
```

## The Universal Service Floor Lives in Tree Admission

Leaf-class peers (Ch1 §1.2.5) — battery-constrained mobiles, onion-routed privacy nodes — cannot reciprocate uploads at all, yet the protocol guarantees them the **base layer** as a floor. That guarantee is a rule on **tree admission in the $L_0$ trees**, not on the PULL unchoker: base-layer slots are never rank-preempted, and a relay must hold at least $20\%$ of its $L_0$-tree slots open to leaf-class children before it may refuse one (Ch1 §1.2.2 *Rank Admission Rule*, Ch1 §1.1.5 §5.5). Enhancement-layer slots are **never** part of the floor: they are allocated by rank, so quality above the base layer is always earned.

What the unchoker contributes to the floor is PULL *repair* for leaves: a leaf holding an $L_0$ slot still loses blocks and must be able to pull them. A leaf never reciprocates, so it relies on optimistic slots for repair; the FIFO bootstrap queue admits it like any joiner, and the $\beta_{\text{pull}}$ per-slot cap bounds what it can draw. This keeps entitlement monotone in contribution — more upload always buys better playback — while ensuring a device that physically cannot upload still gets a watchable stream rather than a spinner.
