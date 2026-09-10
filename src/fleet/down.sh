#!/usr/bin/env bash
# Tear down the fleet. Pass --keep-results to leave the results volume in place.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"
if [ "${1:-}" = "--keep-results" ]; then
  docker compose down
else
  docker compose down -v
fi
