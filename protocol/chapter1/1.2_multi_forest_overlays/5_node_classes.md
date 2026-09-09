# 5. Node Classes and Relay Eligibility

Not every device can be a relay. A phone on a metered cellular link, a set-top box with a thermal budget, and a privacy-conscious viewer behind a 3-hop onion circuit are all legitimate participants that cannot sustain interior-node duty. The Orthogonal Placement Rule (§1.3) assigns *every* node a relay tree by rendezvous rank, which is correct for ordinary peers and wrong for these.

This document defines the node-class taxonomy that resolves this, without opening a free-riding hole in the incentive system.

## 5.1 The Three Classes

A node self-declares its class in the `NodeClass` field of `REGISTER_PEER` (0x0A), `PROBE_RESPONSE` (0x0F), `JOIN` (0x03), in `NEIGHBOR` handshakes, and in the `ROSTER` used for sibling election (§3.2); it travels with every Peer Record (Ch2 §2.3.3). A change of class is re-registered immediately.

| Class | Code | Relay duty | Proof-of-Work | Entitlement |
| :--- | :---: | :--- | :--- | :--- |
| **`RELAY`** | `0x00` | Orthogonal Placement applies: interior in its assigned tree(s), leaf elsewhere. Eligible for Deputy election and relay recruitment. | $C_1 = 16$, adaptive $C_2$ (Ch2 §2.2) | Full quality, ranked by Tit-for-Tat / PoU standing |
| **`LEAF`** | `0x01` | Leaf in **all** $M$ trees. Never elected Deputy, never recruited as an emergent relay. Self-declared by battery-, thermal-, or data-constrained devices. | $C_1 = 16$ (one-time), $C_2$ tier reduced by one step, plus the same-subnet reconnect discount | **Base layer guaranteed** by the universal service floor; enhancement layers only from surplus capacity — a leaf is the first child preempted when an enhancement tree is contended (Ch5 §5.1) |
| **`LEAF_PRIVATE`** | `0x02` | Leaf-only inherently — an onion-routed peer (Ch7 §7.3) has no inbound reachability to serve tree children. | as `LEAF` | as `LEAF`, plus the inherent $150$–$400\text{ ms}$ onion latency penalty. **May** earn Tit-for-Tat standing by serving `PULL_REQUEST`s through its circuit |

Leaf-class peers are **tree children like every other subscriber**: they receive the base layer by push at the live edge and anchor at the same $\Delta_{\text{buffer}}$ as relays (Ch4 §4.3.1). An earlier draft had them join $5$–$10$ s behind the edge, "drawing from older, well-replicated segments"; that described a pull-served delivery path the protocol does not define, and a tree parent cannot push ten-second-old data in any case. The leaf's price is quality and standing, not latency.

### Reachability Is a Class Input

Relay duty means accepting inbound tree joins. A node behind a symmetric NAT or CGNAT cannot be reached by a joiner it has not first contacted, and the hole-punch success rate against it from the common port-restricted home router is $\approx 10\%$ (Ch6 §6.1.1). Such a node advertising $K_v > 0$ is a relay nobody can use: every `PROBE` to it times out (200 ms per tree per round), and every slot it advertises is a slot the forest counts but cannot fill. Class is therefore gated on the **`Reachability`** a node measures during bootstrap (Ch2 §2.1.3) and carries in its `Flags`:

| `Reachability` | NAT class (Ch6 §6.1.1) | May declare `RELAY`? |
| :--- | :--- | :--- |
| `PUBLIC` | none / full cone | yes — probed directly |
| `CONE` | restricted / port-restricted | yes — probed after a `PUNCH_REQUEST` rendezvous (App D §D.4.14), one extra RTT |
| `SYMMETRIC` | symmetric / CGNAT | **no** — defaults to `LEAF`; may still earn Tit-for-Tat standing by serving `PULL_REQUEST`s over the connections *it* initiates, exactly as `LEAF_PRIVATE` does |

A `SYMMETRIC` node loses nothing as a *child*: it initiates every parent connection outbound, and `PUBLIC` parents accept that without ceremony. Where it cannot reach a `CONE` parent it needs, or wants to contribute upload, an emergent relay bridges it (Ch6 §6.3). Reachability is re-evaluated on every IP change and re-registered.

## 5.2 Proof-of-Work Is Universal

**No class is exempt from Proof-of-Work.** Earlier design notes proposed exempting mobile devices; that is superseded here, because PoW and upload contribution answer different questions:

*   PoW answers *"is this a distinct, costly-to-forge identity?"* — a Sybil defense (Ch2 §2.2) that every participant must satisfy, since a Sybil flood of leaf-declared identities would be exactly as damaging as one of relay-declared identities.
*   Tit-for-Tat and PoU answer *"is this peer contributing bandwidth?"* — a fairness mechanism, which is where device constraints legitimately apply.

Mobile PoW cost is instead addressed where it actually hurts, by difficulty tiering: leaf-class nodes solve the dynamic puzzle one tier below the swarm-size requirement, and same-subnet reconnects (WiFi↔cellular handover, DHCP renewal) get the halved-difficulty fast path of Ch2 §2.2. The static puzzle is paid once per identity, not per join.

## 5.3 Scoping the Orthogonal Placement Rule

The rendezvous tree assignment $a(v) = \arg\max_m \text{Blake3}(NodeID_v \parallel m)$ and the multi-tree extension of §1.3 apply **only to `RELAY`-class nodes**. A leaf-class node still computes its assignment, but the assignment lies **dormant**: it advertises $K_{\text{avail}} = 0$ in every tree and accepts no children.

This has a consequence for the capacity math: the relays available per tree are
$$\frac{N_{\text{relay}}}{M}, \quad \text{not} \quad \frac{N}{M}$$
where $N_{\text{relay}}$ counts only `RELAY`-class peers. The per-tree sustainability condition of Ch1 §1.1.5 must be evaluated over $N_{\text{relay}}$ (it carries the factor $1/(1-\ell)$ for exactly this reason), and the forest ladder of §1.4 is keyed on $N_{\text{relay}}$ rather than $N$; a swarm that is majority-leaf needs proportionally higher upload from its relay population.

### How Many Leaves the Swarm Can Absorb

`NodeClass` is **self-declared**, and for a viewer who genuinely wants only 480p and does not mind being 10 s behind, declaring `LEAF` costs nothing they value. The QoS pricing of §5.4 is a real disincentive but not a hard limit, so the honest question is not "will peers game this?" but "what leaf fraction does the floor guarantee actually survive?"

Let $\ell$ be the leaf fraction of the swarm. Leaf demand is $\ell N B_1$ (base layer only); the supply reserved for it is the service floor — at most $20\%$ of the relay population's upload, $0.20 (1-\ell) N \bar{u}_{\text{relay}}$. The guarantee holds while

$$0.20\,(1-\ell)\,\bar{u}_{\text{relay}} \ge \ell B_1 \quad \Longleftrightarrow \quad \ell \le \frac{0.20\,\bar{u}_{\text{relay}}}{0.20\,\bar{u}_{\text{relay}} + B_1}$$

At the reference values ($\bar{u}_{\text{relay}} = 8$ Mbps, $B_1 = 1.5$ Mbps) that is $\ell \le 51.6\%$ — the swarm tolerates a **majority-leaf population** before the base-layer floor is contended, which is a comfortable margin for a class intended for phones and privacy peers.

Beyond that point the floor is oversubscribed and leaf-class peers begin to miss even the base layer. The response is **not** to weaken the guarantee silently: the source's Tree-1 reserve (Ch1 §1.1.5 §5.4) absorbs the overflow first, and a publisher whose swarm is persistently past the bound should raise the floor cap — the relation above gives the cap needed for a target $\ell$ ($25\%$ buys $57\%$, $30\%$ buys $62\%$). What must not happen is for the floor to be quietly unmet while the spec still calls it a guarantee.

**Class upgrade and downgrade** are permitted at runtime, are re-registered with the DHT immediately (Ch2 §2.3.2), and take effect at the next parent-selection cycle: a phone that is plugged in and joins WiFi may promote itself to `RELAY`, activating its dormant tree assignment; a laptop switching to battery may demote to `LEAF`, which drains its children through the standard sibling-election path (§3) rather than dropping them.

A demoting node must keep serving its children for a bounded **drain window** $\tau_{\text{drain}} = 5\text{ s}$ — the same duration as the forest-resize migration window (§1.2.4) — or until every child has re-attached, whichever comes first. The bound matters in both directions: an unbounded obligation is unsatisfiable for the very devices the class exists to accommodate (a laptop at 2% battery cannot promise to serve indefinitely), while no obligation at all makes demotion a free way to dump children. Announcing the demotion and serving out the window is compliant; dropping children without it is treated as churn and penalized accordingly.

## 5.4 Why Leaf-Only Is Not Free-Riding

The protocol's core axiom is that network contribution and playback performance are inextricably linked. Leaf classes do not violate it, because their exemption from *upload* is paid for in *quality of service*:

*   **Quality ceiling.** Leaf nodes are entitled to the base layer ($L_0$) and receive enhancement layers only from genuine surplus — whenever capacity is contended (Ch1 §1.1.5), relay-class contributors are served first and leaf nodes fall back to 480p.
*   **Last in, first out under contention.** A leaf's contribution rank is zero, so when an enhancement tree is full it is the child a parent preempts for a contributing joiner (Ch5 §5.1) — it holds enhancement layers only while there is genuine surplus.
*   **No promotion.** Leaf nodes cannot become Deputies, emergent relays (Ch6 §6.3), or Layer-1 children, so the lowest-latency positions in the forest remain reserved for contributors.

Entitlement therefore remains **monotone in contribution** — more upload always buys better playback — with a humanitarian floor at the base layer, which is what the heavily-FEC-protected Tree 1 of §4.2 was designed to provide. Privacy-mode peers are subject to exactly the same trade rather than being silently exempted (Ch7 §7.3).

## 5.5 Where the Floor Is Enforced

The service floor is a rule on **tree admission in the base-layer trees** (Ch1 §1.2.2 *Rank Admission Rule*): $L_0$ slots are never rank-preempted, and a relay must keep at least **20%** of its $L_0$ slots available to leaf-class children before it may refuse one. Above that share, leaf-class requests wait for genuine surplus. The Tit-for-Tat unchoker (Ch5 §5.1) governs PULL service only; its contribution to the floor is the optimistic slots through which a leaf, which can never reciprocate, still obtains repair of lost base-layer blocks.
