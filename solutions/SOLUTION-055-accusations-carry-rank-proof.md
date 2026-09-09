# SOLUTION-055: Accusations Carry Their Accuser's Standing

**Closes:** ISSUE-055 (Low)
**Lives in:** `appendix_d_frame_registry.md` §D.4.18 (`RankProofLength`, attached `RANK_PROOF`), §D.4.17 (trigger); `protocol/chapter5/5.3_reputation_auditing/3_consensus_eviction.md` (*Accusers must have standing*, forwarding rule)
**Class:** A consumer (accuser rank) whose only producer (`RANK_PROOF` at a tree join) was on a different path from the input (gossip)

---

## The problem in one line

An accusation counted only from an accuser the receiver could rank, and the receiver could rank only its own tree children, so accusations arriving by gossip — the mechanism's whole distribution path — were verified, forwarded and counted by nobody.

## The decision

`REPUTATION_AUDIT_GOSSIP` carries the accuser's own `RANK_PROOF`, bound to it by `Prover NodeID = Accuser NodeID` and the accusation signatures. The proof's nonce is a source-signed manifest root, so it is verifiable by anyone who holds or can fetch that manifest, and a forwarder re-sends it unchanged. A receiver counts an accusation only if it can rank the accuser, and forwards only what it counted. The proof may be omitted only toward a current tree parent or child.

## Why this and not the alternatives

*   **A `RANK_PROOF_REQUEST` frame**: the receiver has no session with an accuser it learned of by gossip, so there is nobody to send the request to.
*   **Scope eviction to accusers the receiver already ranks** (the charitable reading): eviction then happens only at parents of three of the suspect's accusers, and the forwarding rule and rate limit protect a channel with no consumers. Stated as an option in the issue; rejected because the mechanism's cost had already been paid.

## Defects found during verification

*   The issue's claim that "the nonce check is receiver-relative" is wrong: the nonce is a public manifest root post-dating the commitment. That is precisely what makes attaching the proof sound — the issue's own preferred fix depended on the property it doubted.
*   Reach is bounded in time by $\tau_{\text{retain}} = 8$ s: an accusation whose proof's nonce manifest nobody downstream holds is dropped, not forwarded. Gossip crosses the swarm in a few hops, well inside that.
*   Frame size grows from $\approx 1.2$ KB to $\le 11$ KB; at 4 accusations per accuser per minute this is immaterial.

## The generalisable lesson

**When a credential is presented on one path and consumed on another, the credential travels with the message that needs it, or the consumer never sees it.**

## Residual risk

A forwarder cannot refresh a stale proof; long gossip paths lose accusations at the 8 s horizon. Acceptable: eviction is local and bounded by design, and the accuser re-sends with a fresh proof if it still cares.

## Validation owed (Chapter 8)

*   Fraction of gossip-received accusations that are countable at the receiver, as a function of hop count and $\tau_{\text{retain}}$.
