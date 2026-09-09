# 3. Consensus Eviction

Peers do not rely on a central server to track blacklists. Instead, they gossip **signed, evidence-carrying accusations** and each peer decides locally, from evidence it has itself re-verified, whether to evict a suspect.

## Accusations Carry Their Proof

An accusation that is only an opinion — "peer $X$, penalty $1.0$" — costs its sender nothing and cannot be checked by anyone who receives it. Under any aggregation that counts such opinions, an attacker with a few hundred cheap identities (Ch2 §2.2.2) can accuse the swarm's best relay and have every honest node sever it: one packet, ten thousand orphans. The mechanism intended to remove attackers would be the cheapest attack in the protocol.

An accusation is therefore admissible only if it is **self-contained**: it carries the specific signed statements that constitute the offence, and a receiver re-runs the check before counting it. Two evidence types are defined — the two the protocol can prove from signatures alone:

| `EvidenceType` | Evidence carried | What the receiver re-verifies |
| :---: | :--- | :--- |
| `0x01` `SYMMETRIC_TREE_PAIR` | Two `PROOF_OF_UPLOAD` receipts: one signed by $B$ naming $A$ as uploader, one signed by $A$ naming $B$, **same `TreeID`**, timestamps within 60 s, byte values within $10\%$ | Both signatures; same tree; same window; ratio (Ch5 §5.3.1) |
| `0x02` `EQUIVOCATION` | Two statements signed by the suspect that cannot both be true: two receipts for the same (segment, tree, uploader) with different bitmaps, or two `REGISTER_PEER` statements with the same timestamp and different class/trees | Both signatures by the suspect's key; the contradiction |

Block poisoning is deliberately **not** an evidence type: symbols and `BLOCK_PROOF`s are not signed by the relay that forwards them, so a receiver cannot prove to a third party *who* delivered a bad block. Poisoning is punished locally (disconnect and local ban, Ch7 §7.1.1), where the victim's own observation is sufficient. Subnet clustering (§2) is likewise a local discount, not an accusation, because the address mapping it relies on is not carried in any signed statement.

```text
REPUTATION_AUDIT_GOSSIP Frame (Type 0x22):
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|   Version     |  Type (0x22)  |          Payload Length       |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                      Accuser S/Kademlia NodeID (32 bytes)     |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
| Count (<= 2)  |  Reserved (0) |          Reserved (0)         |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|  per accusation:                                              |
|  [Suspect NodeID 32B][EvidenceType 1B][Reserved 1B]           |
|  [EvidenceLength 2B][Evidence ...][Timestamp 8B]              |
|  [Accuser Signature 64B over the accusation's preceding bytes]|
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

A frame carries at most **two** accusations ($\approx 600$ bytes each with two receipts of evidence), against the 65,535 the earlier layout allowed.

## Aggregation

A peer maintains, per suspect, the set of **verified** accusations it has received in the last $\tau_{\text{accuse}} = 10$ minutes. It evicts the suspect — severs active connections, drops it from its Passive Set and pools, refuses its joins — when

$$\left|\{\, \text{distinct accusers with verified evidence, rank} \ge r_{\text{accuser}} = 0.25 \,\}\right| \ \ge\ 3 \quad \text{and} \quad \text{they span} \ge 3 \text{ distinct } /24 \text{ (or } /48\text{) prefixes}$$

*   **Distinct accusers, not accusation count.** One accuser sending the same evidence ten times is one accuser.
*   **Accusers must have standing.** An accuser's rank is its $\Theta^{\text{rate}}/B$ as known to the receiver (a current parent or child, or a peer that has presented a `RANK_PROOF`); a fresh identity with no contribution history cannot vote. This is the cost of a fresh identity actually doing work: a Sybil can be minted in 10 ms, but a Sybil with rank $0.25$ has relayed a quarter of a stream for a minute.
*   **Three prefixes.** The Sybil bound of this protocol is address diversity, not proof-of-work (Ch2 §2.2.2); a single hosting block cannot reach the threshold alone.

Because every counted accusation was re-verified by the receiver, the threshold is a guard against *coordinated true* accusations being amplified beyond their evidence — not a guard against false ones, which never enter the count.

## Eviction Is Local and Bounded

Eviction is a **local** decision with a **bounded** duration: $\tau_{\text{ban}} = 10$ minutes on the first eviction, doubling on each repeat, capped at 24 hours, then forgotten. There is no swarm-wide permanent ban and no ban list is gossiped — only accusations are, and only with their evidence. A peer that receives an accusation and finds its evidence invalid **does not forward it**, and treats the accuser as having presented a forgery (local ban, Ch7 §7.1.1). Lying about a peer therefore costs the liar its standing at every honest node that checks — the cost to accuse that the earlier design lacked.

Rate limits close the remaining amplification: a receiver accepts at most **4 accusations per accuser per minute** and discards the rest unread, so a compromised high-rank node cannot flood the audit channel; and an accusation older than $\tau_{\text{accuse}}$ is not counted, so evidence cannot be hoarded and released at once.

The earlier design — an unsigned `(Suspect, PenaltyScore)` list, an undefined "consensus" over $\Theta_{\text{malicious}} \ge 0.8$, and an immediate, permanent, swarm-wide sever — had no aggregation rule, no evidence, and no cost to accuse. Against a multi-tree super node it was the highest-leverage packet in the protocol.
