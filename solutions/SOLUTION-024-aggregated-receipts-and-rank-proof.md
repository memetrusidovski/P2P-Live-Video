# SOLUTION-024: One Receipt per Segment per Tree, and a Sampled Proof of Rank

**Closes:** ISSUE-024 (High); resolves ISSUE-029 item 2 (loss rate on the wire) and completes ISSUE-029
**Lives in:** `protocol/chapter5/5.2_proof_of_upload/1_receipt_cryptography.md`, `2_exponential_decay_scoring.md`, `3_pou_frame.md` (all rewritten); `appendix_d_frame_registry.md` §D.4.12, §D.4.17 (`RANK_PROOF` 0x19); `appendix_b_parameters.md`; `protocol/chapter4/4.1_segment_serialization/2_progressive_blake3_hashing.md`; `appendix_a_sequence_diagrams.md` A.3
**Class:** A per-unit cost that was fine at home-relay fan-out and impossible at super-node fan-out (recurring pattern #2)

---

## The problem in one line

One signed receipt per 16 KB block per child is $\approx 66{,}000$ Ed25519 verifications per second at a 10 Gbps node — 4–8 cores, 120 Mbps inbound, 800 MB per 60 s window — and the "bundle of verified receipts" a node presents to earn standing had no bound at all.

## The decision

*   **One receipt per segment per tree**, carrying a bitmap of the blocks verified, the link loss rate, and flags for `RELAYED` and `PULL`. Volume drops $\approx 7.6\times$: $\approx 8{,}700$ verifications per second at the super node, under one core.
*   **$\Theta$ counts bytes**, $16\text{ KB} \times \text{popcount}$, weighted $3\times$ for `RELAYED`, with `PULL` credit from any one counterparty capped at $\beta_{\text{pull}} \times 1$ s per segment. $\Theta^{\text{rate}}$ is the undecayed 60 s sum; rank is $\Theta^{\text{rate}}/B$.
*   **`RANK_PROOF` is a commit-then-sample proof**: the prover signs (count, total bytes, Merkle root of its ordered receipt list) during segment $s$; the sample of 8 receipts is selected by the Merkle root of a chunk manifest signed *after* $s$; any failing sample rejects the proof. Under 8 KB at any fan-out.
*   **`LossRate` rides in the receipt**, closing the last open item of ISSUE-029: the child→parent loss report that adaptive parity and the slot-count overhead factor consume needed no frame of its own.

## Why this and not the alternatives

*   **Per-block receipts on the push path, aggregated only on the pull path** (the issue's suggestion) leaves the super node's problem intact — the push path *is* the volume.
*   **A range (`FirstBlock`, `LastBlock`) instead of a bitmap** is 4 bytes instead of up to 64 but cannot express the ordinary case of one lost block in the middle of a segment; the bitmap is exact and averages 8–16 bytes.
*   **An interactive challenge** (verifier sends a nonce, prover returns the sample) is the textbook construction and costs one RTT at every tree join — the join path's budget is 40 ms. Using a future source-signed manifest root as the nonce makes the proof non-interactive without letting the prover grind: the list is committed before the nonce exists, and the verifier already holds the manifest.
*   **Trusting the prover's own total with spot checks against the parent's local view** fails because a parent has no local view of a joiner's uploads to *other* peers — that is the whole point of presenting receipts.

## Defects found during verification

*   The old score's $\ln(1 + \text{Duration})$ term measured "continuous block series" — a quantity no receipt carried. Bytes verified is what the receipt attests and what the multi-tree gate ($\Theta^{\text{rate}}$ against $B$) actually needs.
*   Sample statistics: inflating the receipt list by a fraction $f$ survives with probability $(1-f)^8$ — $10\%$ at $f = 0.25$, $0.4\%$ at $f = 0.5$. A prover who is caught is treated as having forged a signature (local ban). The expected gain from a $25\%$ inflation attempted repeatedly is negative.
*   Bitmap capacity: 4 chunks $\times$ 128 blocks per chunk $= 512$ bits $= 64$ bytes, i.e. streams up to $\approx 64$ Mbps per tree stripe. Above that the receipt would need a second bitmap page; noted, not needed for the reference ladder.
*   `PULL` receipts had to be excluded from the symmetry test (SOLUTION-039) and capped instead, since mesh repair is legitimately bidirectional. Without the cap, two peers could pump each other through fake repair receipts with no tree edge to audit.

## The generalisable lesson

**Anything signed per unit of data must be re-costed at the maximum fan-out the design permits, and anything "presented" to a verifier must have a bounded size and a sampling rule the presenter cannot steer.** The receipt was designed for ten children; the design also allows eight thousand.

## Residual risk

The nonce manifest must post-date the commitment and the verifier must hold it; a prover and verifier whose live edges differ by more than $\tau_{\text{retain}}$ cannot complete a proof and fall back to rank 0 for that join. Rare, and self-correcting once the joiner is pushed a current manifest.

## Validation owed (Chapter 8)

*   Receipt verification CPU at the super node under Scenario A churn (bursts of re-joins mean bursts of new receipts).
*   Empirical false-reject rate of `RANK_PROOF` in an honest swarm (should be zero) and detection rate against a prover inflating by 10–50%.
