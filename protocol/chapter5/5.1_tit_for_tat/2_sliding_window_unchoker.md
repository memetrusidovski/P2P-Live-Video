# 2. Sliding-Window Unchoker

## What the Unchoker Governs

Tit-for-Tat governs **PULL service** — the reactive, mesh-side traffic of Ch4 §4.3: repair of lost blocks and late-joiner backfill — and *only* that. It does **not** govern tree slots. The distinction is forced by the traffic pattern: a tree edge carries data in one direction, parent to child, so a tree child's "ingress rate to its parent" is zero by construction (Ch5 §5.3.1 relies on exactly this asymmetry). An unchoker that allocated tree slots by reciprocal ingress would choke every child within one cycle and dissolve the forest. Tree slots are allocated by **contribution rank** at admission time (Ch1 §1.2.2 *Rank Admission Rule*), using the receipts a peer earned by uploading to *anyone*; the unchoker allocates the PULL budget by reciprocity between mesh neighbours, which is bidirectional and is what Tit-for-Tat was designed for.

The budget it allocates is the PULL reserve of Ch1 §1.2.1, $u_{\text{pull}} = r_{\text{pull}} \cdot u_v$ with $r_{\text{pull}} = 0.10$, divided into slots of $\beta_{\text{pull}} = 0.5$ Mbps each:

$$\text{PullSlots} = \left\lfloor \frac{r_{\text{pull}} \cdot u_v - \sum_{\text{handovers in progress}} B_m \Omega_v}{\beta_{\text{pull}}} \right\rfloor$$

— two slots on a 10 Mbps uploader, twenty on 100 Mbps, two thousand on 10 Gbps, when no handover is in progress. The subtraction is the other consumer of the reserve: a preemption or displacement in an assigned tree (Ch1 §1.2.2 *The Drain Path*) serves one child beyond $K_v(m)$ until the drained child leaves, and that transient slot is paid for here — the owning expression is in Ch1 §1.2.1 *The Two Budgets*. `PullSlots` is recomputed at every cycle, so a handover chokes a PULL neighbour for its duration and releases the slot when it ends. An unchoked neighbour may draw up to $\beta_{\text{pull}}$ sustained from this node; a joiner backfilling its buffer draws from several unchoked neighbours in parallel.

## The Loop

Every $500\text{ ms}$, the peer executes the following scheduler to evaluate reciprocity across its Active Set $\mathcal{A}$:

```text
Algorithm: Sliding-Window Tit-for-Tat Unchoker (PULL service)
Input: ActiveSet, PullSlots (from r_pull · u_v / beta_pull), OptimisticSlots (see §3)
Output: New Choke/Unchoke States for PULL service

1:  CurrentTime <- GetMicrosecondTimestamp()
2:  NeighborsList <- GetPeersInActiveSet(ActiveSet)
3:
4:  // Rolling PULL bytes each neighbor served TO US over the last 2 seconds
5:  For each peer in NeighborsList do:
6:      peer.RollingIngressRate <- PullBytesReceivedFrom(peer, window=2_000_000) / 2 s
7:      If peer missed a receipt deadline (Ch5 §5.2.1): peer.RollingIngressRate <- 0
8:
9:  // Sort by what they gave us, descending
10: SortByIngressRateDescending(NeighborsList)
11:
12: RegularSlots <- PullSlots - OptimisticSlots
13: // Unchoke top reciprocators for PULL service
14: For i from 0 to RegularSlots - 1 do:
15:     BestPeer <- NeighborsList[i]
16:     If BestPeer.ChokedState == CHOKED then:
17:         BestPeer.ChokedState <- UNCHOKED
18:         SendControlPacket(BestPeer, CHOKE_STATE=UNCHOKE)
19:
20: // Choke non-reciprocating peers (optimistically unchoked peers exempt, §3)
21: For i from RegularSlots to Size(NeighborsList) - 1 do:
22:     BadPeer <- NeighborsList[i]
23:     If BadPeer.ChokedState == UNCHOKED and not BadPeer.Optimistic then:
24:         BadPeer.ChokedState <- CHOKED
25:         SendControlPacket(BadPeer, CHOKE_STATE=CHOKE)
```

`PullSlots` is **not a constant**: it is a share of the node's upload, and the optimistic-bootstrap slots of [3. Optimistic Exploration](3_optimistic_exploration.md) are carved out of it — the two together may never exceed `PullSlots`. A node with one PULL slot gives it to its best reciprocator and runs no optimistic exploration.

The Tit-For-Tat Rule is strictly enforced for PULL service:
$$\text{Status}(B) = \begin{cases}
  \text{Unchoked (PULL served)} & \text{if } \text{PullRate}(B \to A) \ge \text{Threshold}_{\min} \ \text{or } B \text{ holds an optimistic slot} \\
  \text{Choked (PULL refused)} & \text{otherwise}
\end{cases}$$

**Choking never touches the tree.** A choked peer that is also a tree child keeps receiving its pushed stripe; what it loses is repair and backfill service from this node. Tree membership is lost only through the receipt deadline (three unreceipted segments, Ch5 §5.2.1) or rank preemption (Ch1 §1.2.2).
