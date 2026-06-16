# 3. Rarest-First Pull Heuristics and Buffer Scheduler

The scheduling engine runs every $100\text{ ms}$ to evaluate buffer health and dispatch reactive pull requests. It determines chunk urgency based on the approaching playout deadline and chunk rarity based on neighbor bitfields.

```text
Algorithm: Deadline-Aware Swarm Buffer Scheduler
Input: LocalBufferState, ActiveSet, PlayoutDeadline, SegmentID
Output: Pull Requests Dispatched

1:  CurrentTime <- GetMicrosecondTimestamp()
2:  MissingBlocks <- IdentifyMissingBlocks(LocalBufferState, SegmentID)
3:  
4:  For each block_index in MissingBlocks do:
5:      Urgency <- (PlayoutDeadline - CurrentTime)
6:      If Urgency <= 0 then:
7:          DropBlock(SegmentID, block_index) // Too late to decode
8:          Continue
9:          
10:     // Find neighbors who possess the block based on their Bitfields
11:     CandidateNeighbors <- GetNeighborsWithBlock(ActiveSet, SegmentID, block_index)
12:     
13:     If Size(CandidateNeighbors) > 0 then:
14:         // Sort neighbors using a weighted score of capacity and latency
15:         SortByReputationAndRTT(CandidateNeighbors)
16:         BestProvider <- CandidateNeighbors[0]
17:         
18:         // Dispatch PULL request (Type 0x16)
19:         SendPULLRequest(BestProvider, SegmentID, block_index, urgency=Urgency)
20:         RegisterPendingRequest(SegmentID, block_index, BestProvider, CurrentTime)
21:     Else:
22:         // Block is rare; trigger emergency gossip lookup
23:         TriggerRarityGossip(SegmentID, block_index)
```
