# ISSUE-067: RANK_PROOF is sent after the NEIGHBOR it should inform

**Status:** OPEN
**Filed:** 2026-10-20 (surfaced by the reference implementation, `src/`, while implementing rank admission)
**Spec sections:** App D §D.4.17 (RANK_PROOF), Ch1 §1.2.2 (Rank Admission Rule), App D §D.4.3 (NEIGHBOR)

## Problem

App D §D.4.17 says the proof is "sent on the QUIC control stream immediately after a
`NEIGHBOR(TreeID > 0)`". Ch1 §1.2.2 says the parent decides admission from the rank the proof
establishes. On one ordered stream the NEIGHBOR arrives first, so a parent that follows the text
must either decide without the rank (every joiner ranks 0 and case 2 never fires) or hold the
request until the proof arrives, with a timeout for joiners that send none. Neither is specified.

## Options

1. **Send the proof before the NEIGHBOR.** Same stream, same cost, and the parent decides in one
   step; a joiner without receipts sends nothing. A proof not followed by a NEIGHBOR within a
   short window is discarded.
2. **Hold the NEIGHBOR** for one RTT (or until the proof) at the parent, with the rank-0 decision
   on timeout. Adds a timer and a second path through admission.
3. **Carry the rank claim in the NEIGHBOR** and fetch the proof only when it would change the
   decision (contended enhancement tree). Lowest cost; changes the NEIGHBOR layout.

## Implementation decision (pending spec resolution)

Option 1: `Node::send_neighbor` sends the RANK_PROOF first when the tree carries an enhancement
layer and the joiner holds receipts. A verified proof stands for `rank_proof_ttl` (10 s) at the
verifier, which covers the NEIGHBOR that follows and any retry.
