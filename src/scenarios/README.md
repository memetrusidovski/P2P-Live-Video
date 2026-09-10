# Simulator scenarios

Each `*.toml` here is a simulator gate run by `scripts/check.sh`. The schema the
simulator (`bs-sim`) reads:

```toml
name = "baseline_100"
seed = 1                         # every RNG in the run derives from this
duration_s = 60                  # virtual seconds

[media]
source = "synthetic"             # "synthetic" | "file"
file = "testdata/sample.mp4"     # when source = "file"; streamed as opaque bytes
ladder = "reference"             # "reference" (1.5/1.5/3.0 Mbps) | "single:<kbps>" | inline [[media.layers]]
forest_size = 3                  # M; 0 = derive from the relay-count ladder

[nodes]
count = 100                      # viewers (the publisher is extra)
publisher_upload_kbps = 50000
relay_fraction = 0.7             # rest are LEAF
upload_kbps = { dist = "lognormal", median = 10000, sigma = 0.6, min = 2000, max = 100000 }
regions = 4                      # latency model: region coordinates, base RTT by distance

[join]
schedule = "poisson"             # "poisson" | "burst" | "linear"
rate_per_s = 5                   # poisson/linear
burst_at_s = 5                   # burst

[network]
base_rtt_ms = { intra_region = 20, inter_region = 80 }
jitter_ms = 5
loss = 0.0                       # per-link packet loss probability
egress_queue = true              # model uplink saturation as queueing

[[churn]]                        # any number of events
at_s = 30
kind = "leave"                   # "leave" | "crash" | "join_burst" | "demote"
fraction = 0.3                   # of relays (leave/crash/demote) or count for join_burst
select = "relays_depth_le_2"     # "random" | "relays" | "relays_depth_le_2" | "leaves"

[kpis]                           # gate thresholds (Ch8 sec. 8.2)
psr_max = 0.001
sjl_max_ms = 1500
cdo_max = 0.02
depth_mean_max = 7
depth_max = 8
repair_p99_max_ms = 1000
hash_match = true                # every viewer's output hash equals the source hash

[params]                         # any field of bs_core::params::Params, by spec name
# tau_ping_ms = 100
```

Planned gate files (owned by the parent build): `baseline_100`, `flash_crowd`,
`churn_storm`, `lossy_links`, `poisoned_blocks`, `cold_start_n2`.
