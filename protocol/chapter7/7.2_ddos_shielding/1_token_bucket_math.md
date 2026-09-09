# 1. Token Bucket Math and Shielding Relays

At a million-peer scale, DDoS attacks (such as UDP packet floods) can quickly exhaust user-space CPU resources by forcing the operating system to continuously execute context switches between kernel space and the P2P application. 

**Shielding Relays:** High-tier supernodes closest to the source communicate only through dynamic, rotating public relays, hiding their physical IPs from the general public.

**Token-Bucket Filters:** Nodes implement eBPF-based Token Bucket filters in the OS kernel. This rate-limits incoming UDP traffic before it reaches user space:

```text
  [ Incoming Packet Flood ] ---> [ NIC Driver: XDP / eBPF filter ] ---> [ Linux Kernel Socket ]
                                               |
                                     (Exceeds limit? Drop!)
```

## What the Kernel Filter Can and Cannot Do

The kernel filter **does not** verify cryptographic identity. It cannot: once a QUIC session is established the payload is encrypted, and per-packet Ed25519 verification is far beyond an XDP program's budget in any case. Identity validation happens exactly once, in user space, during the handshake (Ch2 §2.2).

The contract between the two layers is an **allowlist**:

1.  User space completes the QUIC handshake and validates the peer's S/Kademlia identity.
2.  On success it **inserts** `(source IP → {rate, burst})` into a kernel-resident map; on disconnect or eviction it **removes** the entry.
3.  XDP enforces a token bucket per map entry, and applies a strict default-deny budget to everything else arriving on the data port.

This is a weaker claim than "the kernel checks validation blocks," and it is the true one: the kernel enforces *the result* of identity validation, not the validation itself.

## Two-Tier Budget

**Tier 1 — allowlisted peers.** Each entry carries its own token bucket:
$$\text{tokens} \mathrel{+}= \Delta t \cdot \text{rate}, \quad \text{capped at burst}; \quad \text{spend } 1 \text{ per packet}$$
A packet arriving with no tokens available is dropped.

The rate must be sized to the media the peer legitimately sends. At $B = 6\text{ Mbps}$ carried in 1024-byte RaptorQ symbols:
$$\frac{6 \times 10^6 \text{ bits/s}}{1024 \times 8 \text{ bits/packet}} \approx 733 \text{ packets/s}$$

Steady state is therefore ~750 pps per active link. Adding headroom for PULL repair bursts and the 30% FEC ceiling gives $\text{RATE\_PPS\_PEER} = 2000$ with $\text{BURST\_PEER} = 1000$ — QUIC is bursty by design, so the bucket must tolerate a full burst without dropping. (The previously specified $100$ pps would have dropped roughly $87\%$ of a healthy peer's traffic.)

**Tier 2 — unknown sources.** Packets from IPs not in the allowlist are *not* passed unconditionally — that would leave the spoofed-source flood, the actual threat, entirely unfiltered. Instead they draw on two strict budgets:

*   A per-source handshake budget ($\text{RATE\_PPS\_UNKNOWN} = 10$, burst $20$) in a separate LRU map, enough for a legitimate QUIC handshake and pre-session DHT frames.
*   A global new-connection budget ($\text{GLOBAL\_NEW\_PPS} = 5000$) as a percpu counter, bounding total handshake work regardless of how many distinct spoofed sources appear.

Exhausting either budget results in `XDP_DROP`. Sizing: the allowlist holds $65{,}536$ entries (a super node's neighbor set plus churn headroom — the previous $1024$ could not hold it), and the unknown-source map is a $16{,}384$-entry LRU so that a spoofing flood evicts only other unknown sources, never established peers.
