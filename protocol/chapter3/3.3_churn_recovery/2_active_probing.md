# 2. Active Probing Mechanics

The following event-driven algorithm runs continuously as part of the peer's low-latency execution thread to enforce the 250ms timeline:

```text
Algorithm: Localized Churn Recovery
Input: ActiveSet, PassiveSet, SliceID, CurrentTimestamp
Output: Connection Recovery Status

1:  ParentNode <- GetParentForSlice(SliceID)
2:  LastReceivedTime <- GetLastPacketTimestamp(ParentNode)
3:  
4:  If (CurrentTimestamp - LastReceivedTime) > 100_000 microseconds then:
5:      // Step 1: Probe Parent Node
6:      SendUDPControlPacket(ParentNode, PING)
7:      
8:  If (CurrentTimestamp - LastReceivedTime) > 200_000 microseconds then:
9:      // Step 2: Parent has failed. Evict immediately.
10:     EvictNodeFromActiveSet(ParentNode)
11:     AddNodeToPassiveSet(ParentNode, reason=TIMEOUT)
12:     
13:     // Step 3: Find a reliable candidate in PassiveSet
14:     CandidateFound <- False
15:     While not CandidateFound and Size(PassiveSet) > 0 do:
16:         Candidate <- SelectBestReputationPeer(PassiveSet)
17:         Result <- InitiateQUICStream(Candidate)
18:         If Result == SUCCESS then:
19:             // Request parent role for target slice
20:             Ack <- SendPromotionRequest(Candidate, SliceID, priority=HIGH)
21:             If Ack == ACCEPTED then:
22:                 RegisterParentForSlice(Candidate, SliceID)
23:                 MoveNodeToActiveSet(Candidate)
24:                 CandidateFound <- True
25:             Else:
26:                 RemoveFromPassiveSet(Candidate) // Bad peer or saturated
27:                 
28:     If not CandidateFound then:
29:         // Critical Fallback: Query DHT for new active superpeers
30:         NewPeers <- QueryDHT(StreamID)
31:         AddPeersToPassiveSet(NewPeers)
32:         TriggerEmergencyReconnectLoop()
```
