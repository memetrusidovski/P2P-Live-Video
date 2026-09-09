# 1. Static and Dynamic Puzzles

To completely neutralize Sybil attacks (where an attacker spawns hundreds of thousands of fake nodes on a single machine), we bind every Node ID to a cryptographic keypair and require a significant expenditure of computational resources via Proof-of-Work (PoW).

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
def dynamic_puzzle_difficulty(swarm_size, node_class):
    if swarm_size < 50:        tier = 8    # 2^8  = 256 iterations, ~1 ms on mobile
    elif swarm_size < 1000:    tier = 10   # 2^10 = 1,024
    elif swarm_size < 100000:  tier = 12   # 2^12 = 4,096 — the classic default
    else:                      tier = 14   # 2^14 = 16,384 — hardened at scale

    # Leaf-class devices (battery/metered/onion-routed) solve one tier lower.
    # They are NOT exempt — see Ch1 1.2.5 on why PoW is universal.
    if node_class in (LEAF, LEAF_PRIVATE):
        tier -= 2
    return tier
```

No explicit difficulty field is carried on the wire: a proof solved at higher difficulty automatically satisfies every lower threshold, so verifiers simply check the hash against the tier required for the swarm size **they** observe, accepting one tier below to tolerate stale swarm-size reads during rapid growth (see Ch2 §2.2.3).

### Reconnect Fast Path (Mobile IP Changes)

A node whose IP changes but that can prove continuity of identity gets a discounted re-solve. If the node signs a fresh challenge nonce with the Ed25519 key of its existing (static-puzzle-backed) identity, **and** the new external IP falls in the same $/24$ IPv4 subnet (or $/48$ IPv6 prefix) as the previous one — the common case for DHCP renewal and cell-tower handover within one carrier — then:

$$C_2^{\text{effective}} = \left\lceil C_2 / 2 \right\rceil$$

A full-difficulty re-solve is always required when the subnet changes, preserving the anti-migration property the dynamic puzzle exists for.
