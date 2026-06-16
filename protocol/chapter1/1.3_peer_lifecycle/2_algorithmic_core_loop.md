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
17:             If PublisherRecords is not empty then:
18:                 State <- JOINING
19:                 
20:         case JOINING:
21:             TriggerHyParViewHandshakes()
22:             If Size(ActiveSet) >= TargetActiveSize then:
23:                 State <- CONNECTING
24:                 
25:         case CONNECTING:
26:             Parents <- ExecuteMultiForestJoin(M)
27:             If Size(Parents) == M then:
28:                 State <- ACTIVE
29:                 
30:         case ACTIVE:
31:             ProcessStreamingBuffers()
32:             EvaluateNeighborScores()
33:             If AnyParentTimeoutDetected(Timestamp) then:
34:                 State <- CHURN_REPAIR
35:                 
36:         case CHURN_REPAIR:
37:             TriggerSubsecondParentReRoute()
38:             If AllSlicesRestored() then:
39:                 State <- ACTIVE
40:             Else If AllConnectionsLost() then:
41:                 State <- DISCOVERY
42:                 
43:     // Handle OS signals and low-latency network packet arrivals
44:     PollNetworkSockets()
45:     SleepMicroseconds(500) // Yield CPU
```
