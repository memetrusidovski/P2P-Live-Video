# 1. Prisoner's Dilemma Math

In permissionless, tokenless peer-to-peer live streaming, peers are rational agents seeking to maximize their utility (low-latency, high-resolution video playout) while minimizing their costs (bandwidth usage, CPU consumption). Left unmanaged, this leads to the *Tragedy of the Commons*, where all nodes select free-riding strategies, collapsing the swarm.

We model the interaction between any two active neighbors $A$ and $B$ as an infinitely repeated Prisoner's Dilemma game. The payoff matrix for a single round of interaction is defined as:

| | $B$ Cooperates (Uploads) | $B$ Defects (Chokes) |
| :--- | :---: | :---: |
| **$A$ Cooperates (Uploads)** | $(R, R)$ | $(S, T)$ |
| **$A$ Defects (Chokes)** | $(T, S)$ | $(P, P)$ |

where:
*   $T$ (Temptation to defect) is the payoff of downloading while refusing to upload.
*   $R$ (Reward for mutual cooperation) is the payoff of a smooth, cooperative stream.
*   $P$ (Punishment for mutual defection) is the payoff of stream failure/buffering.
*   $S$ (Sucker's payoff) is the cost of uploading to a free-rider without receiving data.
*   **Payoff Ranking:** $T > R > P > S$

In an infinitely repeated game with a discount factor $\delta$ (representing the probability that the interaction continues), the **Tit-for-Tat (TFT)** strategy is a subgame perfect Nash Equilibrium if and only if:
$$\delta \ge \frac{T - R}{T - P}$$

Our protocol implements a localized **Adaptation-Aware Tit-for-Tat** mechanism to enforce this equilibrium dynamically over short, 2-second sliding windows.
