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
28:             JoinTrees <- SubscribedTrees ∪ AssignedTrees          // a relay always joins its own trees
29:             Parents <- ExecuteMultiForestJoin(JoinTrees)          // NOT all M — see 1_transition_model.md
30:             If HasParent(Parents, m) for every m in SubscribedTrees then:
31:                 State <- ACTIVE                                   // gated on SubscribedTrees only
32:                 
33:         case ACTIVE:
34:             // Streaming never pauses for a repair: every tree with a parent is
35:             // received, verified, forwarded and PULL-served on every iteration.
36:             ProcessStreamingBuffers(JoinTrees \ Repairing)        // forward assigned trees even if not rendered
37:             ServePullRequests(); SendRostersIfDue(); IssueReceiptsIfDue()
38:             EvaluateNeighborScores()
39:             For each m in JoinTrees:
40:                 If ParentTimeoutDetected(m, Timestamp) then:      // RTT-scaled, Ch3 §3.3
41:                     Repairing <- Repairing ∪ {m}                  // per-tree CHURN_REPAIR
42:                 If DrainNoticeReceived(m) then:                    // parent is releasing us, App D §D.4.19
43:                     Repairing <- Repairing ∪ {m}                  // warm repair: old parent still delivering
44:             For each m in Repairing:
45:                 If TriggerParentReRoute(m) == RESTORED then:       // Ch3 §3.3, tree m only
46:                     Repairing <- Repairing \ {m}
47:             For each m in AssignedTrees \ ParentsOf(Parents):
48:                 RetryJoinAtMigrationCadence(m)                      // unparented assigned tree, Ch1 §1.1.5 §5.3
49:             If Size(Parents) == 0 then:
50:                 State <- DISCOVERY                                  // every parent lost
51:                 
52:     // Handle OS signals and low-latency network packet arrivals
53:     PollNetworkSockets()
54:     SleepMicroseconds(500) // Yield CPU
```
