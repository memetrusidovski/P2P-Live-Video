# SOLUTION-088: Receipt Ledger Indexed by Canonical Key

**Closes:** ISSUE-088 (High, implementation)
**Lives in:** `src/crates/bs-core/src/node/incentive.rs` (`IncentiveState::receipts`, `on_receipt`, `theta_rate`), `src/crates/bs-core/src/node/rank.rs` (`RANK_PROOF` list)
**Class:** Implementation cost quadratic in fan-out where the specification's is linear

---

## The problem in one line

Every accepted receipt scanned the whole 60-segment ledger three times. That is invisible at ten children and 1.8 × 10^10 visits per second at the 10,000-child fan-out the specification permits.

## The decision

*   The ledger is a `BTreeMap` keyed by the canonical key (segment, tree, flags, downloader) of Ch5 §5.2.2. Replacement detection is one lookup, the window trim is one `split_off` at the window's first segment, and the rate sums one key range.
*   The `RANK_PROOF` list is a key-range iteration: the map's order is the canonical order the verifier checks, so no sort and no dedup.

## Why this and not the alternatives

*   **An incremental rate sum**: cheaper still, but `theta_rate` is read once per admission or proof, so a range sum over the window is already below the Ed25519 cost of the same node.
*   **A hash map plus a sorted index**: two structures to keep consistent for one order the proof needs anyway.

## Residual risk

*   Memory is unchanged: about 140 MB of receipts at 10,000 children. The spec's one receipt per child per tree per segment is the bound.

## Validation owed

*   A super-node simulator scenario at `K_v = 10,000` in one tree, checking receipt processing stays under one core.
