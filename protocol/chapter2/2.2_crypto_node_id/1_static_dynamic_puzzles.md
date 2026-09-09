# 1. Static and Dynamic Puzzles

To make identities *rate-limited* rather than free, we bind every Node ID to a cryptographic keypair and to an IP address, and require a Proof-of-Work (PoW) for each. The puzzles do **not** make Sybil identities expensive in any absolute sense — see §2 for the numbers — and the protocol's Sybil bound is address diversity, not hashing. What the puzzles buy is that identity creation and IP migration cost *something* per instance, so they cannot be done at line rate by a script, and that an identity is pinned to the address it was minted at.

### The Static Puzzle (Key Generation)
A peer must generate a 256-bit public key $PK_{\text{node}}$ (Ed25519) and find a static nonce $N_{\text{static}}$ (an 8-byte integer) such that the SHA-256 (or Blake3) hash of their concatenation has a prefix of $C_1$ zero bits:
$$\text{Blake3}(PK_{\text{node}} \parallel N_{\text{static}}) \le 2^{256 - C_1}$$

where $C_1$ is the static difficulty parameter (typically $C_1 = 16$). This establishes a permanent, expensive cryptographic identity.

### The Dynamic Puzzle (IP Binding)
To prevent an attacker from pre-generating millions of static IDs on a supercomputer and migrating them to a target network, the peer must physically bind their Node ID to their active public external IP address. 

The peer must find a dynamic nonce $N_{\text{dynamic}}$ such that:
$$\text{Blake3}(\text{Blake3}(PK_{\text{node}} \parallel N_{\text{static}}) \parallel IP_{\text{external}} \parallel N_{\text{dynamic}}) \le 2^{256 - C_2}$$

where $C_2$ is the dynamic difficulty parameter (default $C_2 = 12$). This must be re-solved whenever the node's external IP address changes (e.g., cell tower handover), binding the cryptographic identity to a specific physical network location in real time.

### Adaptive Dynamic Difficulty

The static puzzle is a one-time identity cost and keeps a fixed difficulty ($C_1 = 16$) at every scale. The dynamic puzzle, by contrast, is paid *repeatedly* — on every IP change — and its cost lands hardest exactly where the Sybil threat is smallest: a low-end mobile device (ARM Cortex-A53) spends ~40 ms per dynamic re-solve and may re-solve many times per session as it hops between WiFi and cellular, yet nobody Sybil-attacks a 5-viewer stream. $C_2$ therefore scales with the known swarm size (read from the DHT Stream Record, Ch2 §2.3):

```python
C2_MIN = 8   # absolute floor: no combination of discounts may go below this

def dynamic_puzzle_difficulty(swarm_size, node_class, is_same_subnet_reconnect):
    if swarm_size < 50:        tier = 8    # 2^8  = 256 iterations, ~1 ms on mobile
    elif swarm_size < 1000:    tier = 10   # 2^10 = 1,024
    elif swarm_size < 100000:  tier = 12   # 2^12 = 4,096 — the classic default
    else:                      tier = 14   # 2^14 = 16,384 — hardened at scale

    # Discounts are alternatives, NOT cumulative: take the largest that applies.
    # Leaf-class devices solve one ladder rung (2 bits) lower. They are NOT
    # exempt — see Ch1 1.2.5 on why PoW is universal.
    delta_class     = 2 if node_class in (LEAF, LEAF_PRIVATE) else 0
    delta_reconnect = tier // 2 if is_same_subnet_reconnect else 0

    return max(C2_MIN, tier - max(delta_class, delta_reconnect))
```

### Discounts Do Not Stack

Three separate reductions to $C_2$ exist — the leaf-class rung, the same-subnet reconnect discount, and the verifier's staleness grace band — and each is individually justified. Applied multiplicatively they are not:

| Applied in sequence at $N \ge 10^5$ | Bits | Hashes |
| :--- | :---: | ---: |
| Base tier | 14 | 16,384 |
| − leaf rung | 12 | 4,096 |
| − verifier grace band | 10 | 1,024 |
| − reconnect halving | **5** | **32** |

A 512× reduction, at precisely the scale where the Sybil threat is real. An attacker declaring `LEAF` class and asserting same-subnet reconnects would face a puzzle costing 32 hashes.

Two rules prevent this, and both are normative:

1.  **Discounts are alternatives, not addends.** The effective difficulty is
    $$C_2^{\text{eff}} = \max\left(C_2^{\min},\ \text{tier}(N) - \max\left(\delta_{\text{class}},\ \delta_{\text{reconnect}}\right)\right), \qquad C_2^{\min} = 8$$
    with $\delta_{\text{class}} = 2$ for leaf classes and $\delta_{\text{reconnect}} = \lfloor \text{tier}/2 \rfloor$ when *both* the identity-continuity signature verifies and the subnet is unchanged.
2.  **The grace band belongs to the verifier, not the solver.** It exists so a verifier whose swarm-size read is *ahead* of the solver's does not reject an honest proof during rapid growth. It is applied to $\text{tier}(N)$ before any class discount, never composed with one, and a solver may not target it deliberately. A verifier accepts a proof meeting $\max\left(C_2^{\min},\ \text{tier}(N_{\text{observed}}) - 2 - \delta_{\text{claimed}}\right)$, where $\delta_{\text{claimed}}$ is honoured only when the frame's `NodeClass` (for $\delta_{\text{class}}$) or the continuity proof and subnet check (for $\delta_{\text{reconnect}}$) actually justify it.

The floor $C_2^{\min} = 8$ is what makes the reconnect discount safe to keep aggressive: the anti-migration property it might otherwise weaken is carried by the **same-subnet requirement and the signed continuity proof**, not by the puzzle's bit count. The puzzle only has to stay expensive enough that re-solving is not free to automate.

No explicit difficulty field is carried on the wire: a proof solved at higher difficulty automatically satisfies every lower threshold, so verifiers simply check the hash against the tier required for the swarm size **they** observe, accepting one tier below to tolerate stale swarm-size reads during rapid growth (see Ch2 §2.2.3).

### Reconnect Fast Path (Mobile IP Changes)

A node whose IP changes but that can prove continuity of identity gets a discounted re-solve. If the node signs a fresh challenge nonce with the Ed25519 key of its existing (static-puzzle-backed) identity, **and** the new external IP falls in the same $/24$ IPv4 subnet (or $/48$ IPv6 prefix) as the previous one — the common case for DHCP renewal and cell-tower handover within one carrier — then:

$$C_2^{\text{effective}} = \max\left(C_2^{\min},\ \text{tier}(N) - \lfloor \text{tier}(N)/2 \rfloor\right)$$

A full-difficulty re-solve is always required when the subnet changes, preserving the anti-migration property the dynamic puzzle exists for. This discount is **not** additive with the leaf-class rung — see "Discounts Do Not Stack" above; a leaf-class node performing a same-subnet reconnect pays the larger of the two reductions, not both.
