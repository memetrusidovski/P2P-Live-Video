# 2. Sybil Defense Math and Verification Algorithm

## 2.2.1 What Proof-of-Work Costs, Honestly

By forcing nodes to solve proof-of-work puzzles, the protocol shifts the cost of network entry from zero to a measurable — but small — CPU boundary.

*   **Static Puzzle ($C_1 = 16$):** $2^{16} = 65{,}536$ Blake3 hashes of a 40-byte input on average. At $\approx 0.15\ \mu$s per short hash that is **$\approx 10$ ms on one CPU core**. A commodity GPU sustains $10^9$–$10^{10}$ short hashes per second: **$10^4$–$10^5$ identities per second**.
*   **Dynamic Puzzle ($C_2 = 8$–$14$):** $2^{8}$–$2^{14}$ hashes — microseconds to a few milliseconds — re-solved on every IP change.

Earlier drafts described this as "significant expenditure" that "completely neutralizes" Sybil attacks, and Appendix C called 5,000 identities "computationally expensive". They are not: 5,000 identities is $\approx 50$ s on a laptop core and $\approx 0.05$ s on a GPU. Every argument in this specification that leans on "a fresh identity is expensive" was wrong in kind, and each has been re-derived (below).

**Raising $C_1$ does not change this.** A difficulty that costs a phone one second costs a GPU a millisecond; hash-based PoW cannot price identities for a defender whose adversary has better hardware than its users. A memory-hard puzzle (Argon2-class) would narrow the gap by one to two orders of magnitude but not close it, and is deferred to Chapter 8 as a measured question.

## 2.2.2 What Actually Bounds Sybils: Address Diversity

The operative Sybil defence is that **identities are bound to addresses and addresses are capped per prefix**:

1.  The dynamic puzzle is solved against the node's *observed* external IP (Ch2 §2.2.3), so an identity minted behind one address cannot be used from another without re-solving — and, more importantly, cannot be *used* at all except from an address the attacker actually controls.
2.  **Per-prefix caps** limit what any one address block can hold, at every place identities are counted:

    | Where | Cap | Reference |
    | :--- | :--- | :--- |
    | A node's k-buckets (eclipse) | exactly $1$ of $k = 20$ entries per bucket from one `/24` (IPv4) or `/48` (IPv6) — **unconditional** | Appendix C §C.2.1 |
    | A node's Active Set, parent set, and a relay's child slots per tree | $c_p(K) = \max\left(1, \lceil K / \min(P_{\text{obs}}, 20) \rceil\right)$ of a slot set of size $K$ | below; Ch1 §1.2.1 (*Distinct-Parent Rule*) |
    | A receipt bundle (`RANK_PROOF`) | $> 20\%$ of the *resolvable* signers from one prefix discounts the bundle; leaf-class signers cannot be resolved, so forged rank is bounded at the places rank is spent rather than in the bundle | Ch5 §5.3.2 |
    | Consensus eviction | accusers must span $\ge 3$ prefixes | Ch5 §5.3.3 |
    | Eviction cooldowns and the FIFO bootstrap queue | keyed on prefix as well as NodeID | Ch5 §5.1.3, §5.2.1 |

    **The cap is a share of observed diversity, with its rounding defined.** $P_{\text{obs}}$ is the number of distinct prefixes among every peer the node has itself observed — its k-bucket contacts, every Peer Record it has received, every session it has held — a locally verifiable count that a third party can raise only by controlling more prefixes and can lower only by eclipsing the node, which the unconditional k-bucket cap and the disjoint-path lookup exist to prevent. Once twenty prefixes have been seen, $c_p(K) = \lceil K/20 \rceil$: one of $c_a = 8$ Active Set slots, one of $M \le 6$ parents, one of a home relay's $5$–$10$ child slots per tree, $87$ of a super node's $1{,}734$. Below twenty, the cap relaxes in proportion — with $P_{\text{obs}} = 1$ there is no cap — because a swarm whose viewers share one `/24` (a dorm, a campus, an office, one carrier's CGNAT pool at small $N$) has nothing else to admit.

    An earlier draft stated an unconditional "$5\%$ of slots" for every set. Five percent of the slot counts this protocol actually has — $M \le 6$ parents, $8$ Active Set entries, $1$–$20$ children on a home relay — is less than one slot, so the unstated rounding decided everything: under floor no home relay could accept any child; under "at least one" a same-`/24` stream of ten viewers degenerated to a chain that hit $D_{\max}$ at the ninth and rejected the tenth in every tree, including $L_0$, forever. The k-bucket cap is left unconditional because it is the eclipse defence that makes $P_{\text{obs}}$ trustworthy in the first place, and $k = 20$ slots make it exactly one per prefix with no rounding question.

An attacker's power is therefore proportional to the number of **distinct `/24` or `/48` prefixes** it controls — a scarce, purchasable, traceable resource — not to its hash rate. The `/48` granularity for IPv6 is deliberate: a single customer is routinely allocated a `/64` or `/56`, so a per-`/64` cap would give one subscriber millions of "prefixes". Carrier-grade NAT cuts the other way — thousands of honest viewers share one IPv4 address — which is why the caps are on *slots held*, not on *connections attempted*, and why the XDP unknown-source budget is sized per address for a CGNAT population (Ch7 §7.2.1). At large $N$ the cap still bounds what one CGNAT prefix can be *placed*: summed over the relays of one tree, $\sum_v \lceil K_v(m)/20 \rceil \ge N_{\text{relay}}/M$, so a single prefix can hold at least $\approx 8\%$ of $N$ as children per tree at the reference numbers ($N_{\text{relay}} = N/2$, $M = 6$) and more where slots are plentiful. A carrier pool larger than that share of the swarm is capacity the forest cannot place; those viewers fall to the source reserve and to emergent-relay bridges. That limit is stated here rather than hidden: it is the price of a Sybil bound keyed on addresses, and Chapter 8 should measure how often real CGNAT populations reach it.

## 2.2.3 The Dependent Arguments, Re-Derived

Three mechanisms had been justified by identity cost and now rest on prefix diversity:

*   **FIFO bootstrap-queue re-entry** (Ch5 §5.1.3): a peer that burns an optimistic slot goes to the tail with a 30 s cooldown keyed on **NodeID and prefix**. Returning under a fresh NodeID from the same `/24` is still in cooldown.
*   **Reveal-before-prove exposure** (Ch5 §5.2.1): a non-signing downloader gets $\le 3$ segments of one stripe before eviction; the eviction is recorded against its prefix, and a prefix may hold at most $c_p(K_v(m))$ of a relay's child slots per tree — so the leak is bounded per prefix per cooldown, not per identity.
*   **Eclipse resistance** (Appendix C §C.2.1, Ch2 §2.1.2): 5,000 Sybils near a target in XOR space are cheap to *mint*; the one-per-prefix cap on k-bucket entries means they need $\ge 20$ distinct prefixes to fill even one bucket, and the disjoint-path lookup bound $P_{\text{hijack}} \le f^{\alpha}$ is stated in terms of the *prefix* fraction $f$ the attacker controls.

## 2.2.4 S/Kademlia Identity Verification Code

The following Python-like pseudocode illustrates how the cryptographic ID is quickly verified by neighboring peers in constant time $O(1)$. `c2` is the tier for the swarm size the verifier observes (Ch2 §2.2.1), and `external_ip` is the **observed** source address, never a self-declared one.

```python
import hashlib

def verify_node_identity(node_id, public_key, static_nonce, dynamic_nonce, external_ip, c1=16, c2=12):
    # 1. Verify NodeID is the hash of the Public Key and Static Nonce
    hash_static = hashlib.blake3(public_key + static_nonce).digest()
    derived_id = hash_static
    if derived_id != node_id:
        return False

    # 2. Verify Static Puzzle Difficulty (C1 zero bits prefix)
    val_static = int.from_bytes(hash_static, byteorder='big')
    if val_static >= (1 << (256 - c1)):
        return False

    # 3. Verify Dynamic Puzzle Difficulty (C2 zero bits prefix bound to the OBSERVED IP)
    hash_dynamic = hashlib.blake3(hash_static + external_ip + dynamic_nonce).digest()
    val_dynamic = int.from_bytes(hash_dynamic, byteorder='big')
    if val_dynamic >= (1 << (256 - c2)):
        return False

    return True
```
