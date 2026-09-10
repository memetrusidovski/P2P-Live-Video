#!/usr/bin/env bash
# Generates a small known video file for the file-source scenarios and the
# M2 fleet tests. Output is gitignored (testdata/generated/).
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p testdata/generated
out=testdata/generated/sample.mp4
if [ -f "$out" ]; then
  echo "testdata: $out exists"
  exit 0
fi
if ! command -v ffmpeg >/dev/null 2>&1; then
  echo "testdata: ffmpeg not found; writing a pseudo-random 3 MB file instead" >&2
  head -c 3000000 /dev/urandom > "$out"
  exit 0
fi
# 20 s of test pattern + tone, 1 s closed GOPs, ~1.5 Mbps.
ffmpeg -hide_banner -loglevel error -y \
  -f lavfi -i "testsrc2=size=640x360:rate=30" -f lavfi -i "sine=frequency=440:sample_rate=48000" \
  -t 20 -c:v libx264 -preset veryfast -b:v 1500k -g 30 -keyint_min 30 -sc_threshold 0 \
  -c:a aac -b:a 96k -movflags +faststart "$out"
echo "testdata: wrote $out ($(wc -c < "$out") bytes, blake3 via b3sum if installed)"
