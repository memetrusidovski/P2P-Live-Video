# BitStream reference implementation

Rust implementation of the protocol specified in [`../protocol/`](../protocol/),
plus the simulator, tooling and clients around it.

- [`ARCHITECTURE.md`](ARCHITECTURE.md): what each crate does and why the core is sans-I/O.
- [`MILESTONES.md`](MILESTONES.md): phases, exit gates, status.
- [`docs/TESTING.md`](docs/TESTING.md): the test levels and how failures explain themselves.
- [`docs/MULTI_MACHINE.md`](docs/MULTI_MACHINE.md): one publisher and swarms of viewers across machines, on a LAN or a VPN.
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


## Real network quick start

```sh
cargo build -p bs-node
# publisher: streams a file byte-exact as opaque data and is the DHT bootstrap seed.
# It prints its stream id.
./target/debug/bsnode publish --file testdata/generated/sample.mp4 --listen 127.0.0.1:4000 --start-delay-s 3
# viewers (any number), each writes the reassembled bytes and checks the hash
H=$(./target/debug/bsnode hash testdata/generated/sample.mp4)
./target/debug/bsnode watch --bootstrap 127.0.0.1:4000 --stream <stream id> --listen 127.0.0.1:0 --out /tmp/v1.bin --expect-hash $H

# real video: ffmpeg renditions as layers, watched in a browser
./target/debug/bsnode publish --ingest ffmpeg --input movie.mp4 --renditions 360p:800,720p:2500 --listen 127.0.0.1:4000
./target/debug/bsnode watch --bootstrap 127.0.0.1:4000 --stream <stream id> --listen 127.0.0.1:0 --player 127.0.0.1:8080
open http://127.0.0.1:8080

# docker fleet: one publisher + N viewers with netem, report in the simulator's verdict schema
fleet/up.sh --viewers 6 --file movie.mp4 --max-bytes 6000000 --duration 45 --profile lossy
python3 fleet/report.py --out results/fleet && fleet/down.sh
```

http://localhost:9000/ is the fleet dashboard.

The publisher keeps its key in `~/.bitstream/publisher.key`, so its stream id is the
same after a restart. `--identity FILE` names another key file. A viewer gets a new
random key at every start. `--seed N` derives a fixed key for tests and fleets: every
node with the same seed and port then shares one key.

Across machines: `bsnode publish --listen 0.0.0.0:4000` prints the join commands, and
`bsnode swarm --count 10 --results DIR` runs ten viewers on any other machine. See
[`docs/MULTI_MACHINE.md`](docs/MULTI_MACHINE.md).