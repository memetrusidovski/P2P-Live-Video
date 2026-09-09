# 2. The Parent Selection & Scoring Algorithm

## 2.1 The Multivariate Scoring Function

When a peer $i$ wishes to join tree $T_m$ as a child, it must select the best possible parent from a pool of candidates discovered via the S/Kademlia DHT. Selecting a random parent leads to high latency and geographic inefficiency. Selecting only based on bandwidth creates hotspot bottlenecks.

To solve this, peer $i$ evaluates every candidate parent $p$ using a continuous **Multivariate Scoring Function**:
$$\text{Score}(p, i) = \left( w_1 \cdot \text{CapacityScore}(p) \right) - \left( w_2 \cdot \text{LatencyPenalty}(p, i) \right) - \left( w_3 \cdot \text{HopPenalty}(p) \right)$$

### Component 1: Capacity Score
The parent $p$ gossips its available remaining upload slots ($K_{\text{avail}}$) and historical reliability ($R \in [0, 1]$, representing the percentage of chunks delivered on time).
$$\text{CapacityScore}(p) = \sqrt{K_{\text{avail}}} \cdot R$$
*We use square-root compression so that a peer with more slots is scored higher, but not proportionally higher, encouraging load distribution. An earlier draft used $\ln(1 + K_{\text{avail}})$, but the log is too flat at the top end: it gave a 10 Gbps server (10,000 slots) only a 3.8× advantage over a 10 Mbps node — so nearby mid-tier peers routinely outscored available backbone capacity. The square root yields a 31.6× advantage in that comparison: strong enough to route toward super nodes, still far from the raw 1000× that would cause monopolisation. The exponent is a tuning parameter to be validated in simulation (Chapter 8) before deployment.*

**Warm-Up Gating:** A freshly joined relay has no data to forward until it has received and Merkle-verified its first complete segment (~1 second of buffering). During this window it MUST advertise $K_{\text{avail}} = 0$, which removes it from parent selection entirely (probes are skipped by the `available_slots > 0` check below). Without this rule, a new relay's high capacity score attracts children who then receive nothing for a full second — the relay answers keepalives (so it is never evicted as dead) while its empty bitfield forces every child into mesh-PULL fallback, compounding into latency spikes during rapid-growth phases when many relays are warming up at once.

```python
# Capacity gossip advertisement:
if len(verified_segment_buffer) == 0:
    advertise_K_avail = 0                  # warm-up: hidden from parent selection
else:
    advertise_K_avail = floor(u_v / B_m)   # normal advertisement
```

### Component 2: Latency Penalty (RTT)
Peer $i$ pings candidate $p$ over UDP to establish the current Round Trip Time (RTT) in milliseconds.
$$\text{LatencyPenalty}(p, i) = \text{RTT}(p, i)$$

### Component 3: Hop Penalty (Layer Depth)
To prevent the tree from becoming too deep, parent $p$ advertises its hop-distance $h_p$ from the source in tree $T_m$.
$$\text{HopPenalty}(p) = e^{\lambda \cdot h_p}$$
*The exponential penalty ensures that joining deeply nested nodes (e.g., Hop 10) is heavily penalized compared to joining a node at Hop 2, aggressively keeping the tree shallow.*

---

## 2.2 The Tree Join Algorithm (Pseudocode)

The following sequence dictates exactly how a node connects to the network upon completing its DHT bootstrap phase.

```python
import time

def execute_tree_join(node_i, target_slice_m, dht_interface):
    # Step 1: Discover Candidates
    # Query the DHT specifically for peers assigned to relay slice 'm'
    candidate_list = dht_interface.get_peers(stream_id, slice=target_slice_m)
    
    scored_candidates = []
    
    # Step 2: Parallel Probing
    for p in candidate_list:
        # Send lightweight UDP probe to measure RTT and fetch current Hop Count/Capacity
        probe_response = send_udp_probe(target=p.ip, timeout_ms=200)
        
        if probe_response.success and probe_response.available_slots > 0:
            # Step 3: Execute Scoring Function
            w1, w2, w3 = 1000.0, 1.0, 50.0  # Tuning weights
            
            cap_score = math.sqrt(probe_response.available_slots) * probe_response.reliability
            lat_penalty = probe_response.rtt_ms
            hop_penalty = math.exp(0.5 * probe_response.hop_count)
            
            final_score = (w1 * cap_score) - (w2 * lat_penalty) - (w3 * hop_penalty)
            scored_candidates.append( {"peer": p, "score": final_score} )

    # Step 4: Sort and Dispatch
    # Sort descending so the highest score is index 0
    scored_candidates.sort(key=lambda x: x["score"], reverse=True)
    
    for best_candidate in scored_candidates:
        # Send formal join request via QUIC
        join_ack = send_quic_join_request(best_candidate["peer"], target_slice_m)
        
        if join_ack == ACCEPTED:
            register_active_parent(best_candidate["peer"], target_slice_m)
            return SUCCESS
            
    # If all candidates rejected (e.g., they filled their slots during probing)
    return FAILURE_RETRY_BACKOFF
```

### Depth Admission Rule

A parent **must reject** any join request that would place the child deeper than $D_{\text{max}} = 8$ hops from the source — that is, a candidate advertising $h_p \ge D_{\text{max}}$ is not a legal parent. A saturated forest may never resolve pressure by growing deeper than its latency budget allows.

### On `FAILURE_RETRY_BACKOFF`

This return value is the protocol's **capacity-saturation signal** for tree $T_m$: every candidate either had no free slots or sat at the depth limit. Its handling is defined normatively in [Chapter 1 §1.1.5 Capacity Adaptation](../1.1_scale_latency/5_capacity_adaptation.md): the peer retries with backoff, and after **2 consecutive failed rounds** sheds tree $T_m$ (dropping to the next lower SVC layer) rather than continuing to search, with a 10-second hysteresis before the standard upward-migration loop attempts re-join. The base-layer tree is never shed.

## 2.3 Continuous Optimization (Greedy Upward Migration)

Parent selection is not a one-time event. Trees degrade over time due to churn. Therefore, nodes run a background thread every $5\text{ seconds}$ to continuously optimize the tree.

1. Node $i$ currently has parent $P_{\text{current}}$ with $\text{Score}(P_{\text{current}})$.
2. Through the gossip layer (HyParView Passive Set), node $i$ learns of a new node $P_{\text{new}}$.
3. If $\text{Score}(P_{\text{new}}) > \text{Score}(P_{\text{current}}) + HysteresisMargin$:
    * Node $i$ connects to $P_{\text{new}}$.
    * Once $P_{\text{new}}$ begins delivering chunks, node $i$ sends a `DISCONNECT_CHOKE` frame to $P_{\text{current}}$.
    * Node $i$ has successfully migrated upward to a faster/shallower branch without dropping a single video frame. The $HysteresisMargin$ prevents nodes from oscillating back and forth rapidly between two similar parents.
