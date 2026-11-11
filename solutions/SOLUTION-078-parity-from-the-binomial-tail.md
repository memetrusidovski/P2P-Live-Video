# SOLUTION-078: Parity Sized from the Binomial Tail, Loss Measured on Source Symbols

**Closes:** ISSUE-078 (High)
**Lives in:** Ch4 §4.2.2 rules 2 and 3, App B (`Overhead_fec`, `p_fail`), Ch1 §1.2.1 §1.3, Ch5 §5.2.3 (`LossRate`); `src/crates/bs-media/src/fec.rs` (`parity_for_loss`), `src/crates/bs-core/src/swarm.rs` and `node/incentive.rs` (loss measurement)
**Class:** A sizing rule that used the mean of a distribution whose tail decides the outcome

---

## The problem in one line

`E = ⌈2ρK⌉` covers the mean loss of a 16-symbol block, so 4.5 % of blocks per hop failed at 2 % loss. The implementation never saw real loss at all: its loss report was a constant set by the default parity.

## The decision

*   `E` is the smallest parity in `[1, ⌈0.30K⌉]` for which more than `E` of the `K + E` symbols are lost in at most `p_fail = 10⁻³` of the blocks. With `K = 16`: 1 below 0.3 % loss, 2 to 1 %, 3 at 2 %, 4 at 3 %, 5 from 5 %.
*   The child measures loss on the source symbols: the share missing when the block decoded. A source symbol that arrives later is taken back out. A block that PULL completed counts if its `BLOCK_PROOF` arrived. No wire change.
*   The FEC residue, `1 − (1 − p_fail)^depth`, is a named consumer of the PULL reserve.

## Why this and not the alternatives

*   **Raise `K` to 64**: halves the relative spread, but adds store-and-forward delay per hop (ISSUE-077) and changes the wire format.
*   **A parity byte on `BLOCK_PROOF`** so the child knows what was sent: a wire change for a number the source symbols already give.
*   **Counting all symbols past decode**: needs the sent parity and a closing time for each block. The source symbols need neither.

## Measured

Sixty viewers at 80 ms, every link starting at parity 1 (`internet_lossy_parity1` with the loss varied), share of chunks starved:

| Loss | Constant report (before) | Real loss, old rule | Real loss, new rule |
|---|---|---|---|
| 2 % | 0.48 % | 2.43 % | 0.09 % |
| 5 % | 1.54 % | 4.71 % | 0.29 % |
| 10 % | 3.44 % | 6.53 % | 1.34 % |

## Residual risk

*   Above 5 % loss the 30 % ceiling cannot hold the target: 1.4 % of blocks per hop fail at 10 % loss and 8.3 % at 15 %. Cellular links still lean on PULL.
*   Typical parity is now 2 to 3, not 1, so `Ω` is 1.22 to 1.29 on ordinary links. Slot counts there are lower than the worked examples at `Ω = 1.15`.
*   The loss byte reads 0 below 0.4 % loss, where the rule would ask for 2. The floor of 1 applies there.

## Validation owed

*   A core test that varies link loss and asserts the parity a parent sends follows it.
*   The same sweep at depth 7, to confirm the residue stays inside the PULL reserve.
