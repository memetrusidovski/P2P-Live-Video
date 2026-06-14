# 2. The Parent Selection & Scoring Algorithm

## 2.1 The Multivariate Scoring Function

When a peer $i$ wishes to join tree $T_m$ as a child, it must select the best possible parent from a pool of candidates discovered via the S/Kademlia DHT. Selecting a random parent leads to high latency and geographic inefficiency. Selecting only based on bandwidth creates hotspot bottlenecks.

To solve this, peer $i$ evaluates every candidate parent $p$ using a continuous **Multivariate Scoring Function**:
$$\text{Score}(p, i) = \left( w_1 \cdot \text{CapacityScore}(p) \right) - \left( w_2 \cdot \text{LatencyPenalty}(p, i) \right) - \left( w_3 \cdot \text{HopPenalty}(p) \right)$$

### Component 1: Capacity Score
The parent $p$ gossips its available remaining upload slots ($K_{\text{avail}}$) and historical reliability ($R \in [0, 1]$, representing the percentage of chunks delivered on time).
$$\text{CapacityScore}(p) = \ln(1 + K_{\text{avail}}) \cdot R$$
*We use the natural log so that a peer with 100 slots is scored higher than one with 10, but not 10 times higher, encouraging load distribution.*

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
            
            cap_score = math.log(1 + probe_response.available_slots) * probe_response.reliability
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

## 2.3 Continuous Optimization (Greedy Upward Migration)

Parent selection is not a one-time event. Trees degrade over time due to churn. Therefore, nodes run a background thread every $5\text{ seconds}$ to continuously optimize the tree.

1. Node $i$ currently has parent $P_{\text{current}}$ with $\text{Score}(P_{\text{current}})$.
2. Through the gossip layer (HyParView Passive Set), node $i$ learns of a new node $P_{\text{new}}$.
3. If $\text{Score}(P_{\text{new}}) > \text{Score}(P_{\text{current}}) + HysteresisMargin$:
    * Node $i$ connects to $P_{\text{new}}$.
    * Once $P_{\text{new}}$ begins delivering chunks, node $i$ sends a `DISCONNECT_CHOKE` frame to $P_{\text{current}}$.
    * Node $i$ has successfully migrated upward to a faster/shallower branch without dropping a single video frame. The $HysteresisMargin$ prevents nodes from oscillating back and forth rapidly between two similar parents.
