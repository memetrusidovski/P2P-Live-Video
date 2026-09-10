#!/usr/bin/env bash
# Apply a tc netem profile to eth0 inside a container. Profiles live in
# /profiles/<name>.env (mounted) or ./profiles/<name>.env (host).
set -euo pipefail
PROFILE="${1:-clean}"
DIR="${PROFILE_DIR:-/profiles}"
[ -d "$DIR" ] || DIR="$(dirname "${BASH_SOURCE[0]}")/profiles"
FILE="$DIR/$PROFILE.env"
[ -f "$FILE" ] || { echo "no such profile: $FILE" >&2; exit 2; }
# shellcheck disable=SC1090
source "$FILE"
IFACE="${IFACE:-eth0}"
tc qdisc del dev "$IFACE" root 2>/dev/null || true
if [ "${NETEM_DELAY_MS:-0}" = 0 ] && [ "${NETEM_LOSS_PCT:-0}" = 0 ] && [ -z "${NETEM_RATE:-}" ]; then
  echo "netem: profile $PROFILE is clean, nothing applied"
  exit 0
fi
ARGS="delay ${NETEM_DELAY_MS:-0}ms ${NETEM_JITTER_MS:-0}ms loss ${NETEM_LOSS_PCT:-0}%"
[ -n "${NETEM_RATE:-}" ] && ARGS="$ARGS rate ${NETEM_RATE}"
tc qdisc add dev "$IFACE" root netem $ARGS
echo "netem: applied '$ARGS' on $IFACE"
