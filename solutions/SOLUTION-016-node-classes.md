# SOLUTION-016: Node Classes (RELAY / LEAF / LEAF_PRIVATE)

**Closes:** ISSUE-016 (High)
**Lives in:** `protocol/chapter1/1.2_multi_forest_overlays/5_node_classes.md` (new), `1_graph_theory_and_slicing.md`, `4_stream_slicing_architecture.md`, `protocol/chapter2/2.2_crypto_node_id/1_static_dynamic_puzzles.md`, `protocol/chapter5/5.1_tit_for_tat/3_optimistic_exploration.md`, `protocol/chapter7/7.3_privacy_routing/2_onion_latency_tradeoffs.md`, `appendix_b_parameters.md`
**Class:** Incentive integrity / device reality

---

## The problem in one line

Four rules contradicted each other: exploratory notes exempted mobiles from PoW, Ch2 required PoW from everyone, the Orthogonal Placement Rule assigned *every* node relay duty with no opt-out, and Ch7 exempted onion-routed peers from relay obligations outright — a sanctioned free-ride in a protocol whose foreword says contribution and playback performance are inextricably linked.

## The decision

A three-class taxonomy carried in `PROBE_RESPONSE`, `JOIN`, `NEIGHBOR`, and the child roster:

| Class | Relay duty | PoW | Entitlement |
| :--- | :--- | :--- | :--- |
| `RELAY` `0x00` | Orthogonal Placement applies | $C_1{=}16$, adaptive $C_2$ | Full quality by TFT rank |
| `LEAF` `0x01` | Leaf in all $M$ trees, never Deputy | $C_1{=}16$ once, $C_2$ one rung lower | Base layer by right; enhancement from surplus; joins 5–10 s behind live edge |
| `LEAF_PRIVATE` `0x02` | Leaf-only inherently (no inbound reachability) | as `LEAF` | as `LEAF`, plus 150–400 ms onion penalty; **may** earn TFT credit serving PULLs through its circuit |

## The distinction that resolves the contradiction

**PoW and Tit-for-Tat answer different questions, and conflating them was the whole bug.**

* PoW asks *"is this a distinct, costly-to-forge identity?"* — a Sybil defense. A flood of leaf-declared Sybils is exactly as damaging as relay-declared ones, so **no class is exempt**. The exploratory note's mobile exemption is superseded.
* TFT/PoU asks *"is this peer contributing bandwidth?"* — a fairness mechanism, and *this* is where device constraints legitimately apply.

Once separated, the mobile-cost concern is addressed where it actually hurts — the *dynamic* puzzle, re-solved on every IP change — via difficulty tiering, not exemption.

**Leaf-only is priced, not exempted.** The exemption is paid in QoS: base layer only, enhancement from genuine surplus, 5–10 s behind the live edge, never promoted to Deputy or emergent relay. Entitlement stays monotone in contribution — more upload always buys better playback — with a humanitarian floor at the base layer, which is precisely what the heavily-FEC-protected Tree 1 was designed to provide.

The placement rule is scoped to `RELAY`; leaves compute their hash assignment but hold it **dormant**, which makes class upgrade instant when a phone is plugged in.

## Addition: how many leaves the swarm can actually absorb

`NodeClass` is self-declared, and QoS pricing is a disincentive, not a limit — for a viewer who genuinely wants 480p and does not mind being 10 s behind, declaring `LEAF` costs nothing they value. The spec asserted the base-layer floor as a guarantee without ever checking at what leaf fraction it breaks.

Deriving it: leaf demand is $\ell N B_1$; floor supply is $20\%$ of relay upload, $0.20(1-\ell)N\bar{u}_{\text{relay}}$. So

$$\ell \le \frac{0.20\,\bar{u}_{\text{relay}}}{0.20\,\bar{u}_{\text{relay}} + B_1} = 51.6\% \quad \text{at reference values}$$

The swarm tolerates a **majority-leaf population** — a genuinely comfortable margin, and worth knowing rather than hoping. It also gives the publisher a lever: $25\%$ floor cap buys $57\%$, $30\%$ buys $62\%$. The spec now states the bound and says explicitly that the failure response is the source reserve plus raising the cap — never quietly leaving the floor unmet while still calling it a guarantee.

## Addition: the drain window

"A node that downgrades while holding children must continue serving them until they re-attach" is unsatisfiable for exactly the devices the class exists for — a laptop at 2% battery cannot promise indefinite service. But no obligation makes demotion a free way to dump children.

Bounded at $\tau_{\text{drain}} = 5\text{ s}$ (matching the forest-resize migration window), or until all children re-attach, whichever is sooner. Announce and serve out the window is compliant; dropping without it is churn.

## The generalisable lesson

**When a policy exempts a group, check whether the exemption is from a *defense* or from an *obligation*.** The mobile-PoW exemption looked like relief from an obligation (uploading) and was actually a hole in a defense (identity cost). They were bundled because both are "costs to the peer", but only one is negotiable.

Related: **an entitlement stated as a guarantee needs a derivation showing the regime in which it holds.** "Capped at 20% of upload slots" sounded like a safety bound and was really an unexamined supply limit against unbounded demand.

## Validation owed (Chapter 8)

* Observed leaf fraction under a realistic device mix, against the 51.6% bound.
* Whether the QoS price (480p + 10 s delay) actually deters opportunistic `LEAF` declaration, or whether a meaningful share of capable nodes declare leaf anyway.
* Whether `LEAF_PRIVATE` peers can earn meaningful TFT standing by serving PULLs through an onion circuit, or whether the latency makes them uncompetitive in practice.
