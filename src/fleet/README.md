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
