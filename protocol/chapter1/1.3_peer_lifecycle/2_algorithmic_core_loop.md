# 2. Algorithmic Core Main Loop

The peer runtime execution loop is implemented as a non-blocking, event-driven loop operating with microsecond accuracy. This loop dictates how state transitions are triggered by external events.

```text
Algorithm: Peer Lifecycle Main Loop
Input: Node Config, StreamID
Output: Execution Status

1:  State <- BOOTSTRAP
2:  InitializeCryptoEngine()
3:  ActiveSet <- empty map, PassiveSet <- empty map
4:  
5:  While State != TERMINATED do:
6:      Timestamp <- GetMicrosecondTimestamp()
7:      
8:      // State Evaluation
9:      Match State with:
10:         case BOOTSTRAP:
11:             LoadStaticSeedsFromConfig()
12:             GenerateS_KademliaNodeID()
13:             State <- DISCOVERY
14:             
15:         case DISCOVERY:
16:             PublisherRecords <- QueryDHT(StreamID)
17:             SwarmSize <- KnownPeerCount(PublisherRecords)  // cached for JOINING
18:             If PublisherRecords is not empty then:
19:                 State <- JOINING
20:                 
21:         case JOINING:
22:             TriggerHyParViewHandshakes()
23:             JoinThreshold <- Max(1, Min(4, SwarmSize - 1))  // adaptive (see 1_transition_model.md)
24:             If Size(ActiveSet) >= JoinThreshold then:
25:                 State <- CONNECTING
24:                 
26:         case CONNECTING:
27:             Parents <- ExecuteMultiForestJoin(M)
28:             If Size(Parents) == M then:
29:                 State <- ACTIVE
30:                 
31:         case ACTIVE:
32:             ProcessStreamingBuffers()
33:             EvaluateNeighborScores()
34:             If AnyParentTimeoutDetected(Timestamp) then:
35:                 State <- CHURN_REPAIR
36:                 
37:         case CHURN_REPAIR:
38:             TriggerSubsecondParentReRoute()
39:             If AllSlicesRestored() then:
40:                 State <- ACTIVE
41:             Else If AllConnectionsLost() then:
42:                 State <- DISCOVERY
43:                 
44:     // Handle OS signals and low-latency network packet arrivals
45:     PollNetworkSockets()
46:     SleepMicroseconds(500) // Yield CPU
```
