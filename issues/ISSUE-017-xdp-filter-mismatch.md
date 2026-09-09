# ISSUE-017: XDP Filter Doesn't Match Its Prose and Would Drop Legitimate Traffic

**Status:** Resolved  
**Priority:** Medium  
**Component:** Ch7 §7.2 — DDoS Shielding  
**Affects:** DDoS protection effectiveness; legitimate peer traffic  
**File:** `protocol/chapter7/7.2_ddos_shielding/1_token_bucket_math.md`, `2_xdp_kernel_ebpf.md`

---

## Summary

Ch7.2.1's prose claims the eBPF program "checks S/Kademlia validation blocks against a kernel-resident hash map." The C listing in 7.2.2 does nothing of the sort — and couldn't (post-handshake QUIC payloads are encrypted; per-packet Ed25519 in XDP is infeasible). Worse, its actual behaviour is inverted from the threat model:

- Unknown source IPs fall through to `XDP_PASS` — a spoofed-source flood (the real DDoS vector) passes unfiltered.
- `RATE_LIMIT_PPS 100` applied to known peers drops legitimate stream traffic: 6 Mbps in 1024 B symbols ≈ 750 pps steady-state.
- It enforces a minimum inter-packet interval, not a token bucket — QUIC's constant micro-bursts would be shredded.
- `MAX_PEERS 1024` cannot hold a super node's neighbor set.

## Proposed Fix

Two-tier **default-deny allowlist + real token bucket** on the protocol's UDP data port:

1. **Allowlisted sources:** userspace inserts `(source IP → {rate, burst})` into a `BPF_MAP_TYPE_LRU_HASH` after QUIC handshake + identity validation; removes on disconnect. XDP enforces a real token bucket (`tokens += Δt·rate`, cap at burst, spend 1/packet). `RATE_PPS_PEER = 2000`, `BURST_PEER = 1000` (covers 750 pps media + pull bursts + 30% FEC ceiling). Map: 65,536 entries.
2. **Unknown sources:** per-source handshake budget (10 pps / burst 20, LRU 16,384 entries) plus a global new-connection budget (5,000 pps, percpu counter) — never unconditional pass, closing the spoofed-flood hole while admitting first-contact handshakes and pre-session DHT frames.
3. Rewrite the 7.2.1 prose honestly: identity validation happens once in userspace; the kernel enforces the resulting allowlist. Note IPv6 needs a parallel v6-keyed map.

---

## Resolution

Rewrote the shield as a two-tier default-deny allowlist with a real token bucket:

- `protocol/chapter7/7.2_ddos_shielding/1_token_bucket_math.md` — rewritten: an explicit "What the Kernel Filter Can and Cannot Do" section stating that identity validation happens once in user space and the kernel enforces only its result (no per-packet signature checking); the userspace↔XDP allowlist contract; token-bucket equations; the 750 pps derivation from 6 Mbps ÷ 1024 B and the resulting RATE_PPS_PEER = 2000 / BURST_PEER = 1000 sizing (noting the old 100 pps would have dropped ~87% of healthy traffic); Tier-2 per-source and global budgets closing the spoofed-flood hole; map sizing rationale.
- `protocol/chapter7/7.2_ddos_shielding/2_xdp_kernel_ebpf.md` — C listing replaced: `struct bucket {tokens, last_refill_ns, rate, burst}`, a real `bucket_allow()` refill/spend helper, destination-port filter so unrelated traffic is untouched, allowlist lookup (LRU-safe HASH, 65,536 entries), unknown-source path requiring BOTH a per-source LRU budget (16,384 entries) and a percpu global new-connection ceiling, `XDP_DROP` as the default outcome on the data port, and an explicit IPv6 caveat comment.
- `protocol/appendix_b_parameters.md` — new B.1.4 "Kernel Filter" table with all six constants.
- `protocol/appendix_c_threat_model.md` — DoS row updated to describe the allowlist-based filter accurately.
