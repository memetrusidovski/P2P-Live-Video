# 3. Optimistic Exploration

If nodes only uploaded to active contributors, new joining peers with empty buffers could never start downloading. To solve this, the protocol allocates **1 Optimistic Unchoke Slot**:

*   Every $2\text{ seconds}$ (4 TFT cycles), the node selects a random peer $C$ from its Passive Set $\mathcal{P}$ or Active Set and unchokes it regardless of its current upload rate.
*   This allows $C$ to bootstrap its buffer and start uploading back. 
*   If $C$ reciprocates within the next $2\text{ seconds}$ by uploading at an egress rate exceeding $\text{Threshold}_{\text{min}}$, it is promoted into the active unchoked pool; otherwise, it is re-choked, and a new candidate is explored.

```text
// Appended to the Sliding-Window Unchoker execution loop:

// Optimistic Unchoke: Select 1 random peer from PassiveSet to explore capacity
Candidate <- SelectRandomPeer(PassiveSet)
SendControlPacket(Candidate, UNCHOKE_OPTIMISTIC)
```
