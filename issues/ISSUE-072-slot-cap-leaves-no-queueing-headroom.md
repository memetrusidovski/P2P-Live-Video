# ISSUE-072: The slot cap leaves a full relay no queueing headroom

**Status:** OPEN
**Filed:** 2026-10-22 (surfaced by the reference implementation, `src/`, in the symmetric-NAT gate)
**Spec sections:** Ch1 §1.2.1 §1.3 (slot count), Ch1 §1.1.5 §5.1 (the two budgets), App B (f_frame)

## Problem

`K_v(m) = ⌊(1 − r_pull)·u_v / (t_v·B_m·Ω_v)⌋` spends the whole tree budget on slots, and
the PULL reserve `r_pull·u_v` is spent on repairs. A relay that fills its slots and serves
its reserve runs its uplink at 100 %. Ω's `f_frame` covers framing, not the control plane
of bitfields, gossip, manifests and probes. Nothing covers the burst of a chunk's blocks to
every child at once. The queue then grows without bound and the children whose packets are
queued last starve, while the relay itself plays fine.

A 55 Mbps public relay at 28 children in the symmetric-NAT gate dropped 76 000 datagrams
in 120 s and its last-admitted children starved 70 % of their chunks. Relays in the other
gates never fill their slots, so the effect did not show before.

## Options

1. **Headroom in the slot count.** `K_v(m) = ⌊(1 − r_pull − h)·u_v / (t_v·B_m·Ω_v)⌋` with
   `h = 0.05`. Costs one slot in twenty at a large relay. The same relay then dropped
   17 000 datagrams and its children starved 1 %.
2. **Charge the control plane to Ω.** Raise `f_frame` to cover bitfields and gossip at the
   relay's fan-out. The overhead grows with the child count, so a single constant is wrong
   somewhere.
3. **Pace pushes across the chunk period.** Removes the burst but not the 100 % average.

## Implementation decision (pending spec resolution)

Option 1: `Params::slot_headroom = 0.05` in `Params::slots`.
