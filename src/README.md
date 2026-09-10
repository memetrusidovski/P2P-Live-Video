# BitStream reference implementation

Rust implementation of the protocol specified in [`../protocol/`](../protocol/),
plus the simulator, tooling and clients around it.

- [`ARCHITECTURE.md`](ARCHITECTURE.md): what each crate does and why the core is sans-I/O.
- [`MILESTONES.md`](MILESTONES.md): phases, exit gates, status.
- [`docs/TESTING.md`](docs/TESTING.md): the test levels and how failures explain themselves.
- [`docs/wire_coverage.md`](docs/wire_coverage.md): which frames are implemented.

Run everything:

```
scripts/check.sh          # fmt, clippy, tests, python cross-decoder, sim gates -> results/check.json
```

Individual pieces: `cargo test --workspace`, `cd py && uv run pytest -q`,
`cargo run -p bs-sim --profile sim -- run scenarios/<name>.toml`,
`fleet/up.sh --viewers 20 --profile lossy` (M2).

Layout: `crates/` (bs-wire, bs-crypto, bs-media, bs-core, bs-sim, bs-ingest,
bs-node, bs-player-api), `py/`, `clients/`, `fleet/`, `scenarios/`, `scripts/`, `docs/`.
