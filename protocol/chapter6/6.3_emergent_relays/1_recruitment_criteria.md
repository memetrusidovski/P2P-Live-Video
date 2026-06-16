# 1. Recruitment Criteria

For the $\sim 15\%$ of peers stuck behind dual Symmetric NATs or enterprise firewalls where direct hole punching mathematically fails, the protocol recruits **Emergent Relayers** from within the swarm. This replaces expensive central TURN servers with a crowd-sourced model.

A node is recruited as an Emergent Relayer if and only if it satisfies the following strict criteria:
1.  **Direct Accessibility:** Publicly accessible external IP address (verified via STUN as "No NAT" or "Full Cone").
2.  **Symmetrical Bandwidth:** Symmetrical upload capacity $u_{\text{relay}} \ge 20\text{ Mbps}$.
3.  **High Stability:** Rolling uptime $\ge 30\text{ minutes}$ and a packet drop rate $\le 0.5\%$.

```text
   [ Symmetric Peer A ] <---> [ Public Superpeer (Relay) ] <---> [ Symmetric Peer B ]
```
