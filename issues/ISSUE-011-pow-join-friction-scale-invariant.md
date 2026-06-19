# ISSUE-011: Proof-of-Work Join Cost Is Identical at N=2 and N=1,000,000

**Status:** Open  
**Priority:** Low  
**Component:** Chapter 2 — S/Kademlia Cryptographic Node IDs  
**Affects:** Small streams; mobile viewers; frequent re-joins after IP change  
**File:** `protocol/chapter2/2.2_crypto_node_id/1_static_dynamic_puzzles.md`, `protocol/appendix_b_parameters.md`

---

## Summary

The S/Kademlia Proof-of-Work system requires every peer to solve a static puzzle (`C1 = 16` leading zero bits) and a dynamic IP-bound puzzle (`C2 = 12` leading zero bits) before joining. These values are hardcoded constants designed to make Sybil attacks prohibitively expensive at million-peer scale. At small N (2–50 peers), the Sybil threat is negligible — there is no economic incentive to Sybil-attack a 5-person stream — but every legitimate viewer still pays the same computational cost. Additionally, the dynamic puzzle must be re-solved every time a viewer's IP changes (mobile users, DHCP renewal), creating repeated join friction.

---

## Detailed Description

From `1_static_dynamic_puzzles.md`:

**Static puzzle:** Find nonce `N_static` such that:
```
Blake3(PK_node || N_static) ≤ 2^(256 - C1)     where C1 = 16
```

At `C1 = 16`, expected iterations = `2^16 = 65,536`. On a modern CPU at ~500 MHz Blake3 throughput, this takes roughly **130 microseconds**. Negligible.

**Dynamic puzzle:** Find nonce `N_dynamic` such that:
```
Blake3(Blake3(PK_node || N_static) || IP_external || N_dynamic) ≤ 2^(256 - C2)     where C2 = 12
```

At `C2 = 12`, expected iterations = `2^12 = 4,096`. At the same throughput: ~8 microseconds. Also negligible on a PC.

**However, this assumes a modern desktop CPU.** On:
- A low-end mobile device (ARM Cortex-A53, ~100 MHz Blake3): static puzzle ≈ 655ms, dynamic ≈ 40ms.
- A heavily throttled browser WebAssembly context: 3–10× slower.

More importantly: **the dynamic puzzle must be re-solved every IP change.** Mobile users switching between WiFi and cellular, or users on networks with short DHCP leases, may re-solve the dynamic puzzle many times per stream session. Each re-solve requires re-handshaking with the DHT and potentially re-doing the JOINING state sequence.

**The Sybil threat at small N:**

The PoW is designed to make it expensive to generate thousands of fake IDs. At N=5, an attacker gaining control of 1 extra node is already 20% of the swarm — but with only 5 legitimate users there is no meaningful economic gain from doing so. The PoW cost is the same whether the swarm is 5 or 500,000 peers.

---

## Impact

- Low-end mobile devices experience 1–3 second join delays from PoW.
- Mobile users with unstable IP addresses repeatedly pay the dynamic puzzle cost during a single session.
- At N=5, this friction is particularly noticeable — the join process takes longer than it takes the stream to start producing useful data.
- No functional failure, but user experience impact on mobile at small N.

---

## Proposed Fix

**Make C2 (dynamic puzzle) adaptive based on known swarm size, with C1 (static puzzle) unchanged.**

The static puzzle is a one-time cost and establishes long-term identity — keep it at C1=16 regardless of swarm size. The dynamic puzzle is re-solved on IP change and is the source of repeated friction — make it adaptive:

```python
def dynamic_puzzle_difficulty(swarm_size):
    if swarm_size < 50:
        return 8     # 2^8 = 256 iterations — trivial, ~1ms on mobile
    elif swarm_size < 1000:
        return 10    # 2^10 = 1024 iterations
    elif swarm_size < 100000:
        return 12    # 2^12 = 4096 — current default
    else:
        return 14    # 2^14 = 16384 — harder at scale where Sybil risk is real
```

The current swarm size is published in the DHT stream registration record (already extended for ISSUE-004). Joining peers read this value, compute the required difficulty, solve accordingly, and include the difficulty level used in their `VALIDATION` frame header so peers can verify against the correct threshold.

**For mobile IP-change handling specifically:**

Add a `RECONNECT` fast path: if a peer can prove it holds a valid static ID (by signing a fresh nonce with their Ed25519 private key), and their new IP is from the same /24 subnet as their previous IP, they are granted a reduced dynamic puzzle difficulty for re-join:

```python
if is_reconnect and same_subnet(old_ip, new_ip):
    C2_effective = C2 / 2   # half the bits required for subnet-local IP change
```

---

## Effort

Low-Medium. Requires the difficulty to be encoded in the `VALIDATION` frame and verified by receiving peers. The puzzle algorithm itself is unchanged — just the difficulty constant varies. The DHT registration record needs a `swarm_size` field (overlaps with ISSUE-004 work).
