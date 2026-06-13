# Chapter 5: Economic Incentives and Fair-Share Swarm Contribution

### 5.1 Adaptation-Aware Tit-For-Tat (TFT) Game Theory

To eliminate free riders, peers evaluate their neighbors on a sliding game-theoretic Tit-For-Tat (TFT) scale:
*   **Game Representation:** The interaction between peer $A$ and peer $B$ is modeled as an infinitely repeated Prisoner's Dilemma.
*   **Unchoking Evaluation:** Every $500\text{ ms}$, peer $A$ ranks its active set $\mathcal{A}$ based on the data volume received during the last window.
*   **The Tit-For-Tat Rule:**
    $$\text{Status}(B) = \begin{cases} 
      \text{Unchoked (Upload Active)} & \text{if } \text{EgressRate}(B \to A) \ge \text{Threshold} \\
      \text{Choked (Upload Paused)} & \text{otherwise}
    \end{cases}$$
*   **Optimistic Unchoking:** One random peer from the Passive Set $\mathcal{P}$ is unchoked per cycle to discover new high-capacity neighbors.

### 5.2 Cryptographic Proof-of-Upload (PoU) Receipt Architecture

To prevent falsified contribution claims, uploads are verified using cryptographic Proof-of-Upload (PoU) receipts:
*   When peer $A$ successfully sends a verified block $B_i$ to peer $B$, $B$ must immediately generate and send a signed PoU receipt back to $A$:
    $$\text{PoU} = \text{Sign}_{SK_B}(\text{StreamID} \parallel \text{SeqNo} \parallel \text{SenderID} \parallel \text{Timestamp})$$
*   Peer $A$ stores these receipts in a local ledger. When establishing new connections or requesting promotion closer to the source, $A$ presents its PoU receipts to prove its historic contribution to the network.

### 5.3 Local Reputation Gossip Auditing (Sybil and Collusion Resistance)

*   **Collusion Attack Vector:** Two Sybil nodes might sign fake PoUs for each other to artificially pump their scores.
*   **Decentralized Auditing:** Neighbors gossip localized peer audits. If peer $X$ presents PoUs issued by peers $Y$ and $Z$, neighbors check if $Y$ and $Z$ reside on unique IP subnets and ASNs. If a suspicious cluster of reciprocal signings is detected (e.g., highly clustered cyclic graphs), the reputation score is completely nullified, and the entire cluster is flagged for eviction.
