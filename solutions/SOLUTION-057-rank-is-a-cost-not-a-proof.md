# SOLUTION-057: The Subnet Penalty Applies to Resolvable Signers; Forged Rank Is Bounded Where It Is Spent

**Closes:** ISSUE-057 (Medium — but the finding is more serious than the ticket's title)
**Lives in:** `protocol/chapter5/5.3_reputation_auditing/2_ip_subnet_penalties.md` (rewritten); `protocol/chapter5/5.2_proof_of_upload/2_exponential_decay_scoring.md` (*What the proof does and does not establish*); `protocol/chapter1/1.2_multi_forest_overlays/1_graph_theory_and_slicing.md` (*Multi-Tree Eligibility*, "the gate is a cost"); `protocol/chapter2/2.2_crypto_node_id/2_sybil_defense_math.md` (table row); `appendix_c_threat_model.md` §C.2.3
**Class:** A defence stated in three chapters whose only input no verifier holds (recurring pattern #4, for a value rather than a frame)

---

## The problem in one line

§5.3.2 discounted a receipt bundle when more than 20% of its signers shared a prefix, and a verifier has no address for a signer it has never met — none at all for leaf-class signers, who are DHT clients in nobody's buckets — so the one mechanism the specification named against Sybil-downloader rank inflation could not be applied where rank is verified.

## The decision

*   The penalty applies to the **resolvable** signers of the sampled receipts: those the verifier has observed itself, plus relay-class signers resolved lazily by `FIND_NODE` after admission (a clustered bundle revokes the admission through the drain path). Leaf-class signers are stated to be unresolvable by anyone but the uploader that served them.
*   The specification now says plainly what that leaves: **a colluding uploader with leaf-class Sybil downloaders can forge rank at a cost of identities, not bandwidth** — about 10 ms per distinct receipt key, no bytes sent. The distinctness and cap checks of SOLUTION-041 bound the value of each fabricated receipt and force distinct identities; they do not make identities expensive, and nothing in this protocol does.
*   The bound on forged rank is therefore relocated to the three places rank is spent, each of which holds without the rank being honest: the honest parent's prefix cap on child slots (one enhancement slot per prefix per parent per tree, taken from a rank-0 child any contributor would also displace); the children's reliability observation and the prefix cap for multi-tree standing; the re-verified evidence requirement for accuser standing.
*   SOLUTION-007's claim that the throughput gate makes multi-tree standing unforgeable is corrected to "unforgeable for free".

## Why this and not the alternatives

*   **Carry the downloader's address in the receipt, signed by the downloader.** A downloader learns its external address from the bootstrap reflections and can bind it with the dynamic puzzle — but the dynamic puzzle costs $2^{8}$–$2^{14}$ hashes, so a Sybil can bind any address it likes for microseconds. A self-attested address is exactly as trustworthy as the Sybil attesting it.
*   **Have the uploader attest each sampled downloader's observed address.** The uploader is the party under suspicion.
*   **Resolve leaf-class signers through the DHT.** They are not in it: leaves and `SYMMETRIC` nodes run in client mode (SOLUTION-037), and making them DHT servers would recreate the unreachable-guardian problem that decision removed.
*   **Discount or refuse receipts from leaf-class downloaders.** Relays whose job is the base-layer floor serve mostly leaves; penalising their receipts inverts the incentive the floor depends on.
*   **Weight receipts by the downloader's own standing** (a web of trust): leaves have rank 0 by definition, so the same relays earn nothing.

## Defects found during verification

*   The forgery arithmetic after SOLUTION-041: 100 leaf-class Sybils ($\approx 1$ s of hashing) signing one receipt per segment per tree yield $60 \times 6 \times 100 = 36{,}000$ distinct genuine-signed receipts per minute; at the $1.5$ Mbps stripe cap ($280$ KB) that is $\approx 10$ GB per 60 s, rank $\approx 220$. Every check of §5.2.2 passes. The ticket rated this Medium; as a statement about "contribution buys placement" it is the most important residual in the incentive layer, which is why the specification now says so rather than implying a bound it does not have.
*   The damage bound at honest parents is real only because the prefix cap is now defined at small $K$ (SOLUTION-042); under the old unconditional 5% a home relay's cap was undefined.
*   The dynamic-puzzle address binding was considered as a cheap verification hook and rejected on arithmetic: it binds an identity to *an* address, chosen by the solver, at negligible cost.

## The generalisable lesson

**A defence is only as real as its inputs at the node that applies it. When the input is an address and the protocol forbids self-declared addresses, the defence exists only where a third party has observed the address — and the specification must say what happens where none has.** Stating the gap honestly, with the bounds that remain, is the fix; a mechanism that pretends to close it is not.

## Residual risk

*   Forged rank buys quality: one enhancement slot per prefix per honest parent per tree, and interior status in every tree until the attacker's children leave. Both are bounded per prefix, and both are what an honest contributor with one prefix would also hold. Chapter 8 owes the measurement.
*   A structural fix — a downloader-address attestation witnessed by a party the verifier trusts (a guardian-witnessed session, or the uploader's own parent observing the child) — is a design question deferred to future work; none of the candidates examined here is both cheap and unforgeable.

## Validation owed (Chapter 8)

*   Enhancement-layer slots and multi-tree positions obtained by a single-prefix attacker with $S \in \{10, 100, 1000\}$ leaf-class Sybils at $N = 10^6$, against an honest single-prefix contributor.
*   Lazy `FIND_NODE` resolution rate of relay-class sampled signers and the revocation latency it implies.
