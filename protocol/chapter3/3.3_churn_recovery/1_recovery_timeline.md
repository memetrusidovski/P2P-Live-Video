# 1. Recovery Timeline (RTT-Scaled, Sub-Second)

At scale, thousands of users join and leave every minute. Standard TCP timeout boundaries (which can range from 15 to 30 seconds) are catastrophic for real-time live streaming buffers. Our protocol mandates sub-second active churn recovery following a strict timeline whose detection phase scales with the path's measured round-trip time:

```text
  T = 0                      T = tau_ping (100 ms)      T = tau_evict                 T = tau_evict + ~2 RTT
  [ Last packet from P ]     [ PING probe sent ]        [ Parent declared dead ]      [ Standby connected ]
         |                          |                          |                            |
         v                          v                          v                            v
  +--------------+           +--------------+           +--------------+             +--------------+
  |  Normal Flow |           | Probing Node |           | Evict Parent |             | Re-Route     |
  |  Streaming   |           | (No response)|           | From Active  |             | Video Slice  |
  +--------------+           +--------------+           +--------------+             +--------------+
                                                               |
                                                               v
                                                        Filtered pick from
                                                        per-tree pool P_m
```

*   **T = 0 ms:** Peer $i$ receives the last packet — media, heartbeat or control — from parent $P$ in tree $T_m$.
*   **T = $\tau_{\text{ping}}$ = 100 ms:** Nothing has arrived. The peer sends an active `PING` probe over the connection's control channel to $P$.
*   **T = $\tau_{\text{evict}}$:** No packet of any kind has arrived. The peer marks the parent as dead **for tree $T_m$**, evicts it from the Active Set $\mathcal{A}$, and demotes it to the Passive Set $\mathcal{P}$.
*   **T = $\tau_{\text{evict}}$ + 10 ms:** Peer $i$ takes the per-tree pool $\mathcal{P}_m$ for the dead parent's tree (§3.1.1) — standbys already known to relay $T_m$ and to be reachable — selects the one with the highest reliability $R$ and advertised $K_{\text{avail}}$, opens a QUIC session to it and sends an urgent `NEIGHBOR(TreeID = m, Priority = HIGH)` promotion request, honouring the distinct-parent rule (Ch1 §1.2.1).
*   **T = $\tau_{\text{evict}}$ + $\approx 2\,RTT$:** Standby peer accepts. Sub-stream transmission resumes, restoring the slice with minimal video buffer disruption.

**Re-attachment costs round-trips, not a constant.** The standby comes from the Passive Set, to which no socket is open (§3.1.1), and `NEIGHBOR` rides a QUIC control stream (Appendix D §D.2). Re-attachment is therefore a QUIC handshake plus one request/response: $\approx 2\,RTT$ to a peer never spoken to (one for the handshake, one for `NEIGHBOR`/`ACCEPTED`), $\approx 1\,RTT$ where the peer holds a **0-RTT resumption ticket** for the standby — which it does for any standby that was demoted from its own Active Set, and does not for one learned only from gossip. Peers **must** retain resumption tickets for every peer they have held a session with, and **should** send the `NEIGHBOR` in the 0-RTT flight when a ticket exists. An earlier draft quoted a flat "$+50$ ms", which is $\approx 2\,RTT$ only at $25$ ms and a fiction on the paths the RTT-scaled eviction deadline was introduced for.

## Silence Is Not Evidence; Heartbeats Are

Media arrival is bursty by nature — a chunk's blocks arrive, then nothing until the next chunk — so "no video for 100 ms" cannot by itself mean anything. Two rules make the timeline sound:

1.  **The parent keeps the channel warm.** A parent that has sent nothing to a child for $\tau_{\text{ping}}$ sends a heartbeat (a QUIC `PING` frame, or the protocol `PING` on the control stream). A healthy connection therefore never shows more than $\tau_{\text{ping}}$ of silence, whatever the media pattern, and the child's probe at $\tau_{\text{ping}}$ fires only when a heartbeat was actually lost (UDP loss of one packet, $1$–$2\%$) or the parent is gone.
2.  **The eviction timeout follows the path.** The child's `PING` cannot be answered in less than one RTT, so a fixed $200$ ms deadline declares every parent more than $100$ ms away dead on its first lost heartbeat — intercontinental, satellite and jittery cellular paths, most links at $N = 10^6$. The deadline is instead taken from the connection's own QUIC RTT estimator:

    $$\tau_{\text{evict}} = \max\left(200\text{ ms},\ 2\,\tau_{\text{ping}} + SRTT + 4\,RTTVAR\right)$$

    — one heartbeat interval to notice, one RTT plus jitter for the probe, one more interval of slack. Recovery completes $\approx 2\,RTT$ after eviction (above). By path:

    | RTT | $\tau_{\text{evict}}$ | Re-attach ($1$–$2$ RTT) | Repair complete | Repair $+ 2$ PULL round-trips |
    | :---: | :---: | :---: | :---: | :---: |
    | $20$ ms | $240$ | $20$–$40$ | $\approx 280$ | $\approx 320$ |
    | $80$ ms | $320$ | $80$–$160$ | $\approx 480$ | $\approx 640$ |
    | $250$ ms | $490$ | $250$–$500$ | $\approx 990$ | $\approx 1{,}490$ |

    The headline is therefore **sub-300 ms on local paths and about one second on the worst intercontinental ones**, not a flat 250 ms; the flat figure was only ever true for parents within 100 ms. The last column is what the PULL zone of Ch4 §4.3.1 has to cover, and is why that zone widens with the path's RTT.

The source, whose output is the whole forest's input, additionally **paces** each chunk's symbols over at most half the chunk period (§3.3.2) so that the tree carries a near-continuous flow rather than four bursts per second; this also keeps every relay's XDP token bucket (Ch7 §7.2) inside its burst allowance.

## Relationship to Tree Healing (Ch1 §1.2.3)

This timeline is the **canonical recovery primitive** of the protocol. The Sibling Election of Chapter 1 §1.2.3 is a coordination layer *on top of* it for the multi-orphan case (a dead tree parent with several children): the election deterministically picks which orphan re-attaches the subtree, and that Deputy then executes exactly this passive-set promotion procedure to find its own new parent. Every election failure path (no roster, stale roster, Deputy unresponsive within $\tau_{\text{deputy}} = 45\text{ ms}$) falls back to each orphan running this timeline independently. Siblings detect the parent's death within $\approx 4\,RTTVAR$ of one another rather than simultaneously, since each evaluates $\tau_{\text{evict}}$ on its own path; the roster's 5 s staleness bound comfortably covers that spread.

## Small-Swarm Fallback: Tiered Candidate Sources

The fast path requires a usable per-tree pool. At small swarm sizes the Passive Set structurally cannot fill: with $N$ total peers the maximum is $|\mathcal{P}| = N - 1 - |\mathcal{A}|$, which is $0$ at $N = 5$ and still far below $c_p = 32$ until $N \approx 41$. Falling back to a *fresh* DHT lookup costs 1–3 seconds — long enough to drain the playout buffer and cause visible stalls.

A thin Passive Set is therefore supplemented, not replaced, from three further sources. They are ordered by **cost first, then freshness**, and the cheap ones are tried concurrently rather than in sequence:

| Tier | Source | Cost | Freshness |
| :---: | :--- | :--- | :--- |
| 1 | Per-tree pool $\mathcal{P}_m$ of the Passive Set | 0 RTT | Maintained by SHUFFLE / GOSSIP_EXCHANGE with `WantedTrees` steering |
| 2 | Cached DISCOVERY peer list | 0 RTT | Up to $\tau_{\text{ttl}}/2 = 90\text{ s}$ stale |
| 3 | **Direct shuffle request**: `SHUFFLE` (0x06) with `TTL = 0` and `WantedTrees = ` bit $m$ over the existing session to a **surviving** active-set peer, answered by `GOSSIP_EXCHANGE` (§3.2.1) | 1 RTT (~40–80 ms) | Current |
| 4 | Fresh DHT `GET_PEERS(WantedTrees = \text{bit } m)` | 1–3 s | Current |

Two properties of this ordering are load-bearing:

*   **Tiers 1 and 2 are unioned, never substituted.** A peer with $|\mathcal{P}| = 2$ holds two *live, recently verified* standbys; discarding them in favour of a cache that may be 90 seconds old would be strictly worse. The cached list extends the candidate pool, it does not replace it.
*   **A dead tree parent does not mean a peer is isolated.** A peer holds up to $M$ parents, one per tree, plus its active set. When the parent for tree $T_m$ dies the other $M-1$ are almost always alive — so tier 3 has somebody to ask, and it costs one RTT for a *current* answer rather than 1–3 s for the same thing. This is why tier 3 sits above the DHT walk and not below it. (Siblings from the child roster are deliberately **not** a source here: they were children of the same dead parent and are themselves orphaned — resolving that case is what the Deputy election of Ch1 §1.2.3 is for.)

```python
# In CHURN_REPAIR state (T = 210ms):
candidates = RankByReliabilityAndKavail(PassivePool[m])   # tier 1 — relays of T_m only

if Size(candidates) < 3:
    # tier 2: zero round-trips, may be stale — extends the pool, never replaces it
    candidates += [r for r in CachedDHTPeers(StreamID) if r.relays(m) and r.reachable]

    # tier 3: one RTT for a current answer, dispatched CONCURRENTLY so its
    # latency overlaps the tier-1/2 connection attempts rather than following them.
    # TTL = 0 over an open session is a direct request; the receiver answers with a
    # GOSSIP_EXCHANGE drawn from its pool for tree m within tau_sched (§3.2.1).
    AsyncSendShuffle(AnySurvivingActivePeer(), ttl=0, wanted_trees=bit(m))

SendNeighborRequest(BestCandidate(candidates), tree_id=m, priority=HIGH)

# tier 4: only if every candidate above is unreachable
if not Connected:
    QueryDHT(StreamID, wanted_trees=bit(m))
```

### Keeping the Cache Warm

The cached list is refreshed whenever the peer queries the DHT for a thin pool, and the Stream Record arrives with every such response. At $90\text{ s}$ that cache is a *best-effort* pool, not a guarantee: in a small swarm with turnover, some entries will be dead, which is precisely why tier 3 exists behind it.

A peer whose per-tree pool is persistently thin ($|\mathcal{P}_m| < 3$ for some subscribed tree) therefore re-queries `GET_PEERS(WantedTrees = \text{bit } m)` for that tree, at most every $30\text{ s}$ (Ch2 §2.3.2). This costs almost nothing in the regime where it applies — small $N$ means few peers, few guardians, and a small response — and it is the same regime where the cache is the difference between a sub-second reconnect and a visible stall. There is no other periodic `GET_PEERS`: at $N = 10^6$ the pools are full from gossip and the guardians must not be polled by a million peers on a timer.
