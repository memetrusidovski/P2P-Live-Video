# 1. Graph Collusion Auditing

A classic vulnerability of tokenless P2P reputation systems is **Collusion (or Mutual Backscratching)**, where a cluster of malicious Sybil nodes run by the same attacker exchange fake Proof-of-Upload receipts to pump each other's reputation scores without uploading actual video bytes.

To detect and neutralize this, we represent the local reputation graph $\mathcal{G} = (\mathcal{V}, \mathcal{E})$, where vertices $\mathcal{V}$ are peer S/Kademlia identities, and directed edges $\mathcal{E}$ represent PoU receipt validations. Neighboring nodes analyze this graph using localized graph audits:

**Symmetric Cycle Spotting — per tree.** If node $A$ presents receipts from node $B$, the auditing node checks if $B$ also possesses corresponding receipts from $A$ **in the same tree** over the same window. If, for some tree $m$, the bi-directional edge weights are close to equal:
$$\frac{W_m(A \to B)}{W_m(B \to A)} \approx 1.0 \quad \text{within one 60 s window}$$
the interaction is flagged as highly suspicious of collusion. Within one tree a node has exactly one parent, so honest flow on a tree edge is strictly one-directional; two peers each serving the other *the same stripe* in the same minute is not a topology the placement rule can produce (short of one being evicted and rejoining under its former child within the window, which the ratio test then also has to pass).

**Why per tree.** Receipts carry a `TreeID` (Ch5 §5.2.3) precisely so that this check does not fire on the most common honest topology in mid-sized swarms: $A$ relays $T_1$ to $B$ while $B$ relays $T_3$ to $A$. Both edges carry a whole stripe at the same bitrate, so the *aggregate* ratio $W(A \to B)/W(B \to A)$ is exactly $1.0$ — and the probability that a given parent of $A$ is also $A$'s child in another tree is $\approx (M-1)K_v/N_{\text{relay}}$, giving each honest relay $\approx K_v^2(M-1)/N_{\text{relay}}$ perfectly symmetric edges: about $10$ at $N_{\text{relay}} = 50$, $2.5$ at $200$, $0.5$ at $1000$. An aggregate test flags most honest relays below a thousand relays; the per-tree test flags none of them, because their symmetric edges are in *different* trees.

**PULL receipts are excluded from the symmetry test and capped instead.** Mesh repair is legitimately bidirectional, so `PULL`-flagged receipts (TreeID `0x00`) cannot be judged by symmetry; they are bounded by the per-counterparty credit cap of Ch5 §5.2.2 ($\beta_{\text{pull}} \times 1$ s per segment), which limits what any pair can pump through fake repair to the rate genuine repair could reach.

## Small-Swarm Guard

The symmetric-edge heuristic is **only valid at sufficient swarm size**. In a small swarm, symmetric low-degree edges are structurally *expected* from legitimate behaviour: at $N = 5$ under the Multi-Forest placement rule, viewer $A$ relaying slice 3 to viewer $B$ while $B$ relays slice 1 to $A$ produces exactly the flagged signature ($W$-ratio $\approx 1.0$, degree $\approx 1$) — and at $N = 2$ the source–viewer pair itself is the only edge in the graph. Without a guard, a small swarm's three or four relays could each produce a *verified* symmetric-pair accusation against one another and reach the three-accuser eviction threshold of §3 — evicting legitimate viewers on evidence that is real but structurally innocent.

Two conditions therefore gate the detector:

1.  **Minimum swarm size:** collusion flags are suppressed entirely when the known swarm size is below $N_{\text{collusion}} = 20$. Below this scale, a colluding pair could fake only a handful of receipts — a negligible attack surface.

    The swarm size used here is a **security-relevant input**, so it is not taken from the DHT Stream Record alone. An adversary able to answer a victim's `GET_PEERS` with a stale-but-validly-signed record from early in the broadcast could otherwise switch the detector off swarm-wide. Every peer also holds a *locally verifiable lower bound* — the distinct peers it has itself seen in its Active Set, Passive Set and gossip history — and that bound cannot be forged by a third party. The guard therefore uses
    $$N_{\text{guard}} = \max\left(N_{\text{record}},\ \left|\text{distinct peers observed locally}\right|\right)$$
    so the detector can be disabled only by a peer that has genuinely seen almost nobody.

2.  **Degree relative to opportunity:** the raw symmetry score is scaled by how much counterparty diversity the node could plausibly have had:
$$\text{collusion\_confidence} = \text{symmetry\_score} \times \left(1 - \frac{\text{Degree}(A)}{\text{Opportunity}(A)}\right), \qquad \text{Opportunity}(A) = \min\left(N_{\text{guard}} - 1,\ c_a + c_p\right)$$

    Degree must be judged **against what was available, not in absolute terms.** A node with one counterparty in a swarm of three had no alternative; a node with one counterparty in a swarm of a million chose it. An earlier formulation used $1 - 1/\text{Degree}(A)$, which sets confidence to zero at $\text{Degree}(A) = 1$ at *every* scale — granting permanent immunity to precisely the topology the detector exists to catch, since a colluding pair's optimal strategy is then simply to keep its degree at 1. Dividing by opportunity preserves the genuine small-swarm insight ($\text{Opportunity} = 1 \Rightarrow$ confidence $0$) without creating that hole at scale.

    Symmetry remains the primary gate, and it is what keeps honest topologies clear: ordinary parent→child flow is overwhelmingly unidirectional, so $W(A \to B)/W(B \to A)$ is nowhere near $1.0$ and $\text{symmetry\_score} = 0$ regardless of degree.

```python
def check_collusion(A, B, swarm_size_record, locally_observed_peers):
    # Security-relevant input: take the larger of the signed record and what
    # this peer can verify for itself (Ch2 §2.3 records can be stale-replayed).
    N = max(swarm_size_record, len(locally_observed_peers))

    if N < COLLUSION_MIN_SWARM_SIZE:               # = 20 (Appendix B)
        return NOT_SUSPICIOUS                       # structurally expected symmetry

    # Per tree, tree receipts only (PULL receipts are capped, not symmetry-tested)
    symmetry_score = 0.0
    for m in trees_with_flow_both_ways(A, B, window=60s):
        ratio = W(A, B, tree=m) / W(B, A, tree=m)
        if abs(ratio - 1.0) < 0.1:
            symmetry_score = 1.0

    # Degree is judged against available counterparties, never in absolute terms.
    opportunity = min(N - 1, C_A + C_P)             # = min(N-1, 40) at defaults
    confidence  = symmetry_score * (1 - degree(A) / opportunity)

    return SUSPICIOUS if confidence > 0.5 else NOT_SUSPICIOUS
```

Worked values at $c_a + c_p = 40$:

| Scenario | $N$ | Degree | Opportunity | Confidence | Verdict |
| :--- | :---: | :---: | :---: | :---: | :--- |
| Source–viewer pair | 2 | 1 | 1 | $0$ | clear |
| Small swarm, mutual relay | 5 | 1 | — | suppressed | clear |
| **Colluding pair at scale** | $10^6$ | 1 | 40 | $0.98$ | **flagged** |
| Honest relay | $10^6$ | 40 | 40 | $0$ | clear |
| Honest leaf (asymmetric flow) | $10^6$ | 8 | 40 | $0$ (symmetry $=0$) | clear |
| **Mutual parents**, $A \to B$ in $T_1$, $B \to A$ in $T_3$ | 200 | 16 | 40 | $0$ (no same-tree symmetry) | clear |
| Same pair under the old aggregate test | 200 | 16 | 40 | $0.6$ | *would have been flagged* |
