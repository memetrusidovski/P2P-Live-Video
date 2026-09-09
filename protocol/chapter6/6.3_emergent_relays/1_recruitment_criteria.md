# 1. Recruitment Criteria

For the $\sim 15\%$ of peers stuck behind dual Symmetric NATs or enterprise firewalls where direct hole punching mathematically fails, the protocol recruits **Emergent Relayers** from within the swarm. This replaces expensive central TURN servers with a crowd-sourced model.

A node is recruited as an Emergent Relayer if and only if it satisfies the following strict criteria:
1.  **Direct Accessibility:** `Reachability = PUBLIC` (Ch2 §2.1.3) — no NAT or full cone.
2.  **Symmetrical Bandwidth:** Symmetrical upload capacity $u_{\text{relay}} \ge 20\text{ Mbps}$.
3.  **High Stability:** Rolling uptime $\ge 30\text{ minutes}$ and a packet drop rate $\le 0.5\%$.

```text
   [ Symmetric Peer A ] <---> [ Public Superpeer (Relay) ] <---> [ Cone Parent B ]
```

**What a relay bridges, and what it does not.** A `SYMMETRIC` peer is leaf-class (Ch1 §1.2.5) and connects *outbound* to `PUBLIC` parents without help; the relay exists for the cases that remain — a needed tree whose only reachable parents are `CONE`, and the leaf's own PULL contribution, which a peer nobody can reach inbound could not otherwise make. The relay bridges media in both directions between A and B: B's tree push to A, and A's `PULL_REQUEST` service to peers that ask it. The protocol has no "relayed relay": a NAT-blocked node never holds tree children of its own through a relay, so no slot accounting has to be split across two machines.

**Advertisement and discovery.** A qualifying node sets `Flags.RELAY_CAPABLE` in its registration (it is exempt from most of the registration sampling, Ch2 §2.3.2) and in every Peer Record about it. A peer needing a relay issues `GET_PEERS` with `ReqFlags.WANT_RELAY_CAPABLE` and receives a random sample of them; it then sends `RELAY_PROPOSAL` (§6.3.3).

**Cost accounting.** Bridged traffic is upload the relay spends outside its own tree duties, so a bind is charged to the relay's **PULL reserve** $r_{\text{pull}} u_v$ (Ch1 §1.2.1) — the relay accepts a `RELAY_PROPOSAL` only while its reserve has $B_m \Omega$ of headroom for the tree being bridged, and never touches its tree slots $K_v(m)$. This is what keeps relaying from silently eating the forest's capacity: at $15\%$ of $10^6$ peers behind symmetric NAT, only the fraction that cannot find a `PUBLIC` parent needs a bridge, and each bridge is bounded by a reserve that is already budgeted.
