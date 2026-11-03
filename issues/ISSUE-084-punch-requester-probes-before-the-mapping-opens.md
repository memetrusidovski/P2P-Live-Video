# ISSUE-084: The punch procedure sends the PROBE before the target's PING can open the mapping, and a SYMMETRIC requester can never learn when to re-send

**Status:** Open
**Priority:** Medium
**Component:** App D §D.4.14; Ch1 §1.2.2 §2.2 step 2 (punch before probe); Ch1 §1.2.5 reachability table; Ch6 §6.1.1
**Affects:** Every probe of a `CONE` candidate reached through a referrer; every `SYMMETRIC` requester
**File:** `protocol/appendix_d_frame_registry.md`, `protocol/chapter1/1.2_multi_forest_overlays/2_parent_selection_algorithm.md`, `protocol/chapter1/1.2_multi_forest_overlays/5_node_classes.md`, `protocol/chapter6/6.1_udp_hole_punching/1_nat_permutation_matrix.md`

---

## Summary

§D.4.14 says the target "sends a `PING` to the requester's address, opening its NAT mapping; the requester's `PROBE` that follows within 1 s then passes", at a cost of "one extra round-trip". The pseudocode of Ch1 §1.2.2 §2.2 sends the `PROBE` immediately after `send_punch_request`. Read literally, the `PROBE` reaches the target's NAT before the target's `PING` has left, and is dropped at any RTT above zero. Read charitably, the requester waits for the `PING`; the text does not say so, and a `SYMMETRIC` requester never receives that `PING` because its own NAT allocated no mapping for the target.

## Detailed Description

1.  RTT 80 ms, requester $V$, target $P_1$ (`CONE`) referred by guardian $G_1$: `PROBE` at $P_1$'s NAT at $t + 40$ ms; $P_1$'s `PING` leaves at $t + 80$ ms and reaches $V$ at $t + 120$ ms; the probe timeout is 200 ms. Measured cost 2.5 RTT, stated $+1$ RTT.
2.  No rule says to probe again after the `PING` arrives; the candidate is marked unreachable ("TIMEOUT: unreachable: drop the record").
3.  A `SYMMETRIC` requester's NAT drops the `PING` (per-destination mapping), so no trigger ever exists; Ch6 §6.1.1's success figure for that pair cannot be reached by this procedure, and every symmetric leaf facing `CONE`-only slots becomes a bridge customer.

## Impact

Guardian-referred `CONE` parents are dropped on first contact, which at the reference RTT is every one of them; the emergent-relay pool is loaded with peers the punch was meant to serve directly.

## Proposed Fix

*   Define the requester's `PROBE` as sent on receipt of the target's `PING` (`CONE` or `PUBLIC` requester), and for a `SYMMETRIC` requester as a blind re-send after $RTT_{\text{referrer}} + \tfrac{1}{2} RTT$.
*   Restate the cost as about 2.5 RTT in §D.4.14 and the §1.2.5 table; fix the pseudocode ordering in Ch1 §1.2.2 §2.2.
*   Touches App D §D.4.14, Ch1 §1.2.2, §1.2.5, Ch6 §6.1.1.

## Effort

Small.

Found by the NAT walk (`audits/traces/nat-and-relays.md`).
