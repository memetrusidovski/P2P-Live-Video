# SOLUTION-001: Adaptive JOINING Threshold (Cold Start)

**Closes:** ISSUE-001 (Critical)
**Lives in:** `protocol/chapter1/1.3_peer_lifecycle/1_transition_model.md`, `2_algorithmic_core_loop.md`
**Class:** Cold-start correctness

---

## The problem in one line

The `JOINING → CONNECTING` transition required an Active Set of 4, but a swarm of $N$ peers can only offer $N-1$ neighbours — so every stream was structurally dead at $N < 5$, which is the state *every* stream starts in.

## The decision

$$\theta_{\text{join}} = \max(1, \min(4, N - 1))$$

with $N$ taken from the `swarm_size` field of the publisher's signed Stream Record, which the peer already holds from its DISCOVERY-phase `GET_PEERS` response. No extra round-trip.

## Why this shape and not another

Three alternatives were available, and the reasoning for rejecting them is the part worth keeping:

| Alternative | Why rejected |
| :--- | :--- |
| Lower the constant to 1 unconditionally | Throws away the eclipse resistance that a 4-peer view buys at real scale. The threshold is not arbitrary — it is half of $c_a = 8$, sized so a joining peer has several independent views before it commits. |
| Time-boxed fallback ("advance after 3 s regardless") | Converts a hard failure into a guaranteed 3-second startup penalty for the exact case — small streams — where the peer could have started instantly. Also fails the $\text{SJL} \le 1.5\text{ s}$ KPI (Ch8 §8.2). |
| Have the source special-case its first viewers | Reintroduces a coordinating authority into a protocol whose premise is that there isn't one. |

The adaptive form is the only one that is *exactly* the old behaviour at $N \ge 5$ and degrades continuously below it. That property — new rule ≡ old rule in the regime the old rule was designed for — is what makes it safe to adopt without re-validating the steady-state analysis.

## The non-obvious part: $\theta_{\text{join}}$ is a floor, not a target

The naive reading of the fix is "at $N=2$ a peer needs only one neighbour." That reading is wrong in a way that matters, because it turns a cold-start concession into a permanent membership view. Two rules were added to the spec alongside the formula:

1. The peer keeps running `FORWARD_JOIN` / `SHUFFLE` toward $c_a = 8$ after the transition. $\theta_{\text{join}}$ gates *progress*, not *growth*.
2. A peer that reaches `ACTIVE` with $|\mathcal{A}| < 2$ keeps a DHT re-query in flight until it has a second distinct parent.

## Residual risk (real-deployment)

$N$ is an **input the peer does not fully control**, and it now steers a security-relevant threshold. An adversary who can answer a victim's `GET_PEERS` and under-report $N$ drives $\theta_{\text{join}}$ to 1 and can attach the victim to a single attacker-chosen peer.

What bounds it today:

* `swarm_size` lives in the Stream Record signed by $SK_{\text{Publisher}}$ — forging it needs the publisher's key, not just a hostile guardian.
* The lookup runs over $\alpha = 3$ disjoint paths ($P_{\text{hijack}} \le f^\alpha$, Ch2 §2.1.2).
* Continued growth toward $c_a$ dilutes a single-peer view within seconds.

What is *not* bounded: a hostile guardian can still **withhold** or replay an older signed record with a genuinely smaller $N$. Replay is limited by the record's 1 s republish cadence and $\tau_{\text{ttl}} = 180\text{ s}$, but a stale-record attack narrowing $\theta_{\text{join}}$ during a rapid-growth phase is not fully closed. Candidate for the Chapter 8 adversarial suite: measure whether the mandatory second-parent rule closes it in practice, or whether the Stream Record needs a freshness nonce.

## Validation owed (Chapter 8)

* Startup Join Latency at $N = 2 \ldots 10$ against the $\text{SJL} \le 1.5\text{ s}$ target.
* Eclipse rate for peers that transitioned at $\theta_{\text{join}} = 1$, versus those that transitioned at 4.
