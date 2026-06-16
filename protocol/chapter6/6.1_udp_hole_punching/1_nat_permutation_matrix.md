# 1. NAT Permutation Matrix

Because a majority of internet users reside behind Network Address Translators (NATs) or firewalls, establishing direct, low-latency peer-to-peer UDP connections is a major engineering hurdle. We classify NAT behaviors using STUN (RFC 5389) into four primary classes: Full Cone (FC), Restricted Cone (RC), Port-Restricted Cone (PRC), and Symmetric NAT / Carrier-Grade NAT (CGNAT).

The mathematical probability of establishing a direct, un-assisted peer-to-peer UDP connection depends on the NAT permutation of the two communicating endpoints, as summarized in the following outcome matrix:

| Endpoint A \ Endpoint B | Full Cone (FC) | Restricted Cone (RC) | Port-Restricted (PRC) | Symmetric / CGNAT |
| :--- | :---: | :---: | :---: | :---: |
| **Full Cone (FC)** | **Direct (100%)** | **Direct (100%)** | **Direct (100%)** | **Direct (100%)** |
| **Restricted Cone (RC)** | **Direct (100%)** | **Hole Punch (100%)** | **Hole Punch (100%)** | **Hole Punch (95%)** |
| **Port-Restricted (PRC)** | **Direct (100%)** | **Hole Punch (100%)** | **Hole Punch (90%)** | **Hole Punch (10%)** |
| **Symmetric / CGNAT** | **Direct (100%)** | **Hole Punch (95%)** | **Hole Punch (10%)** | **Relay Required (0%)** |
