# SOLUTION-010: Collusion Detection — Small-Swarm Guard and Degree Weighting

**Closes:** ISSUE-010 (Medium)
**Lives in:** `protocol/chapter5/5.3_reputation_auditing/1_graph_collusion_auditing.md`, `2_ip_subnet_penalties.md`, `appendix_b_parameters.md`
**Class:** Reputation-layer false positives / detector soundness

---

## The problem in one line

The collusion detector flags symmetric, low-degree PoU edges — but at $N = 5$ the Multi-Forest placement rule *produces* symmetric low-degree edges from entirely legitimate behaviour (A relays slice 3 to B while B relays slice 1 to A), and at $N = 2$ the source–viewer pair is the only edge in the graph, so gossip consensus banned legitimate viewers.

## The decision

Two gates:

1. Suppress the detector (and the /24 subnet penalty) entirely below $N_{\text{collusion}} = 20$.
2. Scale symmetry by counterparty diversity **measured against opportunity**:

$$\text{confidence} = \text{symmetry\_score} \times \left(1 - \frac{\text{Degree}(A)}{\text{Opportunity}(A)}\right), \quad \text{Opportunity}(A) = \min(N_{\text{guard}} - 1,\ c_a + c_p)$$

## Defect found: the degree weighting created an immunity hole

The applied fix used $1 - 1/\text{Degree}(A)$, reasoning that "a node with a single available peer has no choice but to interact with it."

The reasoning is right at $N = 5$ and **wrong at $N = 10^6$**, where a node with one counterparty did not lack choice — it made one. The formula does not know the difference, so it sets confidence to zero at $\text{Degree} = 1$ at *every* scale.

That is not a missed detection; it is an **optimal attacker strategy**. A colluding pair's cheapest configuration is exactly degree 1 — each Sybil trades receipts only with its partner — and the rule grants that configuration permanent, unconditional immunity. The fix for a false-positive problem introduced a false-negative hole aimed precisely at the attack the detector exists to catch.

The correction is to judge degree **relative to what was available**:

| Scenario | $N$ | Degree | Opportunity | Old | New |
| :--- | :---: | :---: | :---: | :---: | :---: |
| Source–viewer pair | 2 | 1 | 1 | 0 | 0 ✓ |
| **Colluding pair at scale** | $10^6$ | 1 | 40 | **0** ✗ | **0.98** ✓ |
| Honest relay | $10^6$ | 40 | 40 | 0.98 | 0 ✓ |

This keeps the genuine small-swarm insight — $\text{Opportunity} = 1 \Rightarrow$ confidence 0 — without extending it to a regime where it is false.

Worth noting what carries most of the safety: **symmetry is the primary gate.** Ordinary parent→child flow is overwhelmingly unidirectional, so $W(A\to B)/W(B\to A)$ is nowhere near 1 and the score is 0 regardless of degree. Degree weighting only refines cases that already look symmetric.

## Second defect: a security control keyed on an attacker-influenced input

The guard disables collusion detection and subnet penalties when $N < 20$, and $N$ came from the DHT Stream Record. An adversary answering a victim's `GET_PEERS` with a **stale but validly signed** record from early in the broadcast — when $N$ genuinely was below 20 — switches the detector off. The publisher's signature does not help: the record was authentic, just old.

The fix uses a bound the attacker cannot touch. Every peer knows how many distinct peers *it has actually seen* across its Active Set, Passive Set and gossip history, and no third party can shrink that number:

$$N_{\text{guard}} = \max\left(N_{\text{record}},\ |\text{distinct peers observed locally}|\right)$$

The detector can now only be disabled by a peer that has genuinely seen almost nobody — which is the condition the guard was meant to describe in the first place.

## The generalisable lesson

Two, both worth carrying forward:

1. **A rule derived from a small-$N$ intuition must be checked at large $N$ before being written as unconditional.** The "no choice but to interact" argument is sound *at small $N$* and became an attack surface the moment it was applied everywhere. Prefer expressions that are relative to the regime (degree/opportunity) over absolutes (degree).

2. **Any input that can disable a security control must be locally verifiable, or bounded by something that is.** `swarm_size` from the Stream Record is fine for tuning $\theta_{\text{join}}$ or the PoW tier — being wrong there costs efficiency. Using it to gate a detector makes it an attack surface. The same value now enters those two classes of decision differently, and that distinction should be applied wherever else `swarm_size` is read.

## Validation owed (Chapter 8)

* False-positive rate at $N = 20$–$100$ under legitimate mutual-relay topologies — the regime just above the guard, where the detector is newly live and structural symmetry has not fully disappeared.
* Detection rate against a colluding pair that deliberately holds degree at 1, confirming the immunity hole is actually closed.
* Whether $N_{\text{collusion}} = 20$ is the right threshold once degree/opportunity weighting is doing most of the work — it may now be safe to lower it.
