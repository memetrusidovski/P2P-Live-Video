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
