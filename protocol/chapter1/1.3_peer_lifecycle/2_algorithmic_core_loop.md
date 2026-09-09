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
26:                 
27:         case CONNECTING:
28:             Parents <- ExecuteMultiForestJoin(SubscribedTrees)  // NOT all M — see 1_transition_model.md
29:             If Size(Parents) == Size(SubscribedTrees) then:
30:                 State <- ACTIVE
31:                 
32:         case ACTIVE:
33:             // Streaming never pauses for a repair: every tree with a parent is
34:             // received, verified, forwarded and PULL-served on every iteration.
35:             ProcessStreamingBuffers(SubscribedTrees \ Repairing)
36:             ServePullRequests(); SendRostersIfDue(); IssueReceiptsIfDue()
37:             EvaluateNeighborScores()
38:             For each m in SubscribedTrees:
39:                 If ParentTimeoutDetected(m, Timestamp) then:      // RTT-scaled, Ch3 §3.3
40:                     Repairing <- Repairing ∪ {m}                  // per-tree CHURN_REPAIR
41:             For each m in Repairing:
42:                 If TriggerParentReRoute(m) == RESTORED then:       // Ch3 §3.3, tree m only
43:                     Repairing <- Repairing \ {m}
44:             If Size(Parents) == 0 then:
45:                 State <- DISCOVERY                                  // every parent lost
44:                 
45:     // Handle OS signals and low-latency network packet arrivals
46:     PollNetworkSockets()
47:     SleepMicroseconds(500) // Yield CPU
```
