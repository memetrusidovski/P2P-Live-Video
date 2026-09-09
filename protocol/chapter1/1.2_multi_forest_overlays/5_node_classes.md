# 5. Node Classes and Relay Eligibility

Not every device can be a relay. A phone on a metered cellular link, a set-top box with a thermal budget, and a privacy-conscious viewer behind a 3-hop onion circuit are all legitimate participants that cannot sustain interior-node duty. The Orthogonal Placement Rule (§1.3) assigns *every* node a relay tree by hash, which is correct for ordinary peers and wrong for these.

This document defines the node-class taxonomy that resolves this, without opening a free-riding hole in the incentive system.

## 5.1 The Three Classes

A node self-declares its class in the `NodeClass` field of `PROBE_RESPONSE` (0x0F), in `JOIN` (0x03), in `NEIGHBOR` handshakes, and in the child roster used for sibling election (§3.2).

| Class | Code | Relay duty | Proof-of-Work | Entitlement |
| :--- | :---: | :--- | :--- | :--- |
| **`RELAY`** | `0x00` | Orthogonal Placement applies: interior in its assigned tree(s), leaf elsewhere. Eligible for Deputy election and relay recruitment. | $C_1 = 16$, adaptive $C_2$ (Ch2 §2.2) | Full quality, ranked by Tit-for-Tat / PoU standing |
| **`LEAF`** | `0x01` | Leaf in **all** $M$ trees. Never elected Deputy, never recruited as an emergent relay. Self-declared by battery-, thermal-, or data-constrained devices. | $C_1 = 16$ (one-time), $C_2$ tier reduced by one step, plus the same-subnet reconnect discount | **Base layer guaranteed** by the universal service floor; enhancement layers only from surplus capacity, allocated after all contributing peers; joins $5$–$10\text{ s}$ behind the live edge |
| **`LEAF_PRIVATE`** | `0x02` | Leaf-only inherently — an onion-routed peer (Ch7 §7.3) has no inbound reachability to serve tree children. | as `LEAF` | as `LEAF`, plus the inherent $150$–$400\text{ ms}$ onion latency penalty. **May** earn Tit-for-Tat standing by serving `PULL_REQUEST`s through its circuit |

## 5.2 Proof-of-Work Is Universal

**No class is exempt from Proof-of-Work.** Earlier design notes proposed exempting mobile devices; that is superseded here, because PoW and upload contribution answer different questions:

*   PoW answers *"is this a distinct, costly-to-forge identity?"* — a Sybil defense (Ch2 §2.2) that every participant must satisfy, since a Sybil flood of leaf-declared identities would be exactly as damaging as one of relay-declared identities.
*   Tit-for-Tat and PoU answer *"is this peer contributing bandwidth?"* — a fairness mechanism, which is where device constraints legitimately apply.

Mobile PoW cost is instead addressed where it actually hurts, by difficulty tiering: leaf-class nodes solve the dynamic puzzle one tier below the swarm-size requirement, and same-subnet reconnects (WiFi↔cellular handover, DHCP renewal) get the halved-difficulty fast path of Ch2 §2.2. The static puzzle is paid once per identity, not per join.

## 5.3 Scoping the Orthogonal Placement Rule

The single-tree assignment $a = (\text{Blake3}(NodeID) \bmod M) + 1$ and the multi-tree extension of §1.3 apply **only to `RELAY`-class nodes**. A leaf-class node still computes its assignment, but the assignment lies **dormant**: it advertises $K_{\text{avail}} = 0$ in every tree and accepts no children.

This has a consequence for the capacity math: the relays available per tree are
$$\frac{N_{\text{relay}}}{M}, \quad \text{not} \quad \frac{N}{M}$$
where $N_{\text{relay}}$ counts only `RELAY`-class peers. The per-tree sustainability condition of Ch1 §1.1.5 must be evaluated over $N_{\text{relay}}$; a swarm that is majority-leaf needs proportionally higher upload from its relay population.

**Class upgrade and downgrade** are permitted at runtime and take effect at the next parent-selection cycle: a phone that is plugged in and joins WiFi may promote itself to `RELAY`, activating its dormant tree assignment; a laptop switching to battery may demote to `LEAF`, which drains its children through the standard sibling-election path (§3) rather than dropping them. A node that downgrades while holding children must continue serving them until they re-attach — abrupt self-demotion is treated as churn and penalized accordingly.

## 5.4 Why Leaf-Only Is Not Free-Riding

The protocol's core axiom is that network contribution and playback performance are inextricably linked. Leaf classes do not violate it, because their exemption from *upload* is paid for in *quality of service*:

*   **Quality ceiling.** Leaf nodes are entitled to the base layer ($L_0$) and receive enhancement layers only from genuine surplus — whenever capacity is contended (Ch1 §1.1.5), relay-class contributors are served first and leaf nodes fall back to 480p.
*   **Live-edge offset.** Leaf nodes join $5$–$10\text{ s}$ behind the live edge, drawing from well-replicated older segments rather than competing for the scarce live edge (this also relieves flash-crowd pressure).
*   **No promotion.** Leaf nodes cannot become Deputies, emergent relays (Ch6 §6.3), or Layer-1 children, so the lowest-latency positions in the forest remain reserved for contributors.

Entitlement therefore remains **monotone in contribution** — more upload always buys better playback — with a humanitarian floor at the base layer, which is what the heavily-FEC-protected Tree 1 of §4.2 was designed to provide. Privacy-mode peers are subject to exactly the same trade rather than being silently exempted (Ch7 §7.3).

## 5.5 Interaction with the Unchoker

The Tit-for-Tat unchoker (Ch5 §5.1) implements the service floor by reserving a bounded share of upload slots for base-layer delivery to zero-contribution peers, capped so contributors are never starved by a leaf influx: at most **20%** of a node's upload slots may be committed to the floor at any time. Above that share, leaf-class requests wait for genuine surplus.
