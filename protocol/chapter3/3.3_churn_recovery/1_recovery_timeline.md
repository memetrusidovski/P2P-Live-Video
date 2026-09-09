# 1. Recovery Timeline (Sub-250ms)

At scale, thousands of users join and leave every minute. Standard TCP timeout boundaries (which can range from 15 to 30 seconds) are catastrophic for real-time live streaming buffers. Our protocol mandates sub-second active churn recovery following a strict timeline:

```text
  T = 0ms                      T = 100ms                  T = 200ms                    T = 250ms
  [ Last Video Chunk Recv ]    [ Ping Probe Sent ]        [ Connection Timeout ]       [ Target Standby Connects ]
         |                            |                          |                            |
         v                            v                          v                            v
  +--------------+             +--------------+           +--------------+             +--------------+
  |  Normal Flow |             | Probing Node |           | Evict Parent |             | Re-Route     |
  |  Streaming   |             | (No response)|           | From Active  |             | Video Slice  |
  +--------------+             +--------------+           +--------------+             +--------------+
                                                                 |
                                                                 v
                                                          Select candidate
                                                          from Passive Set
```

*   **T = 0 ms:** Peer $i$ receives the last valid video chunk slice from parent $P$.
*   **T = 100 ms:** Since no new video slice packet has arrived, the peer sends an active `PING` probe over its UDP/QUIC control channel to $P$.
*   **T = 200 ms:** No `PONG` response is received. The peer marks the parent as dead, evicts it from the Active Set $\mathcal{A}$, and demotes it to the Passive Set $\mathcal{P}$.
*   **T = 210 ms:** Peer $i$ queries its local Passive Set $\mathcal{P}$, selects the standby peer with the highest historical contribution score, and sends an urgent `NEIGHBOR` promotion request.
*   **T = 250 ms:** Standby peer accepts the handshake. Sub-stream transmission resumes, restoring the slice with minimal video buffer disruption.

## Relationship to Tree Healing (Ch1 §1.2.3)

This timeline is the **canonical recovery primitive** of the protocol. The Sibling Election of Chapter 1 §1.2.3 is a coordination layer *on top of* it for the multi-orphan case (a dead tree parent with several children): the election deterministically picks which orphan re-attaches the subtree, and that Deputy then executes exactly this passive-set promotion procedure to find its own new parent. Every election failure path (no roster, stale roster, Deputy unresponsive within $\tau_{\text{deputy}} = 45\text{ ms}$) falls back to each orphan running this timeline independently.

## Small-Swarm Fallback: Cached Discovery Peer List

The 250 ms path requires a usable Passive Set. At small swarm sizes the Passive Set structurally cannot fill: with $N$ total peers the maximum is $|\mathcal{P}| = N - 1 - |\mathcal{A}|$, which is $0$ at $N = 5$ and still far below $c_p = 32$ until $N \approx 41$. Falling back to a *fresh* DHT lookup costs 1–3 seconds — long enough to drain the playout buffer and cause visible stalls.

To keep recovery fast at small $N$, every peer retains the **peer list returned by its most recent DISCOVERY-phase `GET_PEERS` query** (Ch2 §2.3) as an in-memory emergency candidate cache. The CHURN_REPAIR path becomes:

```python
# In CHURN_REPAIR state (T = 210ms):
if Size(PassiveSet) < 3:
    # Passive set too thin to be useful — use the cached DHT peer list
    # from the last DISCOVERY (already in memory, zero round-trips)
    candidates = GetCachedDHTPeers(StreamID)
else:
    candidates = GetTopScoredPeers(PassiveSet)
SendNeighborRequest(BestCandidate(candidates), priority=HIGH)
```

The cached list costs no extra network traffic to maintain — it is refreshed for free whenever the peer re-queries the DHT (registration refreshes every $\tau_{\text{ttl}}/2$, and the Stream Record is re-fetched on manifest updates). Using the cache turns the small-$N$ worst case from a 1–3 s DHT walk into a ~200 ms reconnect. A full DHT re-lookup remains the final fallback if every cached candidate is unreachable.
