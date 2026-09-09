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

Let $\ell$ be the leaf fraction of the swarm. The binding constraint is the per-tree sustainability condition of Ch1 §1.1.5 §5.1 applied to the trees carrying $L_0$ — **every** peer, leaf or relay, needs every $L_0$ stripe, and only the $t_0 / M$ of relays assigned to those trees supply them:

$$\bar{u}_{\text{relay}} \ \ge\ \frac{1}{1 - \ell} \cdot \frac{M \cdot B_0 \cdot \Omega}{1 - r_{\text{pull}}}
\quad\Longleftrightarrow\quad
\ell \ \le\ 1 - \frac{M \cdot B_0 \cdot \Omega}{(1 - r_{\text{pull}})\, \bar{u}_{\text{relay}}}$$

where $B_0 = b_0 / t_0$ is the per-tree bitrate of an $L_0$ stripe under the current mapping (§4.2.1). On the reference ladder at $\Omega = 1.15$:

| $\bar{u}_{\text{relay}}$ | $M{=}6$, $(2,2,2)$: $B_0 = 0.75$ | $M{=}6$ after folding $L_2$, $(3,3)$: $B_0 = 0.5$ | $M{=}6$, $L_0$ alone: $B_0 = 0.25$ |
| :---: | :---: | :---: | :---: |
| $6$ Mbps | $\ell \le 4\%$ | $\le 36\%$ | $\le 68\%$ |
| $8$ Mbps | $\ell \le 28\%$ | $\le 52\%$ | $\le 76\%$ |
| $10$ Mbps | $\ell \le 42\%$ | $\le 62\%$ | $\le 81\%$ |
| $12$ Mbps | $\ell \le 52\%$ | $\le 68\%$ | $\le 84\%$ |

(Slot rounding makes the first column slightly tighter in practice: at $8$ Mbps a relay holds $\lfloor 7.2 / 0.8625 \rfloor = 8$ slots per $0.75$ Mbps stripe, so supply is $2 \cdot \frac{(1-\ell)N}{6} \cdot 8 = 2.67\,(1-\ell)N$ stripe-slots against $2N$ needed, i.e. $\ell \le 25\%$.)

An earlier draft derived $\ell \le 51.6\%$ at $8$ Mbps by treating $20\%$ of *all* relay upload as base-layer supply for leaves alone. Three of its terms were wrong for the mapping this specification defines: only the relays of $L_0$ trees hold $L_0$ slots ($2/6$ at $M = 6$, not all of them); a relay delivers $(1 - r_{\text{pull}}) \bar{u} / \Omega \approx 0.78\,\bar{u}$ of media, not $\bar{u}$; and the $20\%$ is the share of $L_0$ slots *reserved for leaves*, not the supply leaves draw on — relays need the other $80\%$ and leaves also take unreserved slots. The honest figure for a phone-heavy audience at $8$ Mbps relays is a quarter to a third of the swarm, not a majority.

Beyond the bound the base layer is oversubscribed and *some* peers — leaf or relay — miss it. The response is **not** to weaken the guarantee silently, and it is **not** to move the floor cap: raising the leaf share from $20\%$ to $30\%$ changes which peers go without $L_0$, not whether they do, because it creates no slots. The lever that works is the **mapping**: folding the top layer moves the relays of its trees onto finer $L_0$ stripes (second and third columns above), and the publisher-side fold rule of Ch1 §1.1.5 §5.6 fires on exactly the starvation signal an oversubscribed base layer produces. The source reserve (Ch1 §1.1.5 §5.4) absorbs what no mapping can carry. What must not happen is for the floor to be quietly unmet while the spec still calls it a guarantee.

**Class upgrade and downgrade** are permitted at runtime, are re-registered with the DHT immediately (Ch2 §2.3.2), and take effect at the next parent-selection cycle: a phone that is plugged in and joins WiFi may promote itself to `RELAY`, activating its dormant tree assignment; a laptop switching to battery may demote to `LEAF`, which releases its children through the drain path (§2.2, `DRAIN_NOTICE` reason `DEMOTED`) and the sibling election it triggers (§3) rather than dropping them.

A demoting node must keep serving its children for a bounded **drain window** $\tau_{\text{drain}} = 5\text{ s}$ — the same duration as the forest-resize migration window (§1.2.4) — or until every child has re-attached, whichever comes first. It announces the demotion with a `DRAIN_NOTICE` (reason `DEMOTED`, scope *all children*, Appendix D §D.4.19) on every tree stream it serves, which is what lets its children re-select while it is still delivering (§1.2.2 *The Drain Path*). The bound matters in both directions: an unbounded obligation is unsatisfiable for the very devices the class exists to accommodate (a laptop at 2% battery cannot promise to serve indefinitely), while no obligation at all makes demotion a free way to dump children. Sending the notice and serving out the window is compliant; dropping children without it is treated as churn and penalized accordingly.

## 5.4 Why Leaf-Only Is Not Free-Riding

The protocol's core axiom is that network contribution and playback performance are inextricably linked. Leaf classes do not violate it, because their exemption from *upload* is paid for in *quality of service*:

*   **Quality ceiling.** Leaf nodes are entitled to the base layer ($L_0$) and receive enhancement layers only from genuine surplus — whenever capacity is contended (Ch1 §1.1.5), relay-class contributors are served first and leaf nodes fall back to 480p.
*   **Last in, first out under contention.** A leaf's contribution rank is zero, so when an enhancement tree is full it is the child a parent preempts for a contributing joiner (Ch5 §5.1) — it holds enhancement layers only while there is genuine surplus.
*   **No promotion.** Leaf nodes cannot become Deputies, emergent relays (Ch6 §6.3), or Layer-1 children, so the lowest-latency positions in the forest remain reserved for contributors.

Entitlement therefore remains **monotone in contribution** — more upload always buys better playback — with a humanitarian floor at the base layer, which is what the heavily-FEC-protected Tree 1 of §4.2 was designed to provide. Privacy-mode peers are subject to exactly the same trade rather than being silently exempted (Ch7 §7.3).

## 5.5 Where the Floor Is Enforced

The service floor is a rule on **tree admission in the base-layer trees** (Ch1 §1.2.2 *Rank Admission Rule*). $L_0$ slots are never rank-preempted. Each relay's $L_0$-tree slots carry a **leaf share** $R_{\text{leaf}}(m) = \lceil 0.2 \cdot K_v(m) \rceil$ — at least one slot at every $K_v(m) \ge 1$, two from six slots, and so on — which is enforced by **displacement, not by idle reservation**: while leaf-class children hold fewer than $R_{\text{leaf}}(m)$ of the relay's slots in tree $m$ and no slot is free, a leaf-class request displaces the relay's lowest-ranked relay-class child that is a *pure subscriber* of $m$ (it does not relay $m$, so it has no children in $m$ and displacing it orphans nobody). A child that relays $m$ is never displaced. Once leaves hold the leaf share, further leaf-class requests wait for a free slot.

An earlier draft stated the floor two ways at once — as slots a relay must "keep available", and as a priority applied only while a leaf's request was pending. The first idles $20\%$ of every base-layer relay in a swarm with no phones in it and, at the one- and two-slot relays a cold-start swarm consists of, forbids them any relay child at all; the second is a floor that a relay filled by relays never honours. Displacement gives the guarantee of the first at the cost of the second: no capacity is held empty, a leaf is served whenever it asks and the share is not yet full, the displaced relay pays one re-attach through the drain path (Ch1 §1.2.2, reason `DISPLACED`), and relays are guaranteed the other $80\%$.

The Tit-for-Tat unchoker (Ch5 §5.1) governs PULL service only; its contribution to the floor is the optimistic slots through which a leaf, which can never reciprocate, still obtains repair of lost base-layer blocks.
