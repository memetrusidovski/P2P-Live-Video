# 2. Sliding-Window Unchoker

Every $500\text{ ms}$, the peer executes the following scheduler to evaluate reciprocity across its Active Set $\mathcal{A}$:

```text
Algorithm: Sliding-Window Tit-for-Tat Unchoker
Input: ActiveSet, UnchokeSlotsCount (typically 4)
Output: New Choke/Unchoke States

1:  CurrentTime <- GetMicrosecondTimestamp()
2:  NeighborsList <- GetPeersInActiveSet(ActiveSet)
3:  
4:  // Calculate rolling ingress download rates for each neighbor over the last 2 seconds
5:  For each peer in NeighborsList do:
6:      peer.RollingIngressRate <- CalculateIngressRate(peer, window=2_000_000)
7:  
8:  // Sort active neighbors by their contribution rate in descending order
9:  SortByIngressRateDescending(NeighborsList)
10: 
11: // Unchoke top contributors
12: For i from 0 to UnchokeSlotsCount - 1 do:
13:     BestPeer <- NeighborsList[i]
14:     If BestPeer.ChokedState == CHOKED then:
15:         BestPeer.ChokedState <- UNCHOKED
16:         SendControlPacket(BestPeer, UNCHOKE)
17:         
18: // Choke non-reciprocating peers
19: For i from UnchokeSlotsCount to Size(NeighborsList) - 1 do:
20:     BadPeer <- NeighborsList[i]
21:     If BadPeer.ChokedState == UNCHOKED then:
22:         BadPeer.ChokedState <- CHOKED
23:         SendControlPacket(BadPeer, CHOKE)
```

The Tit-For-Tat Rule is strictly enforced:
$$\text{Status}(B) = \begin{cases} 
  \text{Unchoked (Upload Active)} & \text{if } \text{EgressRate}(B \to A) \ge \text{Threshold} \\
  \text{Choked (Upload Paused)} & \text{otherwise}
\end{cases}$$
