# 2. Contribution Score and Its Presentation

## 2.1 The Score

Peers aggregate the receipts they have *received* (as uploader) into a **Swarm Contribution Score** $\Theta_A$. A verifying node computes it from a set of receipts $R$ as

$$\Theta_A = \sum_{j \in R} w_j \cdot \mathbb{I}\bigl(\text{VerifySignature}(PoU_j)\bigr) \cdot \text{Bytes}(PoU_j) \cdot \varphi\bigl(\text{Age}(PoU_j)\bigr)$$

where:
*   $\mathbb{I}(\cdot)$ verifies that the signature matches the S/Kademlia identity of the issuing downloader and that the receipt names $A$ as uploader and $K_s$ as the stream;
*   $\text{Bytes}(PoU_j) = 16\text{ KB} \times \text{popcount}(\text{BlockBitmap}_j)$ — the verified payload the receipt attests;
*   $w_j = 3$ for a `RELAYED` receipt (Ch6 §6.3.2), otherwise $1$; **`PULL` receipts from any one counterparty are credited at most $\beta_{\text{pull}} \times 1\text{ s} = 62.5$ KB per segment**, so that two peers exchanging fake repair traffic cannot pump each other faster than genuine repair could;
*   $\varphi(t) = e^{-\lambda t}$, $\lambda = 0.005\ \text{s}^{-1}$, devalues old receipts (a receipt older than 5 minutes is worth $< 25\%$), forcing peers to maintain active, continuous contribution.

The **delivered-throughput rate** that gates multi-tree standing (Ch1 §1.2.1) is the undecayed 60-second sum:

$$\Theta^{\text{rate}}_A = \frac{1}{60\text{ s}} \sum_{j : \text{Age}(PoU_j) \le 60\text{ s}} w_j \cdot \text{Bytes}(PoU_j) \cdot 8 \quad [\text{bit/s}]$$

and the **rank** used in tree admission (Ch1 §1.2.2) is $\Theta^{\text{rate}}_A / B$ — full-stream-equivalents currently being delivered. A leaf, or a node that has relayed nothing yet, has rank $0$.

## 2.2 Presenting the Score: `RANK_PROOF`

"$A$ presents a bundle of verified receipts" is the mechanism by which rank enters a decision at another node — a tree parent deciding admission, or an auditor. It cannot mean sending every receipt: a super node's 60-second window holds $\approx 520{,}000$ of them. Nor can a verifier trust a self-reported total. The presentation is a **non-interactive commit-then-sample proof**, `RANK_PROOF` (0x19, Appendix D §D.4.17):

1.  **Commit.** $A$ orders its receipts from the trailing 60 s, builds a Merkle tree over their hashes, and signs $(\text{ReceiptCount}, \text{TotalBytes}, \text{CommitSegmentSeq}, \text{ReceiptListRoot})$ during segment `CommitSegmentSeq`.
2.  **Nonce.** The sample is selected by a value $A$ could not have known when it committed: the Merkle root of a chunk `MANIFEST` whose `SegmentSeq` is **greater than** `CommitSegmentSeq` — signed by the source after the commitment existed. Every peer holds recent manifests, so the verifier checks the nonce against its own copy.
3.  **Sample.** $\text{idx}_i = \text{Blake3}(\text{ReceiptListRoot} \parallel \text{Nonce} \parallel i) \bmod \text{ReceiptCount}$ for $i = 0 \ldots 7$. $A$ attaches those eight receipts with their Merkle paths.
4.  **Verify.** The verifier checks $A$'s signature, that the nonce manifest post-dates the commitment and is one it holds, that each sampled receipt verifies (downloader signature, uploader $= A$, stream $= K_s$, age $\le 60$ s), that each Merkle path reaches `ReceiptListRoot`, and that `TotalBytes` $\le \text{ReceiptCount} \times 512 \times 16$ KB. It then uses `TotalBytes` for $\Theta^{\text{rate}}$. **Any** failed sample rejects the whole proof, and a peer presenting a proof that fails is treated as having presented a forged signature (Ch7 §7.1.1: local ban).

Because the ordering is fixed before the nonce exists, $A$ cannot grind the list to steer the sample away from fabricated entries. Inflating the list by a fraction $f$ of fake receipts survives the sample with probability $(1-f)^8$: $10\%$ at $f = 0.25$, $0.4\%$ at $f = 0.5$. A proof is $\approx 8 \times (244 + 32 \cdot \lceil \log_2 \text{ReceiptCount} \rceil)$ bytes — under $8$ KB for a super node, a few hundred bytes for a home relay — and is sent once per admission attempt on the QUIC control stream, never per block.

A parent that has held $A$ as a **child** for a while needs no proof to rank $A$ as a *downloader*; but it learns nothing about $A$'s contribution *to others* except through `RANK_PROOF`. That is why admission reads the proof and not the parent's local view.
