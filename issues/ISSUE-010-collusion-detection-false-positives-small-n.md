# ISSUE-010: Collusion Detection False Positives Flag Legitimate Peers at Small N

**Status:** Open  
**Priority:** Medium  
**Component:** Chapter 5 — Reputation Auditing  
**Affects:** Streams at N < ~20 peers  
**File:** `protocol/chapter5/5.3_reputation_auditing/1_graph_collusion_auditing.md`, `2_ip_subnet_penalties.md`, `3_consensus_eviction.md`

---

## Summary

The graph collusion auditing system detects fake PoU receipt exchanges by flagging symmetric bidirectional edges where `W(A→B) / W(B→A) ≈ 1.0` and `Degree(A) ≈ 1`. At small N (2–15 peers), legitimate peers naturally form bidirectional edges and have low degree — every peer pair looks statistically identical to a colluding Sybil pair. The detector will flag and potentially evict legitimate viewers, degrading or destroying the stream.

---

## Detailed Description

From `1_graph_collusion_auditing.md`:

> If the ratio of symmetric, bi-directional edge weights is close to 1.0:
> ```
> W(A→B) / W(B→A) ≈ 1.0  AND  Degree(A) ≈ 1
> ```
> the interaction is flagged as highly suspicious of collusion.

**Why this fires incorrectly at small N:**

In a legitimate small swarm at N=5:
- Viewer A relays tree 3 to Viewer B. A uploads Slice 3 to B (A→B PoU).
- Viewer B relays tree 1 to Viewer A. B uploads Slice 1 to A (B→A PoU).
- Both viewers are legitimate, contributing exactly one slice each to the other.

From the auditor's perspective:
- `W(A→B) ≈ W(B→A)` — both uploading roughly the same volume (1 slice at 1 Mbps each). Ratio ≈ 1.0. ✓ (suspicious flag)
- `Degree(A) = 1` — at N=5, A only has 1–2 other peers. Low degree. ✓ (suspicious flag)

Both conditions are satisfied. The auditor flags this as collusion. If the penalty score propagates via `REPUTATION_AUDIT_GOSSIP` and exceeds `Θ_malicious ≥ 0.8`, both A and B are evicted — legitimate viewers kicked from the stream.

**The eviction cascade:**

From `3_consensus_eviction.md`:

> When a peer X's Penalty Score in the local gossip consensus exceeds Θ_malicious ≥ 0.8, all well-behaved nodes immediately sever active connections.

At N=5, if 3 peers agree that A and B look like colluders (because they all see the symmetric edge), that consensus is easily reached. The eviction is irreversible in the current design — once gossip consensus tips over 0.8, A and B are network-banned.

**The IP subnet penalty (2_ip_subnet_penalties.md)** also fires incorrectly: if two viewers are on the same ISP (same /24 subnet — common in small streams where friends watch together), the subnet clustering penalty activates.

---

## Impact

- At N=2: the source-viewer pair is immediately flagged (the only edge in the graph is bidirectional via PoU receipts flowing both ways).
- At N=5–15: every peer pair that mutually relays is flagged. Small streams with close-knit audiences (e.g. Discord community streams) are particularly vulnerable.
- Eviction is permanent for the duration of the gossip consensus window — a falsely evicted viewer cannot rejoin until the reputation decays.

---

## Proposed Fix

Add a swarm-size guard to the collusion detection heuristics. The symmetric-edge flag should only activate above a minimum swarm size threshold:

```python
def check_collusion(A, B, swarm_size):
    ratio = W(A, B) / W(B, A)
    degree_A = get_degree(A)
    
    # Heuristics are only reliable at N > 20
    # Below this, symmetric edges are structurally expected
    if swarm_size < COLLUSION_MIN_SWARM_SIZE:   # = 20
        return NOT_SUSPICIOUS
    
    if abs(ratio - 1.0) < 0.1 and degree_A <= 2:
        return SUSPICIOUS
    
    return NOT_SUSPICIOUS
```

Additionally, weight the symmetric-edge detection by the number of unique counter-parties:

```
collusion_confidence = symmetry_score × (1 - 1/degree_A)
```

At `degree_A = 1` (only one peer interaction), `collusion_confidence = 0` regardless of symmetry. A node with only one peer cannot be meaningfully assessed for collusion — it has no choice but to interact with that one peer.

This change means collusion detection is effectively disabled below N=20, which is acceptable — colluding Sybils at N < 20 would need to fake only a handful of receipts, a negligible attack surface.

---

## Effort

Low. Single guard condition in the collusion check function. No wire format changes. The `REPUTATION_AUDIT_GOSSIP` frame format is unchanged; the change is in the local evaluation logic that determines what to include in the gossip.
