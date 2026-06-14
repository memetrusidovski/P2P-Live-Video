# Chapter 8: Simulation and Evaluation Plan

## 8.1 Simulation Framework Selection

To validate a protocol designed for million-scale live streaming before physical deployment, we must run extensive simulated tests. We define two separate testing environments to evaluate the logical overlay and physical transport layers respectively:

### 1. Event-Driven Macro Simulation (OMNeT++ / NS-3)
*   **Purpose:** To evaluate the logical overlay graph properties, gossip convergence rates, and multi-forest routing behaviors under massive peer scales ($10,000$ to $1,000,000$ simulated nodes).
*   **Implementation:** Using **OMNeT++** with a customized network layer or **NS-3**'s discrete-event simulation model. Peers are modeled as light state machines in memory, and the physical internet routing is simulated using simplified latency coordinates (Vivaldi coordinate spaces).

### 2. Containerized Emulation (Mininet / Docker)
*   **Purpose:** To validate the real-world performance of socket bindings, STUN/ICE hole punching, QUIC connection migrations, and eBPF kernel rate-limit filter throughput under realistic CPU and packet queuing constraints.
*   **Implementation:** Using **Mininet** to emulate network topologies (varying bandwidth, latency, and packet drop ratios) running containerized **Docker** nodes executing the actual compiled protocol binaries.

---

## 8.2 Key Performance Indicators (KPIs)

The performance of the protocol is measured against four critical metrics:

### 1. Playback Starvation Ratio (PSR)
The fraction of playout intervals where the client's assembly buffer is missing required video symbols, causing a playback freeze:
$$\text{PSR} = \frac{\text{StarvedPlayoutIntervals}}{\text{TotalPlayoutIntervals}}$$
*   **Target:** $\text{PSR} \le 0.1\%$ across the entire swarm.

### 2. Startup Join Latency (SJL)
The elapsed time from when a user clicks "Play" to the rendering of the first verified GOP segment. This evaluates the efficiency of S/Kademlia bootstrap, HyParView handshakes, and multi-forest parent allocations:
$$\text{SJL} = T_{\text{first\_frame}} - T_{\text{join\_intent}}$$
*   **Target:** $\text{SJL} \le 1.5 \text{ seconds}$ under standard conditions.

### 3. Control-to-Data Overhead Ratio (CDO)
The percentage of total egress bandwidth consumed by control metadata (DHT RPCs, gossip shuffles, pings, bitfields) versus actual video data payloads (source blocks and RaptorQ parities):
$$\text{CDO} = \frac{\text{Bytes}_{\text{control}}}{\text{Bytes}_{\text{video}} + \text{Bytes}_{\text{control}}} \cdot 100\%$$
*   **Target:** $\text{CDO} \le 2.0\%$ under active streaming conditions.

### 4. Average Overlay Hop Count ($D_{\text{avg}}$)
The average number of routing hops traversed by a chunk slice before arriving at a leaf node:
$$D_{\text{avg}} = \frac{1}{N} \sum_{i=1}^{N} \text{HopCount}(i)$$
*   **Target:** $D_{\text{avg}} \le c \log_k(N)$ (typically $\le 7\text{ hops}$ for $N = 1,000,000$).

---

## 8.3 Stress and Adversarial Scenarios

The simulation suite must run automated stress tests simulating adversarial attacks and volatile network failures:

```text
                                [ Test Scheduler ]
                               /        |         \
                              v         v          v
                  [ Flash Crowd ] [ Churn Storm ] [ Sybil Attack ]
```

*   **Scenario A: The Churn Storm (Volatile Departure):** Suddenly disconnect $30\%$ of the active relays in Layer 1 and Layer 2 at once. The simulation evaluates if the remaining nodes can self-heal (using Chapter 3's sub-second re-routing and Chapter 1's tree balancing) without causing playout starvation.
*   **Scenario B: The Malicious Injection Flood:** Inject $10\%$ malicious nodes that continuously distribute corrupted blocks with forged Merkle path hashes. The simulation verifies that the Blake3 Merkle engine (Chapter 4) drops the bytes immediately and the local audit gossip (Chapter 5) blacklists the attackers.
*   **Scenario C: Saturated Symmetric NAT Environments:** Force $75\%$ of the simulated peers behind symmetric NATs or CGNAT. The simulation tests if the Emergent Relayer algorithm (Chapter 6) can recruit and scale public-IP relays autonomously without incurring TURN server fallback dependencies.
