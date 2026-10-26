#!/usr/bin/env bash
# Single test entrypoint. Writes results/check.json and exits non-zero on any failure.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
mkdir -p results
REPORT="results/check.json"
FAILED=0
ENTRIES=()

json_escape() { python3 -c 'import json,sys; print(json.dumps(sys.stdin.read()))'; }

# run_step NAME REPRO CMD...
run_step() {
  local name="$1" repro="$2"; shift 2
  echo "==> $name"
  local start end secs status tail log
  log="$(mktemp)"
  start=$(date +%s)
  if "$@" >"$log" 2>&1; then status="pass"; else status="fail"; FAILED=1; fi
  end=$(date +%s); secs=$((end - start))
  tail=""
  if [ "$status" = "fail" ]; then
    tail="$(tail -n 40 "$log" | json_escape)"
    tail -n 40 "$log"
  fi
  echo "    $status (${secs}s)"
  local tail_field=""
  if [ -n "$tail" ]; then tail_field=", \"tail\": $tail"; fi
  ENTRIES+=("{\"name\": $(printf '%s' "$name" | json_escape), \"status\": \"$status\", \"seconds\": $secs, \"repro\": $(printf '%s' "$repro" | json_escape)$tail_field}")
  rm -f "$log"
}

skip_step() {
  local name="$1" reason="$2"
  echo "==> $name: skipped ($reason)"
  ENTRIES+=("{\"name\": $(printf '%s' "$name" | json_escape), \"status\": \"skip\", \"seconds\": 0, \"repro\": \"\", \"reason\": $(printf '%s' "$reason" | json_escape)}")
}

run_step "cargo fmt" "cargo fmt --all -- --check" cargo fmt --all -- --check
run_step "cargo clippy" "cargo clippy --workspace --all-targets -- -D warnings" cargo clippy --workspace --all-targets -- -D warnings
run_step "cargo test" "cargo test --workspace" cargo test --workspace

if command -v uv >/dev/null 2>&1; then
  run_step "python cross-decoder" "cd py && uv run pytest -q" bash -c "cd py && uv run pytest -q"
else
  skip_step "python cross-decoder" "uv not installed"
fi

# Known test video for the file-source scenario (output is gitignored).
if [ -x scripts/make_testdata.sh ]; then scripts/make_testdata.sh >/dev/null 2>&1 || true; fi

# Simulator gates: only when bs-sim exposes a `run` subcommand.
sim_ready=0
if cargo metadata --format-version 1 --no-deps 2>/dev/null | python3 -c '
import json,sys
m=json.load(sys.stdin)
ok=any(p["name"]=="bs-sim" and any(t["kind"]==["bin"] or "bin" in t["kind"] for t in p["targets"]) for p in m["packages"])
sys.exit(0 if ok else 1)'; then
  if cargo run -q -p bs-sim --profile sim -- --help 2>/dev/null | grep -q -w run; then sim_ready=1; fi
fi

if [ "$sim_ready" = 1 ]; then
  shopt -s nullglob
  scen=(scenarios/*.toml)
  if [ ${#scen[@]} -eq 0 ]; then
    skip_step "sim gates" "no scenarios/*.toml"
  fi
  for f in "${scen[@]}"; do
    name="$(basename "$f" .toml)"
    cmd="cargo run -q -p bs-sim --profile sim -- run $f --out results/$name"
    run_step "sim gate: $name" "$cmd" bash -c "$cmd"
  done
else
  skip_step "sim gates" "bs-sim not ready"
fi

# Docker fleet gate (opt-in: BS_FLEET=1, needs docker and a sample file). The lossy
# profile with a churn of 30 % of the viewers at 30 s is Ch8 §8.3 scenario A on real nodes.
if [ "${BS_FLEET:-0}" = 1 ]; then
  sample="${BS_FLEET_FILE:-fleet/media/sample.mp4}"
  if ! command -v docker >/dev/null 2>&1 || ! docker info >/dev/null 2>&1; then
    skip_step "fleet gate" "docker not available"
  elif [ ! -f "$sample" ]; then
    skip_step "fleet gate" "no sample file ($sample); set BS_FLEET_FILE"
  else
    # 25 MB of the sample is about a minute of video: the stream ends before the viewers
    # do, so every surviving viewer holds the whole stream and the hash check is exact.
    # The publisher writes its report when stopped, so the fleet comes down before the
    # report is built (the results volume is kept).
    fleet_cmd="fleet/up.sh --viewers ${BS_FLEET_VIEWERS:-20} --profile lossy --file $sample --max-bytes 25000000 --duration 90 && fleet/churn.sh --fraction 0.3 --at 30 && sleep 80 && fleet/down.sh --keep-results && python3 fleet/report.py && fleet/down.sh"
    run_step "fleet gate: lossy churn" "$fleet_cmd" bash -c "$fleet_cmd"
  fi
else
  skip_step "fleet gate" "set BS_FLEET=1 to run the docker fleet"
fi

{
  echo "{"
  echo "  \"passed\": $([ $FAILED = 0 ] && echo true || echo false),"
  echo "  \"steps\": ["
  for i in "${!ENTRIES[@]}"; do
    sep=","; [ "$i" = $((${#ENTRIES[@]} - 1)) ] && sep=""
    echo "    ${ENTRIES[$i]}$sep"
  done
  echo "  ]"
  echo "}"
} > "$REPORT"

echo
echo "report: $REPORT"
if [ $FAILED = 0 ]; then echo "ALL GREEN"; else echo "FAILED"; fi
exit $FAILED
