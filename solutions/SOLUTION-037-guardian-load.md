# SOLUTION-037: Bounding DHT Guardian Load

**Closes:** ISSUE-037 (High)
**Lives in:** `protocol/chapter2/2.3_stream_registration/2_store_get_rpcs.md` (*Who Is a Guardian*, *Sampled Registration*, *Query Discipline*); `protocol/chapter2/2.1_skademlia_routing/3_bootstrap_sequence.md`; `protocol/chapter7/7.2_ddos_shielding/1_token_bucket_math.md`, `2_xdp_kernel_ebpf.md`; `protocol/chapter3/3.3_churn_recovery/1_recovery_timeline.md` (re-query rule); `appendix_b_parameters.md`
**Class:** An $O(N)$ hotspot in a design whose whole purpose is to have none (recurring pattern #5)

---

## The problem in one line

Every peer registered with the same 20 guardians every 90 s; at $N = 10^6$ that is $11{,}000$ pre-session UDP packets per second per guardian — more than twice the guardian's own XDP new-connection ceiling, so its DDoS shield silently dropped over half of them — on a node the spec allowed to be a phone.

## The decision

*   **Sampled registration.** The publisher publishes $s$ = `RegisterSampleLog2`; a peer registers iff its NodeID falls in a $2^{-s}$ sample, with $s$ chosen to hold $\approx 10^4$ registrations at any $N$. The source, ingress relays and relay-capable nodes are exempt (the last at $s - 3$). Counts scale by $2^{s}$.
*   **Query discipline.** No unconditional periodic `GET_PEERS`; re-query only a thin per-tree pool, at most every 30 s per tree. Registration refresh is `REGISTER_PEER`.
*   **Guardian eligibility by construction.** A node enters a k-bucket only after answering an unsolicited `PING`; leaf-class and `SYMMETRIC` nodes do not answer DHT RPCs and run in client mode, so a guardian is always a reachable relay-class node.
*   **XDP per-source budget raised** from 10 to 50 pps (burst 100) for the CGNAT case; the global ceiling is unchanged and is now sufficient because the sample bounds unknown-source traffic.

## Why this and not the alternatives

*   **Sharding $K_s$ by $N$** spreads registrations over $S \cdot 20$ guardians but requires the publisher to `STORE_RECORD` to all of them every second — $\approx 2{,}000$ packets per second at $N = 10^6$ — and gives each shard's guardians only a partial count. Sampling keeps one guardian set, one write per second, and a count that scales trivially.
*   **Raising $\tau_{\text{ttl}}$ with $N$** cuts refresh load linearly but leaves the census, the state and the query load untouched, and makes departed peers linger longer in every count.
*   **A separate XDP budget for DHT frames** would need the XDP program to parse the frame type out of the UDP payload. Feasible, but unnecessary once the traffic is bounded at the source.

## Defects found during verification

*   The un-sampled arithmetic: $10^6 / 90 \approx 11{,}100$ registrations/s; $\times 2$ Ed25519 verifications $\approx 1$–$2$ cores; $\times 256$ B $\approx 23$ Mbps inbound; $\times 110$ B state $\approx 110$ MB. Sampled at $s = 7$: $\approx 90$/s, negligible CPU, $< 1$ MB.
*   The 30 s "thin passive set" re-query of SOLUTION-006 was unconditional at every $N$ where the passive set was thin — which at $M = 6$ with leaves is *always*, for per-tree pools. It is now per tree and the only periodic query.
*   The old `RATE_PPS_UNKNOWN = 10` throttled a CGNAT address to one registration or query every 100 ms per guardian — a regional launch behind one carrier pool would have admitted 10 peers per second.

## The generalisable lesson

**Anything every peer does on a timer toward a fixed set of nodes is an $O(N)$ hotspot; bound it by sampling or by making it conditional, and check it against the recipient's own rate limits.** The guardians' shield was tuned for attackers and turned out to be tuned against the swarm.

## Residual risk

*   A peer outside the sample is discoverable only through gossip. At the moment $s$ steps from $0$ to $1$ (around $N = 10^4$), half the registered peers stop refreshing and age out over 135 s; the guardians' view halves and the estimate $\text{count} \cdot 2^{s}$ is momentarily right, then briefly high, then right. Hysteresis on $s$ (step up at $2 N_0 2^{s}$, down at $N_0 2^{s} / 2$) is left to implementation.
*   `GET_PEERS` from a million joiners at 5-minute sessions is still $\approx 500$ per second per guardian — bounded, but not small on a home connection.

## Validation owed (Chapter 8)

*   Guardian inbound pps and CPU at $N = 10^4, 10^5, 10^6$ under the sample, against `GLOBAL_NEW_PPS`.
*   Estimator error of $\text{count} \cdot 2^{s}$ across the $s$ step boundaries.
*   Join success rate when the joiner's own registration is outside the sample (it should be unaffected; verify).
