# ISSUE-069: RANK_PROOF receipt window and nonce are undefined for verifiers that are not the prover's parent

**Status:** OPEN
**Filed:** 2026-10-20 (surfaced by the reference implementation, `src/`, while implementing reputation auditing)
**Spec sections:** Ch5 §5.2.2 (commit and nonce), Ch5 §5.3.3 (proof attached to accusations), App D §D.4.17

## Problem

Ch5 §5.2.2 takes the receipts "from the trailing 60 segments" and the nonce from a manifest
newer than the commitment. It does not say whose live edge anchors the window, and the natural
reading (the newest manifest the prover holds) breaks for the receivers Ch5 §5.3.3 adds.

1. A proof attached to an accusation reaches every Active Set peer. Peers deeper than the prover
   lag it by a few segments and do not hold a nonce manifest taken at the prover's edge. The
   proof is unverifiable there and the accusation is dropped as unranked. In the simulator the
   publisher's own accusations were dropped by every receiver for this reason.
2. A verifier a few segments ahead of the prover anchors the 60-segment window on its own edge
   and rejects the prover's oldest receipts as outside the window.

## Options

1. **Anchor both on the commitment.** The nonce is a manifest two segments behind the prover's
   edge. The commitment is one segment before the nonce. The receipt window is the 60 segments
   that end at `CommitSegmentSeq`. Every receiver within a few segments of the prover holds the
   nonce, and the window is the same for every verifier.
2. **Fetch the nonce manifest** with `MANIFEST_REQUEST` when it is missing, within τ_retain.
   Adds a round trip to every gossip verification and does not fix the window anchor.

## Implementation decision (pending spec resolution)

Option 1. `Node::build_rank_proof` uses the manifest two segments behind the live edge as the
nonce and commits one segment before it. `Node::verify_rank_proof` takes the window from the
proof's `CommitSegmentSeq`. It refuses a commitment more than two segments ahead of its own edge.
