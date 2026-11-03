# Fleet: many real nodes, one command

Runs one publisher and N viewer containers of `bsnode` on a docker bridge
network, with `tc netem` shaping each container's link, and collects every
node's output hash and starvation counters into a `verdict.json` in the same
schema the simulator produces.

**Status:** runs end to end. The first `up.sh` builds the image (a release build of
`bsnode` inside Docker, several minutes); later runs rebuild only what changed. The
build context is `src/` minus the paths in `src/.dockerignore`: without that file the
context carried `target/` and the copy failed. Each
`up.sh` writes `.last_run.env`, and `report.py` reads it for the run id, profile,
seed and publisher start delay, so `python3 fleet/report.py` with no flags reports
the last run against the profile's starvation bound (clean 0.1 %, lossy 2 %, cellular
5 %, matching the simulator gates). Viewers exit after `--duration` (default 60 s);
the publisher keeps streaming until the file ends, so `fleet/down.sh` stops it.

```
fleet/up.sh --viewers 20 --profile lossy --file ~/sample.mp4 --max-bytes 25000000 --duration 90
fleet/churn.sh --fraction 0.3 --at 30     # optional: kill 30 % of the viewers after 30 s
sleep 80
fleet/down.sh --keep-results              # the publisher writes its report when stopped
fleet/report.py --profile lossy           # writes results/fleet/verdict.json, exit 1 on failure
fleet/down.sh
```

The hash check compares each viewer that started at segment 1 with the publisher's whole
stream. The stream must therefore end before the viewers do. `--max-bytes` bounds it
(25 MB is about a minute of the sample) and `--duration` outlasts it.

Size the stream to the fleet. Docker Desktop starts a hundred containers over two to
four minutes on a loaded laptop. A viewer that starts after the stream ended finds no
bootstrap, reports `bootstrap ...: no answer` and never plays. For 100 viewers pass
no `--max-bytes` (the whole 248 MB sample, 5.5 minutes) with `--duration 330` and
`--no-ports`. Publishing a host port per viewer is the slow part of the start; without
the ports the status pages are reachable only from inside the fleet network. Viewers do
not write their output file unless `BS_KEEP_OUTPUT=1`; the hash needs no file, and a
hundred copies of the stream fill the results volume.

The publisher has the fixed address 10.77.0.2 on the fleet network and viewers bootstrap
by that address. Docker's embedded DNS answers "not found" for part of a hundred-container
start, which took most viewers down before they ran.

`churn.sh` is Ch8 §8.3 scenario A on real nodes. It records the killed containers in
`.churned`; `report.py` scores the survivors only and adds a `survivors_stalled` KPI
(a survivor must not starve more than 10 % of its chunks after the churn).

`BS_FLEET=1 scripts/check.sh` runs the lossy profile with that churn as a gate after
the simulator gates. It needs docker and `fleet/media/sample.mp4` (or `BS_FLEET_FILE`).

Profiles (`profiles/*.env`): `clean`, `lossy` (40 ms +-10, 2 % loss), `cellular`
(120 ms +-40, 8 % loss, 8 Mbit/s). Add a profile by adding an env file.

Files:

| File | Role |
|---|---|
| `Dockerfile` | multi-stage: rust builder for `bs-node`, debian-slim runtime with iproute2 |
| `entrypoint.sh` | applies netem, resolves the bootstrap with retries, runs `bsnode` as publisher or viewer, writes `/results/<host>/result.json` |
| `docker-compose.yml` | `publisher` + scalable `viewer` service, shared `results` volume |
| `docker-compose.ports.yml` | one host port per viewer for the status pages; left out by `up.sh --no-ports` |
| `netem.sh` | applies a profile with `tc qdisc ... netem` (needs `NET_ADMIN`, granted in compose) |
| `up.sh` / `down.sh` | scale up under a profile; tear down (`--keep-results` keeps the volume) |
| `report.py` | copies the results volume out and builds `verdict.json` |

The data-correctness check is the `hash_mismatches` KPI: every viewer that started at
segment 1 and starved no chunk must hash equal to the publisher's source file. A viewer
with a starved chunk has a hole in its output; its loss is scored by the PSR gate, not
counted as corruption. `report.py` keeps the raw results under `results/fleet/raw` and
lists every viewer under `facts.per_viewer`.


## Watching the fleet

Every container serves its player page and status API on port 8080 inside the
container. Compose publishes them to the host: the publisher on `localhost:8079`,
viewers on one port each from `8080-8199` in start order (`docker compose ps` shows
which viewer has which port). `up.sh --no-ports` skips the viewer ports.

- `fleet/dashboard.py` then `open http://localhost:9000/`: one row per viewer with
  state, depth per tree, chunks played and starved, blocks rejected, bytes, running
  Blake3, and an **integrity** column that compares the viewer's per-chunk,
  per-layer Blake3 checkpoints with the publisher's for the same segment and chunk.
  "OK (n layer-chunks)" means n layer-chunks matched the publisher byte for byte and
  none differed. Each row links to that viewer's own page. `--publisher host:port`
  and `--viewers host:lo-hi,host:lo-hi` point it at nodes on other machines
  (see `docs/MULTI_MACHINE.md`).
- A viewer's page (`http://localhost:8081/` etc.) shows its status panel and, in
  ffmpeg mode (`up.sh --ingest ffmpeg`), plays the video the swarm delivered to it
  through Media Source Extensions with a rendition selector.
- At exit every container writes `/results/<hostname>/result.json` (bsnode's
  `RunReport`); `fleet/report.py` folds them into the simulator's verdict schema and
  compares each viewer's whole-stream hash with the publisher's source hash.

Two independent checks are therefore always on: every block is Merkle-verified
against the publisher's signed manifest before it is used or forwarded (a corrupt
block never reaches the page), and the checkpoints/report prove end to end that
what the viewer assembled is what the publisher sent.

```sh
fleet/up.sh --viewers 8 --file ~/Downloads/example.mp4 --ingest ffmpeg --renditions 360p:800,720p:2500 --duration 600
python3 fleet/dashboard.py &      # http://localhost:9000/
open http://localhost:8081/       # a viewer playing the video
python3 fleet/report.py --start-delay-s 8 && fleet/down.sh
```

## Profiles and parity

`profiles/<name>.env` sets the netem shape and `BS_PARITY`, the RaptorQ parity symbols
every node adds per 16 KB block (clean 1, lossy 4, cellular 5). With one parity symbol a
2 % loss link loses a block whenever two of its 17 symbols drop, which starves roughly a
fifth of the chunks; four parity symbols absorb it. Adaptive per-link parity from
receipts is M3 work. Each `up.sh` run first tears down the previous one (`down -v`) and
writes results under `/results/<run-id>/`; `report.py` reports the newest run unless
`--run-id` is given.
