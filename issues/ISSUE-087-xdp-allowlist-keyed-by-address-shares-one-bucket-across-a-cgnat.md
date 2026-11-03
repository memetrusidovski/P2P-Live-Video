# ISSUE-087: The XDP allowlist is keyed by IPv4 address, so children behind one CGNAT share one 2,000 pps bucket and lose the entry when any one of them disconnects

**Status:** Open
**Priority:** High
**Component:** Ch7 §7.2.1 allowlist contract; Ch7 §7.2.2 `peer_allowlist` key; App B B.1.4; Ch8 §8.3 scenario C
**Affects:** Every relay with more than about 18 children behind one address; the 75 % NAT population of Ch8 §8.3 C
**File:** `protocol/chapter7/7.2_ddos_shielding/1_token_bucket_math.md`, `protocol/chapter7/7.2_ddos_shielding/2_xdp_kernel_ebpf.md`, `protocol/appendix_b_parameters.md`

---

## Summary

User space inserts "(source IP → {rate, burst})" on handshake and removes the entry on disconnect (Ch7 §7.2.1); the map key is `__u32 saddr` (§7.2.2). The rate is sized for one peer. $n$ children behind one CGNAT address share that bucket, and the first of them to disconnect removes the entry for all of them, dropping the rest to the 50 pps unknown tier.

## Detailed Description

1.  Inbound from one child: about 108 pps of QUIC ACKs on a 216 pps push (ack-every-2), or about 22 pps with ACK-frequency tuning, plus receipts, pulls and probes: 25–115 pps.
2.  `RATE_PPS_PEER` = 2,000: 18 to 90 children behind one address saturate the bucket; ACKs are dropped in the driver, QUIC declares loss, the push is throttled, and dropped receipts evict the children after 3 segments.
3.  The prefix cap admits up to $\lceil K_v / 20 \rceil$ children per /24 per tree: 500 at $K_v = 10{,}000$, far above the 18–90 bound.
4.  Fallback tier on entry removal: 50 pps, burst 100, shared by every remaining child behind the address.

## Impact

Clean-link children behind a shared address starve and are evicted by their own parent's shield. Scenario C (75 % behind NAT or CGNAT) cannot pass as specified.

## Proposed Fix

*   Key the allowlist by `(saddr, sport)`: a NAT allocates a distinct external port per child flow.
*   Or refcount the address entry in user space and set its rate to $n \times$ `RATE_PPS_PEER`.
*   State which direction the 2,000 pps protects and size the inbound rate for the parent direction (ACKs, receipts, probes).
*   Touches Ch7 §7.2.1–§7.2.2, App B B.1.4.

## Effort

Small.

Found by the super-node walk (`audits/traces/super-node-obligations.md`); confirmed by an independent verifier.
