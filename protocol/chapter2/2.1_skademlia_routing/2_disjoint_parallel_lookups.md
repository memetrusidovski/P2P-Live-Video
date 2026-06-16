# 2. S/Kademlia Parallel Disjoint Lookups

Standard Kademlia is vulnerable to routing manipulation (Eclipse Attacks) where malicious nodes intercept queries and return false paths, isolating a victim from the legitimate swarm. 

Our protocol utilizes **S/Kademlia parallel disjoint lookups** to mathematically guarantee secure routing against fractional adversaries.

1.  **Concurrency ($\alpha$):** A node initiates lookup queries to $\alpha$ independent peers in parallel (typically $\alpha = 3$).
2.  **Disjoint Paths:** The routing algorithm forces the lookup requests to traverse strictly disjoint paths through the network. A node from path 1 is never allowed to return a referral that overlaps with path 2.
3.  **Probability of Hijack ($P_{\text{hijack}}$):** If an attacker controls a fraction $f$ of the nodes in the network, the probability $P_{\text{hijack}}$ of the lookup being compromised over $d$ hops with $\alpha$ disjoint paths drops exponentially:
    $$P_{\text{hijack}} \le f^\alpha$$
    
### Adversarial Simulation Example
Assume a highly toxic network where attackers control $20\%$ of all peers ($f = 0.2$). 
With $\alpha = 3$ disjoint paths:
$$P_{\text{hijack}} \le (0.2)^3 = 0.008 \approx 0.8\%$$

By simply enforcing 3 parallel lookups, the chance of an attacker successfully routing a peer to a fake stream drops to less than $1\%$, securing the discovery layer.

```text
                  Path 1: [Local Node] ---> [Peer A] ---> [Peer B] ---> [Target Node]
                /
  [Local Node] === Path 2: [Local Node] ---> [Peer C] ---> [Peer D] ---> [Target Node]
                \
                  Path 3: [Local Node] ---> [Peer E] ---> [Peer F] ---> [Target Node]
```
