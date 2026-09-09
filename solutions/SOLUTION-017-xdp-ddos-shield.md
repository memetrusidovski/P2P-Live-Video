# SOLUTION-017: The XDP DDoS Shield

**Closes:** ISSUE-017 (Medium)
**Lives in:** `protocol/chapter7/7.2_ddos_shielding/1_token_bucket_math.md`, `2_xdp_kernel_ebpf.md`, `appendix_b_parameters.md`, `appendix_c_threat_model.md`
**Class:** Kernel data plane / DoS resistance

---

## The problem in one line

The prose claimed the eBPF program "checks S/Kademlia validation blocks against a kernel-resident hash map"; the C listing did nothing of the sort and could not, and its actual behaviour was inverted from the threat model — unknown sources fell through to `XDP_PASS` (so a spoofed flood, the real vector, passed unfiltered) while known peers were capped at 100 pps against a ~750 pps legitimate rate.

## The decision

A **two-tier default-deny allowlist with a real token bucket**, policing only the protocol's own UDP port:

* **Tier 1 — allowlisted peers.** User space inserts `(source IP → {rate, burst})` after the QUIC handshake and identity validation succeed; removes on disconnect. `RATE_PPS_PEER = 2000`, `BURST_PEER = 1000`, 65,536 entries.
* **Tier 2 — unknown sources.** A per-source handshake budget (10 pps, burst 20, 16,384-entry LRU) **and** an aggregate new-connection ceiling. Never an unconditional pass.

## The honest reframing that made the design possible

The prose was not just inaccurate, it was describing something infeasible. Once QUIC is established the payload is encrypted, and per-packet Ed25519 verification is orders of magnitude beyond an XDP program's budget. Stating the true contract is what produced a correct design:

> **Identity validation happens once, in user space. The kernel enforces the *result* of validation, never the validation itself.**

That is a weaker claim than the original prose and the only one that can actually be built. The sizing then follows from arithmetic rather than assertion: $6\text{ Mbps} \div (1024 \times 8)$ bits ≈ **733 pps** steady state, so 100 pps would have dropped ~87% of a healthy peer's traffic. And LRU for unknown sources specifically, so a spoofing flood evicts only other unknown sources and never an established peer.

## Four implementation bugs found in the C

Prose review would not have caught these; the listing has to be read as code.

**1. Unsigned underflow disables the limiter under load.** `delta = now - b->last_refill_ns` on a `BPF_MAP_TYPE_HASH` shared across RX queues. XDP runs concurrently on every CPU, so another CPU may already have advanced `last_refill_ns` past `now` — the subtraction wraps to ~$2^{64}$, the bucket refills to `cap` on every packet, and the rate limit silently stops existing. It fails open, under exactly the multi-queue flood it defends against.

**2. Multiplication overflow.** `delta * b->rate` exceeds u64 for a sufficiently idle entry. Clamped to the delta that already fills the bucket — safe and exact.

**3. IP options misparse the packet.** `struct udphdr *udp = (void *)ip + sizeof(*ip)` assumes a 20-byte IP header. The header is `ihl * 4`. Any packet with an IP option is misparsed, so the port check reads payload bytes — an attacker **evades the filter entirely by setting a single IP option**. Fixed to honour `ihl`, with explicit bounds for the verifier, plus a non-first-fragment drop (a fragment has no UDP header and this protocol never fragments).

**4. First contact granted `burst + 1` packets.** The fresh bucket was created with full `BURST_UNKNOWN` tokens and the triggering packet passed without spending one. Off-by-one, but it is the budget for the highest-volume adversarial path.

## The subtlest defect: "global" was per-CPU

```c
__uint(type, BPF_MAP_TYPE_PERCPU_ARRAY);   /* global_new */
```

`GLOBAL_NEW_PPS = 5000` is documented as "a global ceiling on new-connection work from all unknown sources." A percpu map gives **each CPU its own bucket**, so the real ceiling is $5000 \times N_{\text{cpu}}$ — 160,000 pps on a 32-core server, 32× the intended bound, on the counter whose entire job is bounding a spoofed-source flood that per-source limits cannot catch.

The fix keeps percpu (a shared atomic counter would cost a cross-CPU cacheline bounce on every packet, which is what XDP exists to avoid) and requires user space to load `GLOBAL_NEW_PPS / num_online_cpus()` per CPU. The accepted trade-off is now stated: uneven RX-queue hashing can leave one CPU exhausted while others idle.

## The generalisable lesson

**A per-CPU data structure changes the meaning of every constant stored in it.** The bug is invisible in the constant, invisible in the prose, and visible only where the map type meets the name. Any percpu BPF map holding a budget needs the division stated at the point of definition — done here in both the listing and Appendix B.

Second, more general: **a rate limiter that fails open is worse than none**, because it is trusted. Both clamps in `bucket_allow` exist to convert "approximate under concurrency" — which is an acceptable design choice — into "approximate but always bounded", which is the property actually required.

## Validation owed (Chapter 8 §8.1, containerised emulation)

* Measured drop rate for a legitimate peer at 750 pps steady state plus PULL bursts, confirming 2000/1000 has real headroom.
* Spoofed-source flood at $10^6$ distinct addresses: does the aggregate ceiling hold once the percpu division is applied, and does the LRU protect established peers?
* Multi-queue concurrency: verify the clamped bucket bounds throughput within an acceptable factor when the same source hashes across many RX queues.
* IPv6 needs a parallel filter keyed on the 128-bit source address; the current maps are IPv4-only and v6 traffic is passed to user space unfiltered.
