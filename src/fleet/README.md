# Fleet: many real nodes, one command

Runs one publisher and N viewer containers of `bsnode` on a docker bridge
network, with `tc netem` shaping each container's link, and collects every
node's output hash and starvation counters into a `verdict.json` in the same
schema the simulator produces.

**Status: scripts are complete but not runnable until M2 lands `bsnode publish`
and `bsnode watch` (see MILESTONES.md).** Nothing here has been executed yet.

```
fleet/up.sh --viewers 20 --profile lossy --file ~/sample.mp4
fleet/report.py --profile lossy          # writes results/fleet/verdict.json, exit 1 on failure
fleet/down.sh
```

Profiles (`profiles/*.env`): `clean`, `lossy` (40 ms +-10, 2 % loss), `cellular`
(120 ms +-40, 8 % loss, 8 Mbit/s). Add a profile by adding an env file.

Files:

| File | Role |
|---|---|
| `Dockerfile` | multi-stage: rust builder for `bs-node`, debian-slim runtime with iproute2 |
| `entrypoint.sh` | applies netem, runs `bsnode` as publisher or viewer, writes `/results/<host>/result.json` |
| `docker-compose.yml` | `publisher` + scalable `viewer` service, shared `results` volume |
| `netem.sh` | applies a profile with `tc qdisc ... netem` (needs `NET_ADMIN`, granted in compose) |
| `up.sh` / `down.sh` | scale up under a profile; tear down (`--keep-results` keeps the volume) |
| `report.py` | copies the results volume out and builds `verdict.json` |

The data-correctness check is the `hash_match_fraction` KPI: every viewer's
reassembled output must hash equal to the publisher's source file.


## Watching the fleet

Every container serves its player page and status API on port 8080 inside the
container. Compose publishes them to the host: the publisher on `localhost:8079`,
viewers on one port each from `8080-8199` (viewer N is `localhost:80(80+N)`,
`docker compose ps` shows the exact mapping).

- `fleet/dashboard.py` then `open http://localhost:9000/`: one row per viewer with
  state, depth per tree, chunks played and starved, blocks rejected, bytes, running
  Blake3, and an **integrity** column that compares the viewer's per-chunk,
  per-layer Blake3 checkpoints with the publisher's for the same segment and chunk.
  "OK (n layer-chunks)" means n layer-chunks matched the publisher byte for byte and
  none differed. Each row links to that viewer's own page.
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
