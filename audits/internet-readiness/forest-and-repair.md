# Forest and repair on lossy, high-RTT links

Area: `forest-and-repair`. Scope: parent liveness and eviction, join retry and shed,
repair after parent loss, adaptive parity, the PULL zone, and the simulator's PSR.
Code: `src/crates/bs-core/src/node.rs`, `node/incentive.rs`, `node/pull.rs`,
`swarm.rs`, `params.rs`; driver `src/crates/bs-node/src/driver.rs`; KPIs
`src/crates/bs-sim/src/kpi.rs`. Date: 2026-11-09.

Three simulator runs back the findings (45 s, 60 relays, 3 trees, 3 % loss, seed 11):

| Run | RTT (ms) | parity | SJL mean / p95 (s) | PSR | repair median / p95 (s) | layer sheds |
|---|---|---|---|---|---|---|
| `scenarios/lossy_links.toml` (stock) | 10 to 160, jitter 2 | 5 | 2.8 / 4.2 | 0.01 % | 0.47 / 4.6 | 81 |
| same, Internet RTT | 80 to 250, jitter 30 | 5 | 5.3 / 10.5 | 0.11 % | 1.77 / 2.46 | 43 |
| same, `bsnode` default parity | 80 to 250, jitter 30 | 1 | 15.4 / 32.8 | 0.36 % | 0.56 / 0.56 | 91 |

The two custom scenarios are in
`/private/tmp/claude-275978885/-Users-memet-rusidovski-dev-P2P-Live-Video/6fbe9a9c-fa60-43c5-9a16-ac1119f58359/scratchpad/sim/`
(`lossy_highrtt_p1.toml`, `lossy_highrtt_p5.toml`); results are under
`src/results/lossy_highrtt_p1/` and `src/results/lossy_highrtt_p5/`. Copy them into
`src/scenarios/` to keep them.

## What works today

- Parent liveness per tree: any frame from the parent resets `last_rx`
  (`node.rs:1975-1981`); after `τ_ping` of silence the child sends one PING, after
  `τ_evict = max(200 ms, 2·τ_ping + SRTT + 4·RTTVAR)` it evicts and starts a probe
  round (`node.rs:2920-2935`, `params.rs:296-303`). Parents heartbeat idle children
  every `τ_ping` (`node.rs:2971-2988`).
- SRTT and RTTVAR are updated RFC 6298 style from the child's own PING/PONG pairs
  (`node.rs:2146-2158`).
- Data-stall guard: a parent that heartbeats but pushes nothing for 2 s is treated
  as lost (3 windows for a fresh or PENDING parent); a relay that stays unparented
  for 2 s drains its children with `CAPACITY` so they repair themselves
  (`node.rs:2772-2815`).
- Cycle guard at probe time (candidates deeper than our own depth, or equal depth
  while we hold a parent, are skipped, `node.rs:885-897`) and again on ACCEPTED
  (`node.rs:1156-1176`).
- Shed rule: two failed rounds on a non-base subscribed tree shed that layer and
  everything above it; WARMING exempts at most 4 rounds (ISSUE-063); a round of pure
  timeouts is a discovery failure, not a shed signal (`node.rs:984-1065`).
- Adaptive parity plumbing exists end to end: the child's receipt carries a loss
  byte (`incentive.rs:300-320`), the parent maps it through
  `E = max(1, ⌈clamp(2ρK, 0.05K, 0.30K)⌉)` and re-encodes per child
  (`incentive.rs:123-128`, `apply_loss_report` at `incentive.rs:495`,
  `node.rs:2594-2620`). The same core code runs in `bsnode`.
- PULL zone: bitfields every `τ_gossip`, scheduler every `τ_sched` over chunks within
  `W_pull = clamp(τ_evict_max + 6·SRTT_max, 1.5 s, 2.0 s)` of their deadline
  (`node/pull.rs:172-182`, `params.rs:306-309`). At 3 % loss the stock gate pulls
  2,477 blocks with PSR 0.01 %.
- Steady-state PSR excludes backfill chunks (`node.rs:1779-1798`,
  `kpi.rs:294-308, 436-443`).

## What breaks on the Internet

1. **No candidate over 200 ms RTT is ever usable.** `begin_probing` sets the round
   deadline at `now + probe_timeout` (200 ms, `node.rs:760`, `params.rs:208`) and
   `on_probe_response` drops any answer that arrives after `finish_probing` ran
   (`node.rs:797-813`: no tree is `Probing`, so `matched = None`). The NEIGHBOR
   deadline is `3 × probe_timeout = 600 ms` counted from before the QUIC handshake
   (`node.rs:954`); at 250 ms RTT the 1.5-RTT open plus one NEIGHBOR round trip is
   625 ms. A viewer in Europe watching a broadcaster in North America sees only the
   same-continent relays, and before any exist it sees nobody. In the Internet-RTT run
   102 of 410 rounds answered nothing; SJL went from 2.8 s to 5.3 s mean with the same
   parity. Worse: a non-public node counts those rounds as silent and after
   `bridge_after_failed_rounds` asks for a relay bridge (`node.rs:1014-1020`), so a
   slow path is treated as a NAT block.
2. **The loss report is a constant, so parity never adapts.** The child records
   `symbols_received` as the collector count at decode time, which is K = 16 the
   moment the block completes (`swarm.rs:532-536`, `fec.rs:149-155`), and divides
   against `K + default_parity` (`incentive.rs:280-285`). With `bsnode`'s default
   parity 1 every receipt reports 1 lost of 17 (loss byte 15, 5.9 %) whatever the
   link does, and the parent sets E = 2 on every child. In the parity-1 run 180 of
   224 `parity_changed` events are `loss 15 → parity 2`; the rest are lower because
   datagrams outran the BLOCK_PROOF on the stream, not because the link was cleaner.
   At 3 % loss and E = 2, 3.6 % of blocks per hop fail the push and go to the PULL
   zone; at 5 % it is 5.9 % per hop, 21 % at depth 4 (ISSUE-078 covers the spec's
   rule; this is the measurement under it). The stock gate hides this with
   `default_parity = 5`, where the same constant reads as 5 of 21 and E stays 5.
3. **Repair costs more than the PULL zone holds on a 200 to 250 ms path.** There is
   no ROSTER and no Deputy election; `lose_parent` runs a full probe round, a
   1.5-RTT session open and a NEIGHBOR round (`node.rs:1946-1967`, known:
   `audits/2026-11-02-tick-walkthrough.md` impl-core-loop F "repair has no fast
   path", ISSUE-074). With `τ_evict = 200 ms + 3·SRTT` (RTTVAR seeded at SRTT/2,
   `node.rs:1181-1190`) eviction alone is 950 ms at 250 ms; the measured repair median
   was 1.77 s and p95 2.46 s against `W_pull` clamped at 2.0 s. Every block of that gap
   is pulled or starved; PSR rose to 0.11 % (0.36 % with parity 1), above the Ch8
   target of 0.1 %.
4. **The parent RTT is the PROBE's RTT and stays there.** App B says `τ_evict` uses
   "the connection's QUIC RTT estimator". `bsnode` reads quinn's `path.rtt` only into
   `Status` (`driver.rs:1632`); core has no RTT input. The estimator moves only when
   the child PINGs, which it does only after `τ_ping` of parent silence, and parents
   heartbeat before that. After a Wi-Fi to cellular migration (QUIC keeps the
   connection) a 20 ms-seeded path keeps `τ_evict = 260 ms`, one chunk period, so any
   260 ms jitter spike evicts the parent and triggers the full re-join of item 3.
   Cellular links spike past that routinely. The simulator never shows it: jitter is
   2 ms and RTT never changes mid-run (`bs-sim/src/scenario.rs:258-266`).
5. **Shed and unshed oscillate every 10.8 s under any top-layer shortage.**
   `unshed_attempt` raises `top_layer` before any parent exists and probes at once;
   two failed rounds 250 and 500 ms later shed again (`node.rs:1112-1136,
   1042-1066`). Ch1 §1.1.5 §5.3 rule 5 says the re-join goes through the 5 s migration
   loop and the layer is restored only when all of its trees have parents. In the
   stock gate 28 of 60 nodes shed L2 four times each at 7.2, 18.1, 28.8 and 39.6 s.
   Each cycle sends a starved re-query (`node.rs:1023-1025`), which the source reads
   as a fold trigger (ISSUE-076 cluster), and a `Deliver` with `layers_subscribed`
   raised for a layer that is not arriving.
6. **The simulator's PSR is not the spec's.** Chunks before the first push in every
   wanted tree are backfill and excluded (`node.rs:1779-1798`); a relay whose
   assigned tree has not pushed yet has every chunk excluded; a chunk counts as `ok`
   with one layer complete (`node.rs:2875`), so a missing subscribed layer never
   counts. Known: tick-walk impl-core-loop F2 (0.39 % by the impl, 1.14 % by the spec
   denominator in churn_storm). The fleet report inherits the same counters.

## Bugs

### 1. Receipt loss byte is a constant that depends on parity, not on loss

- Severity: High
- Where: `src/crates/bs-core/src/node/incentive.rs:280-285`,
  `src/crates/bs-core/src/swarm.rs:532-536`
- Reproduce: `cargo run -q -p bs-sim --profile sim -- run <scratchpad>/lossy_highrtt_p1.toml`,
  then `grep parity_changed src/results/lossy_highrtt_p1/events.jsonl`: 180 of 224
  are `loss 15, parity 2`. A core test that pushes K + 1 symbols with no loss and
  asserts the next receipt's `loss_rate == 0` fails (it reports 15).
- Fix sketch: keep counting symbols for a block after it decodes until the push
  window for that chunk closes (or the next chunk's first block arrives), and record
  `symbols_received` then; take the denominator from the parent's announced parity
  (a byte on BLOCK_PROOF, or the highest ESI seen for the block plus one) instead of
  `default_parity`.
- Effort: medium. Self-contained: no (how the child learns the sent parity is a wire
  decision).

### 2. Probe deadline is a 200 ms constant and late answers are dropped

- Severity: High
- Where: `src/crates/bs-core/src/node.rs:760` (deadline), `:797-813` (late response
  ignored), `src/crates/bs-core/src/params.rs:208`
- Reproduce: `lossy_highrtt_p5.toml` (parity 5 so only RTT differs from the stock
  gate): SJL mean 5.3 s, p95 10.5 s, 102 of 410 rounds with zero answers. A core test
  that answers a PROBE after 210 ms and expects the candidate in the round fails.
- Fix sketch: deadline = `max(probe_timeout, 2 × best RTT estimate so far)` where the
  estimate comes from the bootstrap PONG or DHT RPCs; keep candidates whose answer
  arrives before the next round starts. Do not count a round short of the RTT-scaled
  deadline as silent for the bridge rule (`node.rs:1014-1020`).
- Effort: small. Self-contained: yes.

### 3. NEIGHBOR deadline is 600 ms from before the handshake

- Severity: Medium
- Where: `src/crates/bs-core/src/node.rs:954`
- Reproduce: a core test with a 250 ms RTT session open (1.5 RTT) and a 250 ms
  NEIGHBOR round trip: the candidate is dropped at 600 ms and `request_next` moves
  on. Known as a Low, unverified item in the tick walk (impl-core-loop); verified.
- Fix sketch: start the deadline at `SessionOpened` and set it to
  `τ_sched + RTT` as Ch3 §3.3.1 specifies.
- Effort: small. Self-contained: yes.

### 4. Parent RTT is seeded from the PROBE and quinn's estimator never reaches core

- Severity: Medium
- Where: `src/crates/bs-core/src/node.rs:1181-1190` (seed, RTTVAR = SRTT/2),
  `:2146-2158` (update only on the child's own PONG), `src/crates/bs-node/src/driver.rs:1632`
  (quinn `path.rtt` read into `Status` only), `src/crates/bs-core/src/io.rs` (no RTT
  input). Known: tick walk impl-core-loop, "seeded once from the probe"; verified.
- Reproduce: a core test that attaches at 20 ms, then delivers blocks for 10 s while
  the driver's RTT is 150 ms: `τ_evict` stays 260 ms, and a 300 ms gap between
  chunks evicts the parent.
- Fix sketch: add `Input::RttSample { peer, srtt, rttvar }` fed by the driver from
  `Connection::stats().path` once per tick or per chunk, and use it for the parent
  link; keep the PING path for the simulator.
- Effort: small to medium. Self-contained: no (core input plus driver plus sim).

### 5. `τ_evict` sits inside one chunk period on fast-seeded paths

- Severity: Medium
- Where: `src/crates/bs-core/src/params.rs:296-303`, `src/crates/bs-core/src/node.rs:2925`
- Reproduce: a sim scenario with `jitter_ms = 300` and intra-region RTT 20 ms shows
  `parent_lost reason=timeout` on every spike (the stock model has 2 ms jitter and
  never shows one). The spec table has the same shape (240 ms at 20 ms), so part of
  this is a spec question (tick walk S4: the RTTVAR behind the table is unstated).
- Fix sketch: floor `τ_evict` at one chunk period plus the measured jitter (RTTVAR
  from bug 4), and count a PING answered inside the window as liveness even when
  blocks are late.
- Effort: small. Self-contained: no (constant choice against App B).

### 6. Unshed restores the layer before any parent and retries at the join cadence

- Severity: Medium
- Where: `src/crates/bs-core/src/node.rs:1112-1136` (`unshed_attempt`), `:1042-1066`
  (shed after two rounds), `:1023-1025` (starved re-query per cycle),
  `src/crates/bs-core/src/params.rs:211,213`
- Reproduce: `cargo run -q -p bs-sim --profile sim -- run scenarios/lossy_links.toml`;
  `layer_shed` events: 81 in 45 s, 28 nodes, four per node 10.8 s apart. The gate
  passes because the base layer is never shed.
- Fix sketch: keep `top_layer` until every tree of the layer has a parent (track a
  `pending_layer`), probe those trees at the 5 s migration cadence, do not send the
  starved re-query from an unshed attempt, and double the hysteresis on each repeat.
- Effort: small. Self-contained: yes.

### 7. Join retry backoff base is 250 ms

- Severity: Low
- Where: `src/crates/bs-core/src/node.rs:1059`, `src/crates/bs-core/src/params.rs:213`
- Known: tick walk flash-crowd F6 and impl-core-loop (250/500/1000 ms against the
  1.0 s floor the spec's `FAILURE_RETRY_BACKOFF` handling implies); verified at the
  lines above. Two shed decisions land 0.34 to 0.7 s apart, which is what drives bug 6.
- Fix sketch: base 1 s, cap 4 s, repair rounds exempt as now.
- Effort: small. Self-contained: yes.

### 8. Simulator PSR excludes starvation the spec counts

- Severity: Low
- Where: `src/crates/bs-core/src/node.rs:1779-1798` (`is_steady_chunk`), `:2875`
  (`ok = layers_complete >= 1`), `src/crates/bs-sim/src/kpi.rs:294-308, 436-443`
- Known: tick walk impl-core-loop F2; verified. The gates and the fleet report both
  read the narrow number.
- Fix sketch: emit both numbers (`psr_steady` and `psr_spec` with every played
  chunk in the denominator) and a `layer_miss` ratio for chunks short of a subscribed
  layer; gate on `psr_spec`.
- Effort: small. Self-contained: yes.

## Missing features

| Feature | Why it is needed | Size |
|---|---|---|
| ROSTER and Deputy election (Ch1 §1.2.3, Ch3 §3.3.1) | Repair is a full probe, session and NEIGHBOR cycle: 1.77 s median at Internet RTT against a 2.0 s `W_pull`. Needs ISSUE-074 settled (the 45 ms timer cannot work over 22 ms RTT). | 6 days |
| RTT-scaled timers from the QUIC estimator | Bugs 2, 3, 4 and 5 share one cause: every deadline is a constant chosen for a LAN. One `RttSample` input and one `rtt_budget(peer)` helper replace them. | 2 days |
| Loss measurement the parent can act on | Bug 1 plus a parity signal from parent to child (BLOCK_PROOF byte or ESI inference) and a test that varies link loss and asserts E follows it. | 3 days |
| Internet network model in `bs-sim` | No scenario has RTT above 160 ms, jitter above 2 ms, loss with parity 1, RTT steps or address migration. Add jitter bursts, mid-run RTT changes and a `mobile` profile, then gate `lossy_highrtt_p1` and a mobile scenario. | 3 days |
| Resumption tickets for 1-RTT re-attach | The spec's repair timeline assumes a 1-RTT re-attach; the driver holds no tickets (tick walk S7). | 2 days |
| Spec-definition PSR and layer-miss KPI in sim and `fleet/report.py` | Bug 8; the fleet verdict and the Ch8 target must measure the same thing. | 1 day |

## Open questions

- Should `τ_evict` be floored at one chunk period? App B's table and the impl both go
  below it at 20 ms RTT; on a mobile path that is a spurious eviction per jitter
  spike. The answer sets the constant in bug 5.
- How does a child learn the parity its parent sends? The ESI range of received
  symbols gives a lower bound only; a byte on BLOCK_PROOF is exact but is a wire
  change (App D).
- The cycle guard compares against `last_depth`, which is the pre-loss depth. An
  orphan relay with children refuses every candidate deeper than that depth; in a
  tree that grew since, that can exclude every free slot. How long does an orphan
  hold its children before the 2 s drain in that case, and is the drain the intended
  outcome?
- The parity-1 run's SJL (15.4 s) is three times the parity-5 run's at the same RTT.
  The join path is not in this area. One candidate coupling, unconfirmed: warm-up
  gating (a relay advertises `K_avail = 0` until its first full segment, which takes
  longer when 3.6 % of pushed blocks go to the PULL zone). Needs a trace.
