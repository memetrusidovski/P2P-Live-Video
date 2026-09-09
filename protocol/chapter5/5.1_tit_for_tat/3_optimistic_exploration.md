# 3. Optimistic Exploration

If nodes only uploaded to active contributors, new joining peers with empty buffers could never start downloading. To solve this, the protocol allocates a small pool of **Optimistic Unchoke Slots**:

*   Every $2\text{ seconds}$ (4 TFT cycles), the node selects candidate peers from its Passive Set $\mathcal{P}$ or Active Set and unchokes them regardless of their current upload rate.
*   This allows a candidate $C$ to bootstrap its buffer and start uploading back.
*   If $C$ reciprocates within the next $2\text{ seconds}$ by uploading at an egress rate exceeding $\text{Threshold}_{\text{min}}$, it is promoted into the active unchoked pool; otherwise, it is re-choked, and a new candidate is explored.

## Adaptive Slot Count (Burst-Join Handling)

A single optimistic slot creates a serial bootstrap queue when several viewers join at once (a stream launch or a shared link): with $J$ simultaneous new joiners and one slot rotating every 2 s, the last joiner waits $\approx 2J$ seconds before receiving its first data — and longer in expectation, since selection is random.

The number of optimistic slots therefore scales with the observed number of *new* peers — those whose contribution score over the last 2 s is zero or near-zero:

$$\text{optimistic\_slots} = \min\left(\left\lceil \frac{J_{\text{new}}}{3} \right\rceil,\ 4\right)$$

*   $J_{\text{new}} = 0$–$3$: 1 slot (identical to the classic single-slot behaviour in steady state).
*   Larger join bursts open up to 4 concurrent bootstrap slots.
*   The cap of 4 prevents a flood of new peers (or a Sybil wave) from consuming the unchoker and starving established contributors — regular TFT slots are never displaced.

Candidates are drawn preferentially from the new-joiner pool; if it is empty, a random Passive Set peer is explored as before.

## The Universal Service Floor

Leaf-class peers (Ch1 §1.2.5) — battery-constrained mobiles, onion-routed privacy nodes — cannot reciprocate uploads at all, yet the protocol guarantees them the **base layer** as a floor. The unchoker implements that guarantee here, and bounds it so contributors are never starved:

*   Base-layer (Tree 1) requests from verified leaf-class peers are served from a reserved floor allocation, independent of Tit-for-Tat rank.
*   The floor is capped at **20% of the node's upload slots** at any instant. Beyond that share, leaf-class requests wait for genuine surplus capacity.
*   Enhancement-layer slots are **never** part of the floor: they are allocated strictly by reciprocity rank, so quality above 480p is always earned (Ch1 §1.1.5).

This keeps entitlement monotone in contribution — more upload always buys better playback — while ensuring a device that physically cannot upload still gets a watchable stream rather than a spinner.

```text
// Appended to the Sliding-Window Unchoker execution loop:

// Optimistic Unchoke: scale slots with the observed join burst
NewJoiners  <- { p in (PassiveSet ∪ ActiveSet) : ContributionScore_2s(p) ≈ 0 }
SlotCount   <- Min(Ceil(Size(NewJoiners) / 3), 4)

For i in 1..SlotCount:
    If NewJoiners is not empty then:
        Candidate <- SelectRandomPeer(NewJoiners)   // bootstrap the burst first
        NewJoiners <- NewJoiners \ {Candidate}
    Else:
        Candidate <- SelectRandomPeer(PassiveSet)   // classic capacity exploration
    SendControlPacket(Candidate, UNCHOKE_OPTIMISTIC)
```
