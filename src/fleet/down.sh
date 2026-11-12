#!/usr/bin/env bash
# Tear down the fleet. Pass --keep-results to leave the results volume in place.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"
# WHY: `docker compose down` removes the containers and their logs. A publisher that
#      hung or crashed leaves no result file, so its log is the only evidence.
RUN_ID="$(cat .last_run_id 2>/dev/null || echo unknown)"
LOG_DIR="../results/fleet/logs"
if [ -n "$(docker compose ps -aq 2>/dev/null)" ]; then
  mkdir -p "$LOG_DIR"
  docker compose logs --no-color --timestamps > "$LOG_DIR/$RUN_ID.log" 2>&1 || true
  docker compose ps -a --format '{{.Name}} {{.Status}}' > "$LOG_DIR/$RUN_ID.status" 2>&1 || true
fi
if [ "${1:-}" = "--keep-results" ]; then
  docker compose down
else
  docker compose down -v
fi
