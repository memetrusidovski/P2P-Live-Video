# 2. Exponential Decay Scoring

Peers aggregate received PoU receipts to calculate their global **Swarm Contribution Score** $\Theta_A$. When $A$ connects to a new parent or requests layer promotion, it presents a bundle of verified receipts. The verifying node computes $A$'s reputation score using the following weighted mathematical model:

$$\Theta_A = \sum_{j=1}^{R} w_j \cdot \mathbb{I}(\text{VerifySignature}(PoU_j)) \cdot \ln(1 + \text{Duration}(PoU_j)) \cdot \varphi(\text{Age}(PoU_j))$$

where:
*   $\mathbb{I}(\cdot)$ verifies that the signature matches the S/Kademlia identity of the issuing peer.
*   $\text{Duration}(PoU_j)$ represents the continuous block series uploaded.
*   $\varphi(t) = e^{-\lambda t}$ is an exponential decay function that devalues old receipts (typically $\lambda = 0.005$, making receipts older than 5 minutes virtually worthless), forcing peers to maintain active, continuous contribution.
