#!/usr/bin/env bash
# Container entrypoint: apply netem for the profile, then run bsnode in the
# configured role. Writes /results/<hostname>/result.json on exit.
set -euo pipefail

ROLE="${BS_ROLE:-viewer}"
PROFILE="${BS_NETEM_PROFILE:-clean}"
# Each run gets its own directory so reports never mix runs.
OUT="/results/${BS_RUN_ID:-run}/$(hostname)"
# Parity comes from the netem profile (clean 1, lossy 4, cellular 5) unless overridden.
if [ -z "${BS_PARITY:-}" ] && [ -f "/profiles/${PROFILE}.env" ]; then BS_PARITY="$(grep ^BS_PARITY= "/profiles/${PROFILE}.env" | cut -d= -f2)"; fi
export BS_PARITY="${BS_PARITY:-1}"
mkdir -p "$OUT"

if [ -x /usr/local/bin/netem.sh ]; then
  /usr/local/bin/netem.sh "$PROFILE" || echo "netem: could not apply profile $PROFILE (needs NET_ADMIN)" >&2
fi

case "$ROLE" in
  publisher)
    if [ "${BS_INGEST:-file}" = "ffmpeg" ]; then
      exec bsnode --parity "$BS_PARITY" publish \
        --ingest ffmpeg --input "${BS_FILE:-/media/sample.mp4}" \
        --renditions "${BS_RENDITIONS:-360p:800,720p:2500}" \
        --listen "0.0.0.0:${BS_PORT:-4000}" \
        --advertise-ip "$(hostname -i | awk '{print $1}')" \
        --start-delay-s "${BS_START_DELAY_S:-5}" \
        --upload-kbps "${BS_PUB_UPLOAD_KBPS:-100000}" \
        --seed "${BS_SEED:-1}" \
        --player 0.0.0.0:8080 \
        --result "$OUT/result.json"
    fi
    exec bsnode --parity "$BS_PARITY" publish \
      --file "${BS_FILE:-/media/sample.mp4}" \
      ${BS_MAX_BYTES:+--max-bytes "$BS_MAX_BYTES"} \
      --player 0.0.0.0:8080 \
      --listen "0.0.0.0:${BS_PORT:-4000}" \
      --advertise-ip "$(hostname -i | awk '{print $1}')" \
      --start-delay-s "${BS_START_DELAY_S:-5}" \
      --upload-kbps "${BS_PUB_UPLOAD_KBPS:-100000}" \
      --seed "${BS_SEED:-1}" \
      --result "$OUT/result.json"
    ;;
  viewer)
    # WHY: Docker's embedded DNS fails lookups while a hundred containers start at
    #      once, and bsnode resolves the bootstrap name exactly once. Compose passes
    #      an address; a name is resolved here with retries.
    host="${BS_BOOTSTRAP%%:*}"; port="${BS_BOOTSTRAP##*:}"; ip=""
    for attempt in $(seq 1 60); do
      ip="$(getent ahostsv4 "$host" 2>/dev/null | awk '{print $1; exit}' || true)"
      [ -n "$ip" ] && break
      echo "$(date +%T) resolve $host: attempt $attempt failed" >&2
      sleep 1
    done
    echo "$(date +%T) bootstrap $host -> ${ip:-unresolved}" >&2
    exec bsnode --parity "$BS_PARITY" watch \
      --bootstrap "${ip:-$host}:${port}" \
      --publisher-seed "${BS_SEED:-1}" \
      --publisher-port "${BS_PORT:-4000}" \
      ${BS_KEEP_OUTPUT:+--out "$OUT/output.bin"} \
      ${BS_EXPECT_HASH:+--expect-hash "$BS_EXPECT_HASH"} \
      --class "${BS_CLASS:-relay}" \
      --upload-kbps "${BS_UPLOAD_KBPS:-20000}" \
      --seed "$(( ${BS_SEED:-1} + $(hostname | cksum | cut -d' ' -f1) % 100000 ))" \
      --duration-s "${BS_DURATION_S:-60}" \
      --player 0.0.0.0:8080 \
      --result "$OUT/result.json"
    ;;
  *)
    echo "unknown BS_ROLE=$ROLE" >&2; exit 2 ;;
esac
