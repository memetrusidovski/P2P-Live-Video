#!/usr/bin/env bash
# Run one simulator scenario over several seeds and print one KPI line per seed.
#
#   scripts/sim_seeds.sh baseline_100 1 2 3 4 5 6
#
# WHY: Repair KPIs rest on a handful of repairs per run and move with the seed.
#      Judge a timer or parity change on several seeds, never on one gate run.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
if [ $# -lt 2 ]; then
  echo "usage: $0 SCENARIO SEED..." >&2
  exit 2
fi
scen="$1"; shift
out="results/seeds_$scen"
for seed in "$@"; do
  o="$(cargo run -q -p bs-sim --profile sim -- run "scenarios/$scen.toml" --seed "$seed" --out "$out" 2>&1 || true)"
  kpi() { echo "$o" | awk -v k="$1" '$1 == k { print $2 }'; }
  verdict="$(echo "$o" | head -1 | grep -o 'PASS\|FAIL' || echo ERROR)"
  repairs="$(echo "$o" | awk '$1 == "repairs:" { print $2 }')"
  echo "$scen seed $seed: $verdict psr $(kpi psr) repair_median_s $(kpi repair_median_s) repair_p95_s $(kpi repair_p95_s) sjl_mean_s $(kpi sjl_mean_s) repairs $repairs"
done
rm -rf "$out"
