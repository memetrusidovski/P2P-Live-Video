# 1. Graph-Theoretic Foundations & Slice Assignment

## 1.1 The Directed Graph Formulation

To rigorously define the Multi-Forest overlay, we model the entire peer-to-peer live streaming swarm as a directed graph $G = (V, E)$, where:
*   $V$ is the set of all active nodes (peers), including the root broadcaster node $S$.
*   $E$ is the set of active transport connections (edges) where data flows from a parent to a child. Let $e = (u, v)$ represent an edge where node $u$ uploads to node $v$.

The core engineering challenge in P2P streaming is that while a node's downstream capacity $d_v$ is generally large (e.g., $1000\text{ Mbps}$), its upstream capacity $u_v$ is often severely constrained (e.g., $10\text{ Mbps}$). If the video bitrate is $B = 6\text{ Mbps}$, a standard tree structure forces a node $v$ to upload the full $6\text{ Mbps}$ to each child. A node with $u_v = 10\text{ Mbps}$ can only support $k = 1$ child, creating an incredibly deep, fragile, and high-latency chain.

## 1.2 Edge-Disjoint Spanning Trees

To solve this, we divide the stream across $M$ independent sub-streams, one per tree. Tree $T_m$ carries a bitrate $B_m$ **declared per tree by the source** in the slicing matrix (§4.2.1, Appendix D §D.4.8): a whole SVC layer, a stripe of one, or a bundle of several, depending on how $M$ compares with the number of encoder layers. $B_m = B/M$ is the *nominal average* and is used only for coarse sizing; on the reference ladder actual per-tree bitrates range from $0.75$ to $3.0$ Mbps (§4.2.1). Wherever a formula below takes $B_m$, it means the tree's own declared bitrate.

We then decompose the global graph $G$ into a **Multi-Forest**, which is a collection of $M$ directed spanning trees:
$$\mathcal{F} = \{T_1, T_2, \dots, T_M\}$$
such that $T_m \subseteq G$ for all $m \in [1, M]$.

Ideally, these trees are **edge-disjoint**, meaning that if an edge $e = (u, v)$ exists in tree $T_a$, it cannot exist in tree $T_b$. This prevents duplicate traffic on any single network socket.

## 1.3 The Orthogonal Placement Rule (Capacity Balancing)

The most critical algorithmic constraint of the Multi-Forest is the **Orthogonal Placement Rule**. It mathematically guarantees that no single node is overwhelmed with upload requests across multiple slices.

### Slot Count

A node's upload is spent on three things: pushing tree slices to its children, serving `PULL_REQUEST`s (repair and late-joiner backfill, Ch4 §4.3), and the protocol overhead that rides on every slice — RaptorQ parity and per-packet framing. A slot count that ignores the last two over-commits the link by a factor that grows on exactly the lossy last miles where headroom matters most (see the overhead factor below). Every `RELAY`-class node $v$ therefore computes its active upload slots **per assigned tree** $m$ as

$$K_v(m) = \left\lfloor \frac{(1 - r_{\text{pull}})\, u_v}{t_v \cdot B_m \cdot \Omega_v} \right\rfloor, \qquad \Omega_v = \left(1 + \frac{\bar{E}_v}{K}\right)\left(1 + f_{\text{frame}}\right)$$

where:

*   $u_v$ is the node's measured physical upload capacity;
*   $r_{\text{pull}} = 0.10$ is the share of upload reserved for PULL service (Ch5 §5.1.3) — never available to tree slots;
*   $t_v$ is the number of trees the node is assigned to (1 for an ordinary relay; see *Capacity-Proportional Multi-Tree Assignment* below);
*   $B_m$ is tree $m$'s declared bitrate;
*   $\Omega_v$ is the **overhead factor**: $\bar{E}_v$ is the mean number of RaptorQ parity symbols per $K = 16$-symbol block currently sent to the node's children (Ch4 §4.2.2; initialised to $E_{\min} = 1$ before any child exists), and $f_{\text{frame}} = 0.085$ is the fixed per-symbol framing overhead — the 12-byte `RAPTORQ_SYMBOL` header, QUIC datagram frame and short header, AEAD tag and UDP/IPv4 headers ($\approx 70$ bytes per 1024-byte symbol, $6.8\%$) plus one `BLOCK_PROOF` per 16 KB block ($\approx 1.5\%$).

$\Omega_v$ is $1.15$ at the clean-link parity floor and $1.42$ at the $30\%$ parity ceiling. It is recomputed once per segment, when parity is; a $K_v(m)$ that **falls** is honoured by releasing children through the drain path (§2.2 *The Drain Path*, `DRAIN_NOTICE` reason `CAPACITY`), never by dropping them. Because a rise in loss now *reduces* the slot count rather than only raising egress, the parity loop is damped: oversubscription → loss → more parity → fewer slots → less egress, instead of the positive feedback an overhead-blind count produces.

### The Two Budgets, Each Owned by One Expression

A relay's upload is split once, into a **tree budget** $(1 - r_{\text{pull}})\,u_v$ and a **PULL reserve** $r_{\text{pull}}\,u_v$. Each has several consumers in several chapters, and each has exactly one expression that owns its total:

$$\underbrace{\sum_{m \in \mathcal{T}_{\text{assigned}}} \text{children}_v(m)\, B_m \Omega_v}_{\text{tree slots, Ch1}} \;+\; \underbrace{U_{\text{bridge}}}_{\text{bridged children, Ch6 §6.3}} \;\le\; (1 - r_{\text{pull}})\, u_v, \qquad U_{\text{bridge}} = \sum_{\text{bridged}} B_m \Omega_v \;\le\; \tfrac{1}{2}(1 - r_{\text{pull}})\, u_v$$

$$\underbrace{\beta_{\text{pull}} \cdot \text{PullSlots}_v}_{\text{Tit-for-Tat unchoke, Ch5 §5.1}} \;+\; \underbrace{\sum_{\text{handovers in progress}} B_m \Omega_v}_{\text{preemption / displacement overlap, §2.2}} \;\le\; r_{\text{pull}}\, u_v$$

`PullSlots` is therefore **not** $\lfloor r_{\text{pull}} u_v / \beta_{\text{pull}} \rfloor$ unconditionally; it is recomputed at every Tit-for-Tat cycle from whatever the handovers in progress leave, and a handover that would not fit even with every PULL slot choked is made sequential rather than overlapped (§2.2). A relay that carries bridged children (Ch6 §6.3) computes its assigned-tree slots with $U_{\text{bridge}}$ removed from the numerator:

$$K_v(m) = \left\lfloor \frac{(1 - r_{\text{pull}})\, u_v - U_{\text{bridge}}}{t_v \cdot B_m \cdot \Omega_v} \right\rfloor$$

An earlier draft spent the PULL reserve three times — the unchoker took all of it as slots, a preemption charged "one extra slot" to it, and every emergent-relay bridge charged $B_m \Omega$ to it — with no expression owning the total. On the reference $10$ Mbps relay the reserve is $1.0$ Mbps, which cannot hold even one preempted $1.5$ Mbps slot ($1.73$ Mbps with overhead), so an implementer had to choose between a link at $107\%$ and a rank rule that never fired. Bridges are moved to the tree budget because a bridged child *is* a subscriber served — capacity spent on it is not capacity lost to the forest — and a $10\%$ reserve could not hold a single $3.0$ Mbps bridge on the $20$ Mbps node the relay criteria admit, i.e. no bridge at all at $M \le 3$, where the home broadcaster behind NAT (Ch6 §6.3.3) actually lives. The half-budget cap keeps a relay from becoming a pure bridge and abandoning its assigned tree, and bounds the $3\times$ receipt multiplier (Ch6 §6.3.2) to at most $2\times$ over pure tree relaying.

*(Worked values on the $M = 6$ reference mapping of §4.2.1, $\Omega_v = 1.15$: a $u_v = 10$ Mbps single-tree node holds $K_v = 10$ slots if assigned a $0.75$ Mbps $L_0$ or $L_1$ stripe and $5$ on a $1.5$ Mbps $L_2$ stripe. The same node under the overhead-blind $\lfloor u_v / B_m \rfloor$ would claim $13$ and $6$ — and deliver every child $13$–$15\%$ below the rate it needs.)*

This rule applies to **`RELAY`-class nodes only**. Battery-, data-, or reachability-constrained devices declare themselves leaf-class and bear no relay duty in any tree — see [5. Node Classes and Relay Eligibility](5_node_classes.md), which also corrects the relay density used in capacity calculations to $N_{\text{relay}}/M$ rather than $N/M$.

To ensure fairness, a *standard* node $v$ acts as an **Interior Node** (a relay that uploads data) in exactly **one** tree $T_a \in \mathcal{F}$. In all other $M-1$ trees, node $v$ acts as a **Leaf Node** (a sink that only downloads data).

Let $\text{out-degree}(v, T_m)$ be the number of children node $v$ has in tree $T_m$. The constraint for a standard node is:
$$\exists! \ a \in [1, M] \text{ such that } \text{out-degree}(v, T_a) \le K_v(a)$$
$$\forall b \ne a, \quad \text{out-degree}(v, T_b) = 0$$

One bounded exception exists: an emergent relay bridging a NAT-blocked leaf (Ch6 §6.3) holds out-degree $1$ per bridged child in a tree it is not assigned to. Bridged children are charged to the tree budget as $U_{\text{bridge}}$ above, are never listed in the relay's `AssignedTrees`, and never make it a Deputy candidate for that tree; the exception changes the accounting, not the placement rule's purpose.

### Deterministic Tree Assignment (Rendezvous Ranking)

To decide *which* tree $T_a$ a node relays for, the node ranks every tree by a keyed hash of its own identity and takes the highest:

$$\text{rank}_v(m) = \text{Blake3}\left(NodeID_v \parallel \text{uint8}(m)\right), \qquad a(v) = \arg\max_{m \in [1, M]} \text{rank}_v(m)$$

with the 32-byte digest compared as a big-endian integer. Across a swarm this distributes relay burden uniformly over the $M$ trees, with approximately $N_{\text{relay}}/M$ relays per tree — the same statistical property as a plain modulus — but with one property a modulus lacks: **stability under a change of $M$.** When the forest grows from $M$ to $M+1$ a node moves only if the new tree outranks its current one, which happens for $\approx 1/(M+1)$ of relays; when it shrinks, only the closed tree's relays move. Under $\text{Blake3}(NodeID) \bmod M$, by contrast, $5 \to 6$ reassigns $83\%$ of relays and $2 \to 3$ reassigns $67\%$ — every ladder step tore down and rebuilt essentially the whole forest, warming every relay at once and forcing every peer to re-join every tree (§4.5). The rendezvous form is why a resize is now an incremental operation.

The mapping from tree *index* to layer *content* is the source's business (§4.2.1) and may change at a resize without moving any relay: a relay forwards whatever the source emits on its tree.

### Capacity-Proportional Multi-Tree Assignment (Super Nodes)

The single-tree rule exists to stop *ordinary* peers from being overwhelmed — but applied to a high-capacity node it wastes most of that node's value. A 10 Gbps server locked to one tree spends its entire budget on one slice while the other $M-1$ trees get nothing from it; covering all trees with hash-assigned single-tree servers would take ~14 servers (birthday-problem coverage) where one should suffice.

A node whose upload can carry at least two *full-stream-equivalent* loads — measured with the same overhead and PULL reserve as every other slot computation — therefore relays in multiple trees:

$$\text{max\_trees}(v) = \min\left(M,\ \max\left(1, \left\lfloor \frac{(1 - r_{\text{pull}})\, u_v}{\Omega_v \cdot B} \right\rfloor\right)\right)$$

and takes the $t_v = \text{max\_trees}(v)$ trees with the **highest rendezvous ranks**, so a multi-tree node's set is the natural extension of its single-tree assignment and is equally stable under resizes. Its per-tree slots follow from the slot formula with that $t_v$.

```python
def compute_tree_assignments(node_id, u_v, M, B, omega, r_pull=0.10):
    max_trees = min(M, max(1, floor((1 - r_pull) * u_v / (omega * B))))
    ranked = sorted(range(1, M + 1),
                    key=lambda m: blake3(node_id + bytes([m])), reverse=True)
    return ranked[:max_trees]          # max_trees == 1 → the standard single-tree rule

def slots_in_tree(u_v, t_v, B_m, omega, r_pull=0.10):
    return floor((1 - r_pull) * u_v / (t_v * B_m * omega))
```

Worked examples ($B = 6$ Mbps, $\Omega_v = 1.15$, $M = 6$ reference mapping of §4.2.1 — slots shown per $0.75$ Mbps $L_0$/$L_1$ stripe and per $1.5$ Mbps $L_2$ stripe):

| Node upload $u_v$ | max_trees | slots per 0.75 Mbps tree | slots per 1.5 Mbps tree | Effect |
| :---: | :---: | :---: | :---: | :--- |
| 10 Mbps | 1 | 10 | 5 | Unchanged standard behaviour |
| 16 Mbps | 2 | 8 | 4 | First multi-tree tier ($0.9\,u_v \ge 2\,\Omega B \approx 13.8$ Mbps) |
| 50 Mbps | 6 | 8 | 4 | Interior in every tree |
| 10 Gbps | 6 | 1734 | 867 | One server backbones the whole forest (~8,700 children) |

The anti-hotspot intent of the Orthogonal Placement Rule is preserved by the **per-tree slot cap**: a multi-tree node can never devote more than $1/t_v$ of its tree budget to any single tree. Multi-tree nodes register in the DHT with every assigned tree set in their `AssignedTreeBitmap` (Ch2 §2.3.3) so per-tree `GET_PEERS` discovery finds them, and their `PROBE_RESPONSE` lists the full assigned-tree set rather than a single tree ID.

#### The Distinct-Parent Rule (Preserving Failure Independence)

The slot cap bounds *bandwidth* concentration, but multi-tree assignment introduces a second kind of concentration it does not address: **correlated failure**. The forest is edge-disjoint so that losing one node costs a child one tree's worth of bitrate, not all of it. If a child were to take the same super node as its parent in all $M$ trees, that guarantee is gone — one machine dying takes 100% of that child's stream at once, and the multi-forest becomes an expensive way to build a single tree.

The constraint that restores it is **per child, not per server**:

> A peer selects at most **one** parent per distinct NodeID across all $M$ trees.

This costs the super node nothing. A 10 Gbps server assigned to all six trees still fills every slot — it simply fills them with distinct children rather than the same children taken six times each. The natural configuration was already the safe one; the rule exists to stop parent selection from converging on the unsafe one, which it otherwise would, since the same node wins the capacity score in every tree simultaneously.

Where the rule cannot be satisfied — a small swarm with too few relays to offer $M$ distinct parents — the peer accepts the duplication but **marks the affected trees as correlated**. Churn recovery (Ch3 §3.3) treats a correlated set as a single failure domain: losing that parent is expected to orphan every tree in the set at once, so the peer pre-selects replacements for all of them rather than repairing one tree at a time.

Node-level distinctness is the floor, not the ceiling. Two NodeIDs in the same $/24$ (or behind one datacenter ASN) are also correlated, and the **prefix cap** of Ch2 §2.2.2 applies to a peer's parent set and to a relay's child slots as well as to its k-buckets and Active Set: at most $c_p(K) = \max\left(1, \lceil K / \min(P_{\text{obs}}, 20) \rceil\right)$ of a slot set of size $K$ from one $/24$ or $/48$, where $P_{\text{obs}}$ is the number of distinct prefixes the node has itself observed. The cap is $1$ of $M \le 6$ parents and $\lceil K_v(m)/20 \rceil$ children per tree once the node has seen twenty prefixes, and it relaxes to no limit in a swarm that genuinely contains only one — the same-$/24$ dorm or office stream, which under an unconditional $5\%$ rule could not form a forest at all (Ch2 §2.2.2).

#### Multi-Tree Eligibility Must Be Earned, Not Declared

$u_v$ is **self-declared**. Under the single-tree rule, a node lying about its capacity could attract and then starve at most one tree's worth of children — bad, bounded, and quickly corrected by the reliability term $R$ in the parent score. Multi-tree assignment multiplies that blast radius by $M$: one node claiming 10 Gbps is granted interior status in every tree and can black-hole children across the entire forest at once. Warm-up gating (§2.1) forces it to *receive* a verified segment first, but nothing so far forces it to *forward* one.

Multi-tree standing is therefore gated on demonstrated throughput rather than claimed capacity. A node's effective assignment is

$$\text{max\_trees}^{\text{eff}}(v) = \min\left(\text{max\_trees}(v),\ 1 + \left\lfloor \frac{\Theta^{\text{rate}}_v}{B} \right\rfloor\right)$$

where $\Theta^{\text{rate}}_v$ is the node's PoU-verified delivered throughput over the trailing $60\text{ s}$ (Ch5 §5.2) — receipts signed by the children it actually served, not a number it asserts about itself.

Every node therefore starts at one tree and earns each additional one by having already delivered a full stream's worth of bitrate. A genuine 10 Gbps server reaches all six trees within a couple of minutes of joining; a node that claims 10 Gbps and forwards nothing never leaves its first tree. This keeps super-node placement consistent with the protocol's core axiom — standing follows contribution — rather than making capacity the one claim taken on trust.

The gate is a **cost**, not a proof. Receipts are signed by downloaders, and a downloader identity costs $\approx 10$ ms (Ch2 §2.2.1); an uploader that controls leaf-class Sybil downloaders can have them sign receipts for blocks it never sent, and no verifier can resolve a leaf-class signer's address to apply the subnet penalty (Ch5 §5.3.2). What the gate raises the price of forging multi-tree standing *to* is $S$ distinct identities per minute — from nothing. The blast radius of a node that forges its way into every tree is bounded elsewhere: by the prefix cap on how many of the forest's children one prefix may hold (Ch2 §2.2.2), and by its children observing non-delivery within one segment, scoring it down on $R$, and leaving. An earlier version of this section claimed the gate made standing unforgeable; it makes it *unforgeable for free*, which is the weaker and true statement.

## 1.4 Dynamic Forest Sizing

$M$ is **not a fixed constant** — it scales with the relay population up to the ceiling $M_{\text{max}} = 6$ (Appendix B). A fixed $M = 6$ at small $N$ would leave most trees without any hash-assigned relay until $N_{\text{relay}} \approx 18$ (a birthday-problem coverage effect), forcing the source to push those slices directly to every viewer — full CDN load exactly when the streamer's upload budget matters most.

### The Ladder Input Is $N_{\text{relay}}$, Not $N$

Coverage depends on how many nodes *can relay*, and leaf-class peers (§5) relay nothing. A swarm of $30$ viewers half of whom are phones has $15$ relays, and at $M = 6$ the probability that some tree has no relay at all is $\approx 33\%$; keyed on $N$, the ladder would put that swarm at $M = 6$ and hand the source a $B_m \cdot N$ push for every empty tree. The source therefore sizes the forest on $N_{\text{relay}}$ — the count of **active** `RELAY`-class registrations, which DHT guardians report to the publisher on every Stream Record store (Ch2 §2.3.2). A registration counts as active only if refreshed within $1.5 \cdot \tau_{\text{ttl}}/2 = 135\text{ s}$, so that departed peers stop inflating the count within one refresh period rather than lingering for the full $180$ s TTL.

| Relays $N_{\text{relay}}$ | $M$ | Nominal $B/M$ | Actual per-tree bitrates (reference ladder, §4.2.1) |
| :---: | :---: | :---: | :--- |
| $N_{\text{relay}} < 12$ | 2 | 3 Mbps | $3.0,\ 3.0$ |
| $12 \le N_{\text{relay}} < 18$ | 3 | 2 Mbps | $1.5,\ 1.5,\ 3.0$ |
| $18 \le N_{\text{relay}} < 24$ | 4 | 1.5 Mbps | $1.5 \times 4$ |
| $24 \le N_{\text{relay}} < 30$ | 5 | 1.2 Mbps | $0.75 \times 2,\ 1.5 \times 3$ |
| $N_{\text{relay}} \ge 30$ | 6 | 1 Mbps | $0.75 \times 4,\ 1.5 \times 2$ (full protocol) |

The ladder holds $N_{\text{relay}}/M \ge 5$ from $M = 3$ upward, which bounds the expected number of empty trees below $\approx 0.04\,M$. The current $M$ is carried in the stream manifest's `num_trees` field; transitions are announced via the `MANIFEST_UPDATE` frame with a 5-second migration window (see [4. Stream Slicing Architecture](4_stream_slicing_architecture.md), §4.5).

### Why the Ladder Starts at $M = 2$

An earlier version of this ladder had an $M = 1$ rung for $N < 6$, on the grounds that a single tree has "no splitting overhead". Two things are wrong with $M = 1$, and both bite in the first minutes of every stream:

*   **It excludes ordinary uploaders.** With $B_m = B$, any relay with $(1 - r_{\text{pull}})\,u_v < \Omega B \approx 6.9$ Mbps — typical DSL, LTE and entry-level cable — has $K_v = 0$ and contributes nothing, so the source pushes the full stream to every viewer itself. At $M = 2$ the same relays hold one slot each.
*   **It has a single point of failure per peer.** One parent carries 100% of the stream, and this is the regime where recovery is weakest (the Passive Set is empty at $N \le 5$, Ch3 §3.3).

The source-egress comparison settles it. For a source plus four viewers:

| Viewer uploads | $M = 1$ source egress | $M = 2$ source egress (expected over assignments) |
| :--- | :---: | :---: |
| $5$ Mbps each ($K_v = 0$ at $M{=}1$, $1$ at $M{=}2$) | $24$ Mbps | $\approx 12$–$13$ Mbps |
| $10$ Mbps each ($K_v = 1$ at $M{=}1$, $2$ at $M{=}2$) | $6$ Mbps | $6$ Mbps |

$M = 2$ is never worse and is roughly $2\times$ better for the common upload class; at $N = 2$ the two are identical. The only "splitting overhead" is one additional QUIC connection per peer. The ladder therefore begins at $M = 2$, which also removes one forest rebuild from every stream's growth.

### Ladder Hysteresis and Minimum Dwell

The ladder above gives the **upward** thresholds only. Applied symmetrically it would flap: a swarm hovering at $N_{\text{relay}} = 29$–$30$ would cross the $M{=}5 \leftrightarrow M{=}6$ boundary repeatedly, and each crossing costs a `MANIFEST_UPDATE`, a warm-up for the moving relays and a re-join into the new tree for every peer. Running at a slightly suboptimal $M$ is strictly cheaper.

Two guards apply to every $M$ change:

1.  **Asymmetric thresholds.** $M$ increases at the ladder value $N_{\text{up}}(m)$, but decreases from $m$ to $m-1$ only when $N_{\text{relay}} < N_{\text{up}}(m) - 3$ — half a ladder step of slack:

    | Transition | Grow when | Shrink when |
    | :---: | :---: | :---: |
    | $2 \leftrightarrow 3$ | $N_{\text{relay}} \ge 12$ | $N_{\text{relay}} < 9$ |
    | $3 \leftrightarrow 4$ | $N_{\text{relay}} \ge 18$ | $N_{\text{relay}} < 15$ |
    | $4 \leftrightarrow 5$ | $N_{\text{relay}} \ge 24$ | $N_{\text{relay}} < 21$ |
    | $5 \leftrightarrow 6$ | $N_{\text{relay}} \ge 30$ | $N_{\text{relay}} < 27$ |

2.  **Minimum dwell $\tau_{\text{forest}} = 30\text{ s}$.** The source emits no `MANIFEST_UPDATE` that changes $M$ within $30\text{ s}$ of the previous one. The single exception is **flash-crowd growth**: an upward transition may pre-empt the dwell timer when $N_{\text{relay}}$ has reached at least twice the next threshold, since in that regime the source is the bottleneck and waiting 30 s is the more expensive error. Downward transitions never pre-empt the timer.

$M$ is computed from $N_{\text{relay}}$ directly rather than stepped, so a flash crowd from $N_{\text{relay}} = 2$ to $5{,}000$ moves to $M = 6$ in one transition; the ladder is a threshold table, not a walk.

### Uncovered Trees: The Coverage Grant

The ladder makes an empty tree *unlikely*; it cannot make it impossible, and at $N_{\text{relay}} < 12$ with $M = 2$ the probability that both relays land in the same tree is not small ($50\%$ at $N_{\text{relay}} = 2$, $12.5\%$ at $4$). Without a rule, the only parent for an uncovered tree is the source, at a cost of $B_m$ per viewer.

The **coverage grant** is a bounded, local, deterministic response that needs no publisher round-trip:

*   A joiner whose discovery for tree $m$ returns **no `RELAY`-class candidate assigned to $m$** other than the source (Ch2 §2.3.2 returns assignments in every record) may send `NEIGHBOR(TreeID = m, Priority = HIGH)` to a relay whose **second-ranked** tree is $m$ — a fact the joiner computes from the candidate's NodeID and $M$ using the rendezvous rule above.
*   The candidate **accepts the grant iff** it can hold at least one slot in each of two trees: $(1 - r_{\text{pull}})\,u_v \ge 2\,\Omega_v B_m$ for both trees. On accepting, it behaves as a $t_v = 2$ node (per-tree slots recomputed accordingly) for as long as it has children in the granted tree, and sets bit $m-1$ in its `AssignedTreeBitmap` so later joiners find it without a grant.
*   A relay holds at most **one** granted tree beyond its ranked assignment. If no candidate can accept, the source serves the tree directly — the last resort, not the first.

Because "second-ranked" is a deterministic function of NodeID, every joiner facing the same empty tree converges on the same small set of candidates, and the grant is a *coverage* mechanism, not a back door to multi-tree standing: it is capacity-gated, limited to one tree, and lapses when the granted tree empties.

### Shrinking the Forest

A shrink runs the §4.5 migration in reverse and is otherwise identical: the source publishes a `MANIFEST_UPDATE` with the smaller `num_trees`, the new `tree_mapping` and an `EffectiveSegmentSeq`; the relays of the closed tree — and only they — release their children with `DRAIN_NOTICE` (reason `REASSIGNED`, scope *all children*, §2.2 *The Drain Path*), keep serving them through the sibling election that notice triggers, and re-attach in their next-ranked tree; at the switch the closed tree stops receiving data.

Two consequences are worth stating explicitly, because they are the ones an implementation gets wrong:

*   **Per-tree bitrates change for everyone.** $B_m$ is read from the new mapping, and generally rises on a shrink (Tree 3 carries $0.75$ Mbps at $M = 6$ and $1.5$ Mbps at $M = 5$), so a relay's slot count $K_v(m)$ **drops** at the transition. Relays must shed children down to the new $K_v(m)$ *before* the switch, releasing them with `DRAIN_NOTICE` (reason `CAPACITY`, deadline `EffectiveSegmentSeq`) rather than dropping them, exactly as a `RELAY`→`LEAF` downgrade does (§5.3).
*   **Peers never derive the layer mapping.** A peer recomputes only its own relay assignment by rendezvous rank over $M_{\text{new}}$. Which layer each tree carries is read from the signed `tree_mapping` in the `MANIFEST_UPDATE` itself (Appendix D §D.4.8) — never inferred — so a shrink cannot leave two peers disagreeing about what tree $T_2$ contains.

## 1.5 Visualizing the 3-Node, 3-Slice Graph

Consider a swarm at the $M = 3$ rung. We have the Root Source ($S$) and three relay Peers ($P_1, P_2, P_3$) whose rendezvous ranks happen to place them in distinct trees:
*   $P_1$ is assigned to relay **Tree 1** ($L_0$, $1.5$ Mbps).
*   $P_2$ is assigned to relay **Tree 2** ($L_1$, $1.5$ Mbps).
*   $P_3$ is assigned to relay **Tree 3** ($L_2$, $3.0$ Mbps).

```text
       TREE 1 (L0, 1.5 Mbps)      TREE 2 (L1, 1.5 Mbps)      TREE 3 (L2, 3.0 Mbps)
       ---------------------      ---------------------      ---------------------
         [ Source ]                 [ Source ]                 [ Source ]
             |                          |                          |
       (Uploads L0)               (Uploads L1)               (Uploads L2)
             |                          |                          |
             v                          v                          v
        [ PEER 1 ]                 [ PEER 2 ]                 [ PEER 3 ]
       /          \               /          \               /          \
  (Relays L0) (Relays L0)    (Relays L1) (Relays L1)    (Relays L2) (Relays L2)
     /              \           /              \           /              \
    v                v         v                v         v                v
[PEER 2]          [PEER 3] [PEER 1]          [PEER 3] [PEER 1]          [PEER 2]
(Leaf)            (Leaf)   (Leaf)            (Leaf)   (Leaf)            (Leaf)
```

### Analysis of the Graph:
*   **Total Data Downloaded by $P_1$:** $L_0$ (from Source) + $L_1$ (from $P_2$) + $L_2$ (from $P_3$) = $1.5 + 1.5 + 3.0 = B$. It receives the full video stream.
*   **Total Data Uploaded by $P_1$:** It only uploads $L_0$ to $P_2$ and $P_3$. Upload volume = $2 \times 1.5 = 3$ Mbps (plus overhead $\Omega$). $P_3$, holding the $3.0$ Mbps tree, uploads $6$ Mbps for the same two children — which is why $P_3$ needs $(1 - r_{\text{pull}})\,u_v \ge 2 \cdot 3.0 \cdot \Omega \approx 6.9$ Mbps where $P_1$ needs half that. Per-tree bitrate, not $B/M$, is what a relay's slot count must be computed against.
*   By strictly isolating relay duties, we prevent $P_1$ from having to upload all three layers to both peers (which would require $2B$ of capacity).
