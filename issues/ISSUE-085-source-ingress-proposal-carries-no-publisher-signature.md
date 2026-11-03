# ISSUE-085: RELAY_PROPOSAL(SOURCE_INGRESS) has no signature field and no rule binds the session identity to PK_Publisher, so a relay cannot verify that the proposer is the broadcaster

**Status:** Open
**Priority:** High
**Component:** Ch6 §6.3.3 steps 1–3 and the RELAY_PROPOSAL byte diagram; App D §D.3 row 0x30; Ch2 §2.3.1
**Affects:** Every `RELAY_CAPABLE` node; every stream whose broadcaster is behind NAT
**File:** `protocol/chapter6/6.3_emergent_relays/3_relay_frames.md`, `protocol/appendix_d_frame_registry.md`, `protocol/chapter2/2.3_stream_registration/1_publisher_genesis_key.md`

---

## Summary

Step 1 says the proposal is "signed with $SK_{\text{Source}}$ inside the QUIC session so the relay can verify it is the publisher". The byte diagram carries `Target NodeID`, `Port`, `TreeID` and `BindFlags`: 36 bytes and no signature. The QUIC session is authenticated by the proposer's node key, and nothing requires the source's NodeID to derive from $PK_{\text{Publisher}}$. A relay therefore has no check to run, and any identity that registers `Flags.SOURCE` can bind the swarm's ingress relays. The prose was amended and the layout was not (recurring pattern 3).

## Detailed Description

1.  Attack cost: one static puzzle, about 10 ms per identity (Ch2 §2.2.1). The attacker sends `RELAY_PROPOSAL(SOURCE_INGRESS)` to every `RELAY_CAPABLE` record.
2.  An accepting relay sets its assignment aside, takes the attacker as parent at depth 0 in every tree, registers `SOURCE_INGRESS`, and is scored at hop 0 by every joiner while forwarding nothing verifiable. Captured capacity: a $\ge 20$ Mbps `PUBLIC` node per proposal.
3.  No rule unbinds an ingress relay whose source stops delivering; the stale depth-0 record lives up to 135 s.
4.  The reference implementation authenticates the proposer from `source_record.flags.source`, a self-declared registration flag (`src/crates/bs-core/src/node/membership.rs` 106-113, `node/nat.rs` 256-261, `dht.rs` 742), and lets a gossip-delivered `PUBLIC` record with `Flags.SOURCE` replace a known `SYMMETRIC` source record.

## Impact

The backbone that a NAT-blocked broadcaster depends on can be captured for the price of one identity, and joiners route to the captured relays as depth 0.

## Proposed Fix

*   Add a 64-byte publisher signature over $(\text{Relay NodeID} \parallel K_s \parallel \text{Timestamp})$ to `RELAY_PROPOSAL` when `SOURCE_INGRESS` is set, verified against the Stream Record's `PublisherPubKey`; or require $\text{NodeID}_{\text{source}} = \text{Blake3}(PK_{\text{Publisher}} \parallel N_{\text{static}})$ and say so in Ch2 §2.3.1.
*   Define unbind on source loss (no block for $N$ segments: drop `SOURCE_INGRESS`, re-register).
*   Implementation: verify against `publisher_pubkey`, and never let a gossip-delivered `Flags.SOURCE` record replace a known source.
*   Touches Ch6 §6.3.3 diagram and steps, App D §D.3/§D.4 (0x30), Ch2 §2.3.1; `membership.rs`, `nat.rs`, `dht.rs`.

## Implementation status

The reference implementation now accepts a `SOURCE_INGRESS` proposal only from a session whose verified key hashes to the stream id (`Blake3(PublisherPubKey) = StreamID`), fetching the key with one `PING` round when it is unknown, and no longer lets a gossip-delivered `Flags.SOURCE` record replace a known source (`node/nat.rs`, `node/membership.rs`, 2026-11-03). The specification gap (no signature field, no binding rule) stays open.

## Effort

Small for the field; medium for the unbind rule.

Found by the NAT walk (`audits/traces/nat-and-relays.md`), which also found the implementation counterpart.
