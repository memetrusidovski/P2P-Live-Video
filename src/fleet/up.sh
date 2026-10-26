#!/usr/bin/env bash
# Bring up one publisher and N viewers under a netem profile.
#   fleet/up.sh --viewers 20 --profile lossy [--file path/to/sample.mp4] [--duration 60] [--seed 1]
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"
VIEWERS=5; PROFILE=clean; FILE=""; DURATION=60; SEED=1; MAX_BYTES=""; INGEST=file; RENDITIONS="360p:800,720p:2500"
while [ $# -gt 0 ]; do
  case "$1" in
    --viewers) VIEWERS="$2"; shift 2 ;;
    --profile) PROFILE="$2"; shift 2 ;;
    --file) FILE="$2"; shift 2 ;;
    --duration) DURATION="$2"; shift 2 ;;
    --seed) SEED="$2"; shift 2 ;;
    --max-bytes) MAX_BYTES="$2"; shift 2 ;;
    --ingest) INGEST="$2"; shift 2 ;;
    --renditions) RENDITIONS="$2"; shift 2 ;;
    *) echo "unknown arg $1" >&2; exit 2 ;;
  esac
done
[ -f "profiles/$PROFILE.env" ] || { echo "unknown profile $PROFILE" >&2; exit 2; }
mkdir -p media
if [ -n "$FILE" ]; then cp "$FILE" media/sample.mp4; fi
[ -f media/sample.mp4 ] || { echo "no media/sample.mp4; pass --file" >&2; exit 2; }
# Viewers are checked against the publisher's reported source hash by report.py.
EXPECT_HASH=""
RUN_ID="$(date +%Y%m%d-%H%M%S)"
echo "$RUN_ID" > .last_run_id
# report.py reads these as defaults (profile-specific PSR bound, publisher start delay).
rm -f .churned
START_DELAY="${BS_START_DELAY_S:-8}"
printf 'BS_RUN_ID=%s\nBS_NETEM_PROFILE=%s\nBS_START_DELAY_S=%s\nBS_SEED=%s\n' "$RUN_ID" "$PROFILE" "$START_DELAY" "$SEED" > .last_run.env
# Start from a clean slate: previous containers and their results volume.
docker compose down -v --remove-orphans >/dev/null 2>&1 || true
export BS_RUN_ID="$RUN_ID" BS_SEED="$SEED" BS_NETEM_PROFILE="$PROFILE" BS_DURATION_S="$DURATION" BS_EXPECT_HASH="$EXPECT_HASH" BS_MAX_BYTES="$MAX_BYTES" BS_INGEST="$INGEST" BS_RENDITIONS="$RENDITIONS"
docker compose build
docker compose up -d --scale viewer="$VIEWERS"
echo "fleet up: 1 publisher + $VIEWERS viewers, profile=$PROFILE, seed=$SEED"
echo "publisher status: http://localhost:8079/   viewers: http://localhost:8080.. (one port per replica)"
echo "live dashboard:   python3 fleet/dashboard.py   (then open http://localhost:9000/)"
echo "collect with:     python3 fleet/report.py"
