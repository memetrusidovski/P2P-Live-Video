# 1. NAT Permutation Matrix

Because a majority of internet users reside behind Network Address Translators (NATs) or firewalls, establishing direct, low-latency peer-to-peer UDP connections is a major engineering hurdle. We classify NAT behaviors using STUN (RFC 5389) into four primary classes: Full Cone (FC), Restricted Cone (RC), Port-Restricted Cone (PRC), and Symmetric NAT / Carrier-Grade NAT (CGNAT).

The mathematical probability of establishing a direct, un-assisted peer-to-peer UDP connection depends on the NAT permutation of the two communicating endpoints, as summarized in the following outcome matrix:

| Endpoint A \ Endpoint B | Full Cone (FC) | Restricted Cone (RC) | Port-Restricted (PRC) | Symmetric / CGNAT |
| :--- | :---: | :---: | :---: | :---: |
| **Full Cone (FC)** | **Direct (100%)** | **Direct (100%)** | **Direct (100%)** | **Direct (100%)** |
| **Restricted Cone (RC)** | **Direct (100%)** | **Hole Punch (100%)** | **Hole Punch (100%)** | **Hole Punch (95%)** |
| **Port-Restricted (PRC)** | **Direct (100%)** | **Hole Punch (100%)** | **Hole Punch (90%)** | **Hole Punch (10%)** |
| **Symmetric / CGNAT** | **Direct (100%)** | **Hole Punch (95%)** | **Hole Punch (10%)** | **Relay Required (0%)** |

The protocol collapses these four classes into the three-valued **`Reachability`** carried in every Peer Record (Ch2 §2.3.3): `PUBLIC` = no NAT or full cone, `CONE` = restricted or port-restricted, `SYMMETRIC` = symmetric or CGNAT. A node determines its own value from two seed reflections during bootstrap (Ch2 §2.1.3). The consequences for the forest are drawn in Ch1 §1.2.5: `PUBLIC` and `CONE` nodes may relay; `SYMMETRIC` nodes are leaf-class and are served by parents they connect to outbound, or through an emergent relay (§6.3). In the table's terms, a `SYMMETRIC` child reaches a `PUBLIC` parent directly and a `CONE` parent with a hole punch at 95% (RC) or 10% (PRC) — which is why such a child prefers `PUBLIC` parents and why the emergent relay exists for the remainder.
