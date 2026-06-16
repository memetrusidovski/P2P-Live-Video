# 1. Graph Collusion Auditing

A classic vulnerability of tokenless P2P reputation systems is **Collusion (or Mutual Backscratching)**, where a cluster of malicious Sybil nodes run by the same attacker exchange fake Proof-of-Upload receipts to pump each other's reputation scores without uploading actual video bytes.

To detect and neutralize this, we represent the local reputation graph $\mathcal{G} = (\mathcal{V}, \mathcal{E})$, where vertices $\mathcal{V}$ are peer S/Kademlia identities, and directed edges $\mathcal{E}$ represent PoU receipt validations. Neighboring nodes analyze this graph using localized graph audits:

**Symmetric Cycle Spotting:** If node $A$ presents receipts from node $B$, the auditing node checks if $B$ also possesses corresponding receipts from $A$. If the ratio of symmetric, bi-directional edge weights is close to 1.0:
$$\frac{W(A \to B)}{W(B \to A)} \approx 1.0 \quad \text{and} \quad \text{Degree}(A) \approx 1$$
the interaction is flagged as highly suspicious of collusion, since real video flows are overwhelmingly unidirectional (from parent to child).
