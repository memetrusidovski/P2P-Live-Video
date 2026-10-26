#!/usr/bin/env bash
# Kill a fraction of the running viewers at once (Ch8 §8.3 scenario A on real nodes).
#   fleet/churn.sh --fraction 0.3 [--at 30]
# The killed containers are listed in .churned so report.py scores the survivors only.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"
FRACTION=0.3; AT=0
while [ $# -gt 0 ]; do
  case "$1" in
    --fraction) FRACTION="$2"; shift 2 ;;
    --at) AT="$2"; shift 2 ;;
    *) echo "unknown arg $1" >&2; exit 2 ;;
  esac
done
if [ "$AT" != "0" ]; then sleep "$AT"; fi
# A plain read loop: macOS ships bash 3, which has no mapfile.
viewers=()
while IFS= read -r name; do
  [ -n "$name" ] && viewers+=("$name")
done < <(docker compose ps --status running --format '{{.Name}}' viewer | sort)
total=${#viewers[@]}
count=$(python3 -c "import math; print(max(1, math.floor($total * $FRACTION)))")
# A stable pick: every other container from the end, so the set is reproducible.
victims=()
for ((i = total - 1; i >= 0 && ${#victims[@]} < count; i -= 2)); do
  victims+=("${viewers[$i]}")
done
: > .churned
for v in "${victims[@]}"; do
  # A viewer that exited on its own between the listing and the kill is churn too.
  docker kill --signal SIGKILL "$v" >/dev/null 2>&1 || true
  echo "$v" >> .churned
done
echo "churn: killed ${#victims[@]} of $total viewers at +${AT}s: ${victims[*]}"
