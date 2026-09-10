# Testing Plan

The test framework exists so that an agent (or a human) can take a failure
report and fix the defect without re-deriving context. Every level below
answers four questions on its own: what was expected, where the spec says so,
how to reproduce it, and what the node was doing when it went wrong.

## Single entrypoint

```
scripts/check.sh            # everything, writes results/check.json
just check                  # same thing if `just` is installed (brew install just)
```

`results/check.json` has one entry per step: `name`, `status` (`pass`, `fail`,
`skip`), `seconds`, `repro` (the exact command to re-run that step alone) and, on
failure, `tail` (the last 40 lines of output). CI uploads `results/` as an
artifact, so a red build is a JSON file, not a scrollback.

## Level 1: unit and property tests per crate

Every crate has unit tests next to the code and integration tests under
`crates/<crate>/tests/`. Property tests use `proptest` with a fixed case count.
Test names and doc comments cite the specification section they enforce
(for example `/// Ch4 sec. 4.1.2: Merkle proofs verify for every leaf`). A failing
test therefore names the protocol text to open.

```
cargo test --workspace
cargo test -p bs-wire                 # one crate
cargo test -p bs-crypto merkle        # one test name filter
PROPTEST_CASES=4096 cargo test -p bs-wire --test roundtrip   # deeper property run
```

Proptest prints the failing seed and persists it under `proptest-regressions/`;
commit that file when a regression is found so the case stays in the suite.

## Level 2: golden vectors shared with Python

`crates/bs-wire/tests/vectors/frames.json` holds one hex encoding per frame
built from fixed field values (`crates/bs-wire/tests/golden.rs`). Two
independent decoders must agree with it:

```
cargo test -p bs-wire --test golden           # Rust codec
cd py && uv run pytest -q                     # Python decoder, written from the byte diagrams
```

Regenerate the vectors only after an intentional layout change, and say so in
the commit message:

```
BS_UPDATE_VECTORS=1 cargo test -p bs-wire --test golden
```

A vector diff that was not intended is a wire regression. The Python decoder
is deliberately written from Appendix D, not transliterated from the Rust, so a
layout misreading in one implementation is caught by the other.

## Level 3: core state-machine tests

`bs-core` has no I/O, so a test is a script of `Input`s with explicit `now`
values and an assertion on the `Output`s. This is where protocol behaviour is
pinned: join handshake and rejection reasons, warm-up gating, heartbeat and
eviction timing, drain path, manifest sequence window, depth propagation.

```
cargo test -p bs-core
```

Each test constructs a `Node` with `Params::for_simulation()` and a fixed RNG
seed. Timers are advanced explicitly, so there is no wall-clock dependence.

## Level 4: simulator scenario gates

Each `scenarios/*.toml` declares KPI thresholds. `check.sh` runs every scenario
and treats a non-zero exit as a failed gate.

```
cargo run -q -p bs-sim --profile sim -- run scenarios/baseline_100.toml --out results/baseline_100
cargo run -q -p bs-sim --profile sim -- explain --node 37 --segment 41 --in results/baseline_100
```

Outputs per scenario under `results/<name>/`:

| File | Content |
|---|---|
| `verdict.json` | `passed`, per-KPI value / threshold / pass, invariant violations, seed, `repro` |
| `kpis.json` | PSR, SJL, CDO, mean and max depth per tree, repair-time distribution, bytes by frame type, output-hash equality per node |
| `events.jsonl` | one structured event per line: `t_us`, `node`, `kind`, fields |
| `topology.json` | forest snapshots at configured times |

Invariants are checked after every simulated event. A violation halts the run
and the verdict records the node, tree, segment and the last 20 events on that
node. The invariants:

1. verify-before-forward: no block leaves a node unless Merkle-verified against a signed manifest
2. depth <= D_max in every tree
3. exactly one parent per subscribed tree
4. trees are edge-disjoint
5. no symbol is emitted before its chunk manifest is signed
6. heartbeat gap on a healthy link never exceeds tau_evict
7. the live edge is monotone non-decreasing
8. advertised K_avail never exceeds K_v

`explain` renders one node's timeline for one segment: parent per tree, when
each manifest and block arrived, when the deadline passed, and the repair
events in between. It turns "PSR was 0.4 percent" into a specific sequence of
events.

Every scenario carries a `seed`; the verdict repeats it and the `repro` command
re-runs that exact scenario with that seed.

## Level 5: real network

Two-node localhost test (M2), part of `cargo test`:

```
cargo test -p bs-node --test localhost
```

Docker fleet (M2) for tens of real nodes with netem-shaped links:

```
fleet/up.sh --viewers 20 --profile lossy
fleet/report.py results/fleet          # same verdict.json schema as the simulator
fleet/down.sh
```

Every viewer writes its reassembled output hash; the report compares them with
the publisher's. Hash equality is the end-to-end data-correctness check at
every level, from a unit test in `bs-media` up to the fleet.

## Coverage

```
cargo install cargo-llvm-cov
cargo llvm-cov --workspace --html     # target/llvm-cov/html/index.html
```

Uncovered lines in `bs-core` are the first place to add a Level 3 test.

## Conventions

- Test names and doc comments cite the spec section (`Ch1 sec. 1.2.2`, `App D sec. D.4.8`).
- All time and randomness are injected; nothing reads a clock or a thread RNG.
- A test that needs a decision the spec does not make cites the `ISSUE-NNN` that records it.
- Fast tiers (Levels 1 to 3) must complete in under thirty seconds; the 100-node baseline scenario in under one minute of wall time. Longer scenarios run nightly.
