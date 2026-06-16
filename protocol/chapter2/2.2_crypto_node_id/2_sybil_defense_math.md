# 2. Sybil Defense Math and Verification Algorithm

## 2.2.1 Computational Cost Analysis

By forcing nodes to solve proof-of-work puzzles, the protocol shifts the cost of network entry from zero to a measurable CPU boundary.
*   **Static Puzzle ($C_1 = 16$):** Requires on average $2^{16} = 65,536$ hashing operations. This takes less than a few milliseconds on standard consumer hardware, representing a negligible one-time entry fee for a real user.
*   **Dynamic Puzzle ($C_2 = 12$):** Requires on average $2^{12} = 4,096$ hashes, taking less than a millisecond to recalculate upon an IP change.

However, an attacker attempting to flood the DHT with $1,000,000$ Sybil nodes must compute $(2^{16} + 2^{12}) \times 1,000,000$ hashes. Furthermore, because of the dynamic IP binding, the attacker cannot reuse the same IP address without their nodes being rejected under the subnet clustering limits.

---

## 2.2.2 S/Kademlia Identity Verification Code

The following Python-like pseudocode illustrates how the cryptographic ID is quickly verified by neighboring peers in constant time $O(1)$:

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
        
    # 3. Verify Dynamic Puzzle Difficulty (C2 zero bits prefix bound to IP)
    hash_dynamic = hashlib.blake3(hash_static + external_ip + dynamic_nonce).digest()
    val_dynamic = int.from_bytes(hash_dynamic, byteorder='big')
    if val_dynamic >= (1 << (256 - c2)):
        return False
        
    return True
```
