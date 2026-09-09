# SOLUTION-013: The Block/Symbol Verification Boundary

**Closes:** ISSUE-013 (High)
**Lives in:** `protocol/chapter4/4.1_segment_serialization/2_progressive_blake3_hashing.md`, `4.2_fec_raptorq/2_systematic_dispersion.md`, `3_symbol_packet_layout.md`, `4.3_hybrid_push_pull/1_buffer_sliding_timeline.md`, `appendix_d_frame_registry.md`
**Class:** Data plane / zero-trust invariant

---

## The problem in one line

Merkle verification and PoU receipts operated on 16 KB blocks while FEC and loss recovery operated on 1024 B symbols, and nothing said how the two units map — so "verify before forwarding" and "heal gaps with RaptorQ symbols" were describing incompatible data paths.

## The decision

**One RaptorQ source block per Merkle block: $K = 16$ source symbols of 1024 B. `SBN` *is* the Merkle block index.**

The block is the verification boundary. Symbols are a link-local loss-recovery encoding — never individually verified, never forwarded raw. Interior relays operate:

> **decode → Merkle-verify → re-encode → push**

with parity sized per downstream link from that child's reported loss. Proofs travel once per block: inline in `BLOCK_TRANSMISSION` (0x10) on the pull path, in `BLOCK_PROOF` (0x13) ahead of the symbols on the push path.

## Why per-symbol proofs were rejected

A Merkle proof for a 16-leaf tree is up to 4 sister hashes (128 B), and in a full segment tree ~6 (192 B), against a 1024 B payload — roughly 19% overhead, permanently, on the highest-volume frame in the protocol. Per-symbol verification also buys nothing the block boundary does not already provide: a corrupted symbol fails to reconstruct a block that hashes correctly, so the corruption is caught one block later at the same hop. Paying 19% for that is a non-starter.

## The property that makes decode→verify→re-encode affordable

This is the part worth remembering, because the naive implementation is roughly a thousand times more expensive than the correct one.

**RaptorQ is systematic**, so source symbols ($\text{ESI} < K$) are the block's raw 1 KB slices, unmodified. When all 16 arrive — the overwhelmingly common case, which is exactly why parity is only 5–30% — the relay reconstructs the block by **concatenating them**. There is no decoding work at all. The decoder runs only when a source symbol is actually missing.

Per-hop cost accounting:

| Step | Cost |
| :--- | :--- |
| Accumulate $\ge K$ symbols | ~13 ms at 10 Mbps — **dominant** |
| RaptorQ decode | **zero** when nothing was lost |
| Blake3 of 16 KB + Merkle path | ~5 µs |
| Re-encode parity | microseconds |

The cost of the zero-trust guarantee is therefore one block of store-and-forward latency per hop, not per-hop cryptographic or coding work. An implementation that runs a full decode pass on every block at every hop pays a cost the design does not require — at a million peers that is the difference between a negligible term and a dominant one. Now stated explicitly in the spec.

## Defect found: parallel repairs collided

Because every relay decodes to the *identical* verified block, and RaptorQ generation is deterministic in $(\text{block}, \text{ESI})$, symbols from different relays are interchangeable. That is a genuinely useful property — a child can combine symbols from several parents.

It also creates a hazard the spec did not address. A child repairing block $B_i$ from two neighbours receives **the same** repair symbols from both, because each responder naturally starts generating at $\text{ESI} = K, K{+}1, \dots$. Parallel repair — the thing the mesh pull path exists to do — was self-defeating: pulling from three neighbours got you one neighbour's worth of new information.

`PULL_REQUEST` carries `MissingSymbolCount` but no indication of which symbols the requester holds, and widening it would force the requester to enumerate them. The fountain-code property makes that unnecessary — *any* $K$ distinct symbols decode — so responders take disjoint ranges instead:

$$\text{ESI}_{\text{base}}(v) = K + \left(\text{Blake3}(NodeID_v) \bmod (2^{16} - K - 256)\right)$$

Collisions become a birthday coincidence in a ~65,000-wide space. A child pulling from several neighbours now accumulates distinct symbols and reaches $K$ in the minimum number of packets, with **no protocol field added** — the fix is entirely in how responders choose ESI.

## The generalisable lesson

**When a coding layer is deterministic, uncoordinated senders will produce identical output — which is either a feature or a bug depending on whether you wanted redundancy or diversity.** Here it is both: identical *source* symbols make cross-parent combining sound, while identical *repair* symbols make parallel repair useless. The resolution keeps determinism where it helps and derives per-sender divergence where it hurts. Worth checking anywhere else the protocol has multiple peers independently generating the same derived data.

## Validation owed (Chapter 8)

* Measured per-hop relay latency, separating symbol accumulation from decode, to confirm the systematic no-op path is what implementations actually hit.
* Repair efficiency (distinct symbols received per PULL) when pulling from 2–4 neighbours, with and without derived ESI bases.
* Whether $K = 16$ is the right block/symbol ratio: larger $K$ improves coding efficiency but raises the store-and-forward term at every hop.
