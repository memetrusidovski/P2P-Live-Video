# 2. Random Walk Propagation

When a new node $p$ joins, it discovers a bootstrap contact $c$ via the DHT and sends a `JOIN` message. To preserve random clustering and small-world properties, the node uses a **Random Walk** to propagate the join request:

1.  **Direct Handshake:** $c$ immediately inserts $p$ into its own Active Set $\mathcal{A}_c$. If $\mathcal{A}_c$ is full, $c$ evicts a random member to make room, demoting the evicted node to its Passive Set $\mathcal{P}_c$.
2.  **Random Walk Propagation:** $c$ forwards the join request as a `FORWARD_JOIN` frame to its active neighbors. The message contains two parameter constants:
    *   **Active Time-To-Live ($TTL_{\text{active}} \approx 4$):** Decremented at each hop. While $TTL > 0$, the frame is forwarded to a random active neighbor.
    *   **Passive Walk Start ($PR_{\text{gossip}} \approx 2$):** When the hop count reaches $PR_{\text{gossip}}$, the forwarding node adds $p$'s address to its own Passive Set.
3.  **Leaf Attachment:** When $TTL$ reaches $0$, the final receiving node $q$ is forced to add $p$ to its Active Set $\mathcal{A}_q$, establishing a stable, random long-distance connection.
