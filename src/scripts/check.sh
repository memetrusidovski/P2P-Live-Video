#!/usr/bin/env bash
# Single test entrypoint. Writes results/check.json and exits non-zero on any failure.
#
#   scripts/check.sh          quick: lints, tests, every simulator gate on its own
#                             seed, and the end-to-end smoke test
#   scripts/check.sh full     quick, then every simulator gate over 5 seeds against
#                             the baseline (scripts/gates.py)
#   scripts/check.sh fleet    the Docker fleet matrix only (fleet/matrix.py)
#   scripts/check.sh all      full, then the fleet matrix
set -euo pipefail
TIER="${1:-quick}"
case "$TIER" in quick|full|fleet|all) ;; *) echo "usage: $0 [quick|full|fleet|all]" >&2; exit 2 ;; esac

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

if [ "$TIER" != fleet ]; then
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

  # A scenario that misses on its own seed is run over 5 seeds by gates.py; the
  # miss fails the step only if the 5 seeds confirm it.
  run_step "sim gates: own seed" "scripts/gates.py run" scripts/gates.py run
  run_step "smoke test" "scripts/smoke.py" scripts/smoke.py
fi

if [ "$TIER" = full ] || [ "$TIER" = all ]; then
  run_step "sim gates: 5 seeds" "scripts/gates.py run --seeds 5" scripts/gates.py run --seeds 5
else
  skip_step "sim gates: 5 seeds" "run scripts/check.sh full"
fi

# COMPATIBILITY: BS_FLEET=1 added the fleet to a run before the tiers existed.
if [ "$TIER" = fleet ] || [ "$TIER" = all ] || [ "${BS_FLEET:-0}" = 1 ]; then
  run_step "fleet matrix" "fleet/matrix.py run" fleet/matrix.py run
else
  skip_step "fleet matrix" "run scripts/check.sh fleet"
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
