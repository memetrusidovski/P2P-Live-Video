# SOLUTION-051: `CONE` Registrants Keep Their Guardian Mappings Alive

**Closes:** ISSUE-051 (Medium)
**Lives in:** `protocol/chapter2/2.3_stream_registration/2_store_get_rpcs.md` (registration step 5); `appendix_d_frame_registry.md` §D.4.14; `appendix_b_parameters.md` ($\tau_{\text{nat}}$)
**Class:** An external assumption (NAT mapping lifetime) never stated, on which a control path silently depended

---

## The problem in one line

A guardian forwards `PUNCH_REQUEST`s to a `CONE` relay through the UDP mapping the relay's last registration opened up to 90 s earlier; many consumer and carrier NATs expire idle UDP mappings in 30–60 s, so the forward was dropped for most of every refresh period, the joiner's probe timed out, and every DHT-discovered `CONE` relay was a dead candidate — with nothing marking it as such.

## The decision

A `CONE` registrant sends a `PING` to each guardian it registered with every $\tau_{\text{nat}} = 25$ s. A joiner whose probe times out after a guardian-forwarded punch retries once via a gossip referrer if it has one, else marks the record unpunchable locally for one registration period. The NAT-timeout assumption is stated where the guardian is a referrer: RFC 4787 asks for $\ge 2$ min; deployed NATs commonly use 30–60 s (an external fact, flagged as such).

## Why this and not the alternatives

*   **Route the forward through a node with an open session to the target** (its tree parent, an active-set neighbour): the target's Peer Record would need a "punch via" pointer that changes at every migration, and the guardian would need to know it — a second registration field refreshed on every topology change.
*   **Shorten the registration refresh to 25 s**: four times the registration load at every guardian for a path only `CONE` peers need.

## Defects found during verification

*   Cost at $N = 10^6$, $s = 7$: $\approx 7{,}800$ sampled registrants $\times \approx 0.6$ `CONE` $\times 20$ guardians $/ 25$ s $\approx 3{,}700$ pps swarm-wide, under 200 pps per guardian — inside the unknown-source budget of Ch7 §7.2.1 and a fraction of the registration and query load SOLUTION-037 already bounded.
*   Out-of-sample peers are referred by gossip neighbours, which hold an open session; that path was sound and is unchanged.
*   `CONE` relays were counted in $N_{\text{relay}}$ and drove the ladder upward while unreachable through the DHT; with the keepalive the count and the reachability agree.

## The generalisable lesson

**"It can reach the target because it holds the registration" is a claim about a NAT's state table; state the mapping lifetime the claim needs and put a keepalive under it.**

## Residual risk

NATs with sub-25 s UDP timeouts exist; a `CONE` relay behind one is unreachable through the DHT path and, if it is also unreachable through gossip, its reliability score falls and it is abandoned — the correct slow correction, at the cost of a wasted probe per joiner until then.

## Validation owed (Chapter 8)

*   Punch success rate through the guardian path against measured NAT timeout distributions, with $\tau_{\text{nat}} \in \{15, 25, 45\}$ s.
