#!/usr/bin/env bash
# Bring up one publisher and N viewers under a netem profile.
#   fleet/up.sh --viewers 20 --profile lossy [--file path/to/sample.mp4] [--duration 60] [--seed 1]
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"
VIEWERS=5; PROFILE=clean; FILE=""; DURATION=60; SEED=1
while [ $# -gt 0 ]; do
  case "$1" in
    --viewers) VIEWERS="$2"; shift 2 ;;
    --profile) PROFILE="$2"; shift 2 ;;
    --file) FILE="$2"; shift 2 ;;
    --duration) DURATION="$2"; shift 2 ;;
    --seed) SEED="$2"; shift 2 ;;
    *) echo "unknown arg $1" >&2; exit 2 ;;
  esac
done
[ -f "profiles/$PROFILE.env" ] || { echo "unknown profile $PROFILE" >&2; exit 2; }
mkdir -p media
if [ -n "$FILE" ]; then cp "$FILE" media/sample.mp4; fi
[ -f media/sample.mp4 ] || { echo "no media/sample.mp4; pass --file" >&2; exit 2; }
EXPECT_HASH="$(python3 - <<'PY'
import hashlib, sys
try:
    import blake3  # type: ignore
    h = blake3.blake3(open("media/sample.mp4", "rb").read()).hexdigest()
except ImportError:
    h = ""  # bsnode computes and reports the hash itself; the report compares publisher vs viewers
print(h)
PY
)"
export BS_SEED="$SEED" BS_NETEM_PROFILE="$PROFILE" BS_DURATION_S="$DURATION" BS_EXPECT_HASH="$EXPECT_HASH"
docker compose build
docker compose up -d --scale viewer="$VIEWERS"
echo "fleet up: 1 publisher + $VIEWERS viewers, profile=$PROFILE, seed=$SEED"
echo "collect with: fleet/report.py"
