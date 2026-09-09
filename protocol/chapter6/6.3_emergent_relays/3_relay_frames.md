# 3. Relay Frame Layouts

```text
RELAY_PROPOSAL Frame (Type 0x30):
Sent by a NAT-blocked node to a RELAY_CAPABLE superpeer (found via GET_PEERS with
ReqFlags.WANT_RELAY_CAPABLE, Ch2 §2.3.2) to request a bridge to Target for tree TreeID.
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|   Version     |  Type (0x30)  |          Payload Length       |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                      S/Kademlia Target NodeID                 |
|                            (32 bytes)                         |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|   Target Port Number (16-bit) |    TreeID     |   BindFlags   |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+

RELAY_BIND Frame (Type 0x31):
Sent by the Relayer to establish the active routing bridge.
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|   Version     |  Type (0x31)  |          Payload Length       |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                      S/Kademlia Client A ID                   |
|                            (32 bytes)                         |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                               |
|                      S/Kademlia Client B ID                   |
|                            (32 bytes)                         |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|    TreeID     |   BindFlags   |          Reserved (0)         |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

*   **`TreeID`:** the tree whose push is bridged from B to A (`0x00` = PULL service only). The relay charges $B_m \Omega$ of its **tree budget** per bridged tree (§6.3.1, Ch1 §1.2.1 *The Two Budgets*).
*   **`Target NodeID` / `Target Port`** in `RELAY_PROPOSAL`: the parent $B$ that A could not reach, or all-zero to let the relay run parent selection for $T_m$ itself — the relay is `PUBLIC` and can punch to any `CONE` candidate, so its choice is usually better than A's.
*   **`BindFlags`:** bit 0 `SOURCE_INGRESS` — see below. Bit 1 `REJECTED` (in `RELAY_BIND` only): the relay declines — no budget, or no parent obtained for `TreeID` — and A tries the next `RELAY_CAPABLE` record. Other bits reserved.
*   `RELAY_BIND` is sent by the relay to **A** once it holds a slot at B for `TreeID` (its own `NEIGHBOR`/`ACCEPTED` exchange with B is an ordinary tree join, §6.3.1). `Client B ID` names the parent actually obtained.

## The Source Behind NAT

Every join path assumes the broadcaster can be reached: it registers under $K_s$ as a `RELAY`-class peer with `Flags.SOURCE` (Ch2 §2.3.2) and is the root of every tree. A broadcaster on a home connection behind CGNAT — a primary use case at $N = 2$–$10$ — cannot be. It **must** therefore satisfy the `RELAY` reachability requirement (`PUBLIC` or `CONE`, Ch1 §1.2.5), or bind one or more **ingress relays**:

1.  The source finds `RELAY_CAPABLE` peers exactly as any NAT-blocked peer does and sends `RELAY_PROPOSAL` with `BindFlags.SOURCE_INGRESS`, signed with $SK_{\text{Source}}$ inside the QUIC session so the relay can verify it is the publisher.
2.  The relay answers with `RELAY_BIND(SOURCE_INGRESS)`, registers under $K_s$ with `Flags.SOURCE_INGRESS` and `AssignedTrees` covering every tree, and becomes hop 0 of the forest: the source pushes all $M$ slices to it ($\Omega B$ of the source's upload per ingress relay) and it forwards them as the root would.
3.  Joiners treat a `SOURCE_INGRESS` record exactly as they treat the source. The hop penalty (§1.2.2) counts the ingress relay as depth 0.

**An ingress relay is a root, and is budgeted as one.** Its rendezvous assignment is set aside for the duration of the bind: it is a $t_v = M$ node whose assignment is every tree, and it lends **tree slots**, not reserve. With $n_{\text{ingress}}$ ingress relays bound, each carries its share of the source's base-layer pool $R_{\text{src}} = 3 b_0 \Omega$ (Ch1 §1.1.5 §5.4) and holds

$$K_R(m) = \left\lfloor \frac{(1 - r_{\text{pull}})\, u_R - R_{\text{src}} / n_{\text{ingress}}}{M \cdot B_m \cdot \Omega} \right\rfloor$$

ordinary slots in each tree. The source accepts a relay as ingress only if $K_R(m) \ge 2$ for **every** tree, which the largest per-tree bitrate decides: $0.9\,u_R \ge 2 M B_{\max} \Omega + R_{\text{src}}/n_{\text{ingress}}$. With two ingress relays on the reference ladder that is $13.8 + 2.6 \Rightarrow u_R \ge 18.2$ Mbps at $M = 2$ ($B_{\max} = 3.0$) and $20.7 + 2.6 \Rightarrow u_R \ge 25.9$ Mbps at $M = 6$ ($B_{\max} = 1.5$) — so the $20$ Mbps `RELAY_CAPABLE` floor suffices for the small swarm an ingress relay is usually bound in, and a stream that grows to $M = 6$ needs its ingress relays at $\ge 26$ Mbps. $40$ Mbps gives $6$ slots per $0.75$ Mbps tree and $3$ per $1.5$ Mbps tree at $M = 6$, which is the recommended minimum for a stream expected to grow. The source's own cost is $\Omega B \approx 6.9$ Mbps per ingress relay, so a broadcaster on a $10$ Mbps uplink can bind one and one on $15$ Mbps two. An earlier draft said only that the relay "forwards as the root would" and that the reserve was "met through them"; read against §6.3.1's reserve accounting, a stream's root would have had $2$ Mbps of egress.

Two ingress relays are recommended so that the root is not a single point of failure. A source that can bind no ingress relay cannot broadcast — the specification states this plainly rather than assuming reachability.
