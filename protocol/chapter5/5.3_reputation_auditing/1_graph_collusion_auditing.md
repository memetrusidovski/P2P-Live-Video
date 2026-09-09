# 1. Graph Collusion Auditing

A classic vulnerability of tokenless P2P reputation systems is **Collusion (or Mutual Backscratching)**, where a cluster of malicious Sybil nodes run by the same attacker exchange fake Proof-of-Upload receipts to pump each other's reputation scores without uploading actual video bytes.

To detect and neutralize this, we represent the local reputation graph $\mathcal{G} = (\mathcal{V}, \mathcal{E})$, where vertices $\mathcal{V}$ are peer S/Kademlia identities, and directed edges $\mathcal{E}$ represent PoU receipt validations. Neighboring nodes analyze this graph using localized graph audits:

**Symmetric Cycle Spotting:** If node $A$ presents receipts from node $B$, the auditing node checks if $B$ also possesses corresponding receipts from $A$. If the ratio of symmetric, bi-directional edge weights is close to 1.0:
$$\frac{W(A \to B)}{W(B \to A)} \approx 1.0 \quad \text{and} \quad \text{Degree}(A) \approx 1$$
the interaction is flagged as highly suspicious of collusion, since real video flows are overwhelmingly unidirectional (from parent to child).

## Small-Swarm Guard

The symmetric-edge heuristic is **only valid at sufficient swarm size**. In a small swarm, symmetric low-degree edges are structurally *expected* from legitimate behaviour: at $N = 5$ under the Multi-Forest placement rule, viewer $A$ relaying slice 3 to viewer $B$ while $B$ relays slice 1 to $A$ produces exactly the flagged signature ($W$-ratio $\approx 1.0$, degree $\approx 1$) — and at $N = 2$ the source–viewer pair itself is the only edge in the graph. Without a guard, gossip consensus at small $N$ trivially reaches the $\Theta_{\text{malicious}} \ge 0.8$ eviction threshold and bans legitimate viewers.

Two conditions therefore gate the detector:

1.  **Minimum swarm size:** collusion flags are suppressed entirely when the known swarm size (from the DHT Stream Record, Ch2 §2.3) is below $N_{\text{collusion}} = 20$. Below this scale, a colluding pair could fake only a handful of receipts — a negligible attack surface.
2.  **Degree-weighted confidence:** the raw symmetry score is scaled by the number of unique counterparties:
$$\text{collusion\_confidence} = \text{symmetry\_score} \times \left(1 - \frac{1}{\text{Degree}(A)}\right)$$
At $\text{Degree}(A) = 1$ the confidence is $0$ regardless of symmetry — a node with a single available peer has no choice but to interact with it, so symmetry there carries no evidence of collusion.

```python
def check_collusion(A, B, swarm_size):
    if swarm_size < COLLUSION_MIN_SWARM_SIZE:      # = 20 (Appendix B)
        return NOT_SUSPICIOUS                       # structurally expected symmetry
    ratio = W(A, B) / W(B, A)
    symmetry_score = 1.0 if abs(ratio - 1.0) < 0.1 else 0.0
    confidence = symmetry_score * (1 - 1 / degree(A))
    return SUSPICIOUS if confidence > 0.5 else NOT_SUSPICIOUS
```
