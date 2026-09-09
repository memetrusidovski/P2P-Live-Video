# SOLUTION-041: `RANK_PROOF` Proves Distinctness and Reads Bytes From the Sample

**Closes:** ISSUE-041 (High); amends SOLUTION-024
**Lives in:** `protocol/chapter5/5.2_proof_of_upload/2_exponential_decay_scoring.md` (§2.1 window and cap, §2.2 rewritten); `appendix_d_frame_registry.md` §D.4.17; `appendix_b_parameters.md` (Rank sample, Receipt cap, Rank window)
**Class:** A proof that verified membership when the decision needed cardinality (recurring pattern #5 — a check sound for one adversary, an attack surface for another)

---

## The problem in one line

The verifier checked eight sampled receipts and then used a prover-declared `TotalBytes` bounded only by `ReceiptCount` $\times 8$ MB, with neither number tied to the sample and no distinctness check — so a relay with 600 genuine receipts could declare rank $107$, or list each receipt a thousand times and declare rank $1{,}280$, and every sampled receipt would verify.

## The decision

*   The receipt list is **sorted by a canonical content key** $(\text{SegmentSeq}, \text{TreeID}, \text{RxFlags}, \text{Downloader})$, strictly increasing.
*   Samples are **eight adjacent pairs** $(2p, 2p+1)$ with a shared Merkle path; the verifier checks $\text{key}(\text{left}) < \text{key}(\text{right})$ in each. A duplicate must sit beside its original in sorted order, so a fraction $f$ of duplicates fails with probability $1 - (1-f)^8$, the same as fabrication.
*   **`TotalBytes` is removed.** $\Theta^{\text{rate}} = \text{ReceiptCount} \times \overline{\min(\text{Bytes}, \text{Cap})}_{16} \times 8 / 60$ s, with $\text{Cap} = 1.5 \times \text{BitrateKbps}_m \times 1$ s for a tree receipt and $\beta_{\text{pull}} \times 1$ s for a `PULL` receipt.
*   Receipt age is judged by **`SegmentSeq` against the verifier's live edge** (60 segments), not by the downloader-signed timestamp.

## Why this and not the alternatives

*   **Order leaves by hash and check order statistics** (the issue's proposal). This does *not* catch duplication: $d$ copies of each of $n$ distinct hashes, sorted, place hash $h_k$ at positions $\approx (k-1)d \ldots kd$, so the leaf at fraction $x$ of the list still has hash $\approx x \cdot 2^{256}$. Every order statistic is preserved. The check binds `ReceiptCount` to the number of *leaves*, which is the number the prover controls.
*   **Require the 8 samples to be pairwise distinct**: catches duplication only when two samples land in the same run — $\approx 28/n$ for $n$ distinct receipts, under $5\%$ for a home relay.
*   **Interactive challenge from the verifier**: one RTT per join, the same reason SOLUTION-024 rejected it.
*   **Keep `TotalBytes` as a sanity bound**: it has no information the sample does not, and a second declared number is a second thing to get wrong.

## Defects found during verification

*   Inflation table for an honest 600-receipt relay (rank $1.28$): `TotalBytes` at the cap → rank $107$; $1000\times$ duplication → rank $1{,}280$; both pass every check of the old rule. Sixteen draws over 600 receipts collide with probability $1 - e^{-0.2} \approx 18\%$, so even a distinctness check on samples would have missed four in five duplicators.
*   The per-receipt cap of $512 \times 16$ KB was a 64 Mbps stream's; the verifier holds the tree's declared bitrate in the slicing matrix and never used it. It cannot use the exact manifest block counts because it retains manifests for only $\tau_{\text{retain}} = 8$ s of a 60 s window, hence the $1.5\times$ declared-bitrate cap.
*   Age by wall-clock timestamp let a colluding downloader post-date receipts into the window; the window is now the verifier's own segment count.
*   Proof size grows from $\le 8$ KB to $\le 10$ KB at $2^{20}$ receipts (two bodies per pair, one fewer path level).

## The generalisable lesson

**A sampling proof establishes what the sample can be compared against. If the decision consumes a total, the sample must estimate the total; if it consumes a count, the list must have a structure in which a wrong count is locally visible.** Membership of eight leaves says nothing about how many leaves there are.

## Residual risk

*   The byte estimate from 16 receipts carries $\approx \pm 15\%$ error at one standard deviation; the prover cannot steer it, and every rank threshold is a ratio with margin, but an honest node near a threshold will sometimes fall on the wrong side. Chapter 8 decides whether 8 pairs suffice.
*   A prover with many *genuine* colluding downloaders (Sybil leaves that never upload back) still inflates rank through real receipts; that is the collusion problem of §5.3, where the symmetry test does not see one-directional Sybil edges. The subnet penalty of §5.3.2 is the intended bound and cannot currently be applied to receipts from peers the verifier has no address for — filed as ISSUE-057.

## Validation owed (Chapter 8)

*   False-reject rate of honest proofs and detection rate against $f \in \{0.1, 0.25, 0.5\}$ fabricated or duplicated receipts, at 8 and 16 pairs.
*   Distribution of the byte estimate's error across the reference ladder's stripe mix.
