# 2. Active Probing Mechanics

The following event-driven algorithm runs continuously as part of the peer's low-latency execution thread, **once per subscribed tree**, to enforce the timeline of §1. `LastReceivedTime` counts *any* packet from the parent — media, heartbeat or control — and `TauEvict` is the RTT-scaled deadline of §1, read from the connection's QUIC estimator:

```text
Algorithm: Localized Churn Recovery
Input: ActiveSet, PassiveSet, SliceID, CurrentTimestamp
Output: Connection Recovery Status

1:  ParentNode <- GetParentForSlice(SliceID)
2:  LastReceivedTime <- GetLastPacketTimestamp(ParentNode)
3:  
4:  If (CurrentTimestamp - LastReceivedTime) > TAU_PING (100_000 us) then:
5:      // Step 1: Probe Parent Node (once per silence episode)
6:      SendUDPControlPacket(ParentNode, PING)
7:      
7a: TauEvict <- Max(200_000, 2*TAU_PING + SRTT(ParentNode) + 4*RTTVAR(ParentNode))
8:  If (CurrentTimestamp - LastReceivedTime) > TauEvict then:
9:      // Step 2: Parent has failed FOR THIS SLICE. Evict; other slices keep streaming (Ch1 §1.3).
10:     EvictNodeFromActiveSet(ParentNode)
11:     AddNodeToPassiveSet(ParentNode, reason=TIMEOUT)
12:     
13:     // Step 3: Find a reliable candidate among known relays of THIS slice
14:     CandidateFound <- False
15:     While not CandidateFound and Size(PassivePool[SliceID]) > 0 do:
16:         Candidate <- SelectBestReputationPeer(PassivePool[SliceID])  // filtered pick, §3.1.1
17:         Result <- InitiateQUICStream(Candidate)
18:         If Result == SUCCESS then:
19:             // Request parent role for target slice
20:             Ack <- SendPromotionRequest(Candidate, SliceID, priority=HIGH)
21:             If Ack == ACCEPTED then:
22:                 RegisterParentForSlice(Candidate, SliceID)
23:                 MoveNodeToActiveSet(Candidate)
24:                 CandidateFound <- True
25:             Else:                                             // DISCONNECT(TreeID=SliceID, Reason) — App D §D.4.3b
26:                 If Ack == REJECTED_NOT_ASSIGNED then ClearTreeBit(Candidate, SliceID)  // stale bitmap: fix the pool, keep the peer
27:                 Else If Ack in (REJECTED_SATURATED, REJECTED_DEPTH) then MarkFullThisRound(Candidate)  // full, not bad: keep it
27a:                Else RemoveFromPassiveSet(Candidate)                                  // timeout or EVICTION only
27:                 
28:     If not CandidateFound then:
29:         // Critical Fallback: Query DHT for relays of this slice
30:         NewPeers <- QueryDHT(StreamID, wanted_trees=bit(SliceID))
31:         AddPeersToPassiveSet(NewPeers)
32:         TriggerEmergencyReconnectLoop()
```

## Keeping the Channel Warm, and Pacing the Source

The detector above is sound only if silence on a healthy connection is bounded. Two sender-side rules guarantee that:

*   **Heartbeat on idle.** Every relay tracks, per child, the time of its last transmitted packet; if $\tau_{\text{ping}}$ elapses with nothing sent, it sends a heartbeat. This costs one small packet per idle 100 ms per child and nothing at all while media is flowing.
*   **Source pacing.** The source emits each chunk's symbols spread evenly over at most **half the chunk period** ($\le 125$ ms of the 250 ms, Appendix B) instead of as a burst the instant the manifest is signed. Relays forward blocks as they verify them, so pacing at the root paces the whole forest: a child at any depth sees a near-continuous trickle of symbols with gaps far shorter than $\tau_{\text{ping}}$, and no relay's egress ever exceeds $2\times$ its mean — which is what keeps a 10 Gbps super node's per-child XDP buckets (Ch7 §7.2, burst 1000 packets) from dropping legitimate media. The cost is at most $125$ ms added to the last block of each chunk, already counted in the glass-to-glass budget (Ch1 §1.1.3).
