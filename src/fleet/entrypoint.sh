#!/usr/bin/env bash
# Container entrypoint: apply netem for the profile, then run bsnode in the
# configured role. Writes /results/<hostname>/result.json on exit.
set -euo pipefail

ROLE="${BS_ROLE:-viewer}"
PROFILE="${BS_NETEM_PROFILE:-clean}"
OUT="/results/$(hostname)"
mkdir -p "$OUT"

if [ -x /usr/local/bin/netem.sh ]; then
  /usr/local/bin/netem.sh "$PROFILE" || echo "netem: could not apply profile $PROFILE (needs NET_ADMIN)" >&2
fi

case "$ROLE" in
  publisher)
    exec bsnode publish \
      --file "${BS_FILE:-/media/sample.mp4}" \
      --listen "0.0.0.0:${BS_PORT:-4000}" \
      --seed "${BS_SEED:-1}" \
      --result "$OUT/result.json"
    ;;
  viewer)
    exec bsnode watch \
      --bootstrap "${BS_BOOTSTRAP:-publisher:4000}" \
      --out "$OUT/output.bin" \
      --expect-hash "${BS_EXPECT_HASH:-}" \
      --seed "${BS_SEED:-1}" \
      --duration-s "${BS_DURATION_S:-60}" \
      --result "$OUT/result.json"
    ;;
  *)
    echo "unknown BS_ROLE=$ROLE" >&2; exit 2 ;;
esac
