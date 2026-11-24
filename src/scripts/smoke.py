#!/usr/bin/env python3
"""End-to-end smoke test: real nodes on loopback, encoded video, the player API.

    scripts/smoke.py [--viewers 3] [--keep]

Starts one publisher with ffmpeg ingest and several viewers. Then checks what a
person at the player sees: the page loads and the status reports `playing`.
The chunk counter advances and the WebSocket delivers media. A request from a
foreign origin is refused. Exits 0 when every check passes.

Needs ffmpeg and `scripts/make_testdata.sh` (run by this script).
"""
from __future__ import annotations

import argparse
import base64
import json
import os
import pathlib
import shutil
import signal
import socket
import struct
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request

SRC = pathlib.Path(__file__).resolve().parent.parent
BSNODE = SRC / "target" / "release" / "bsnode"
SAMPLE = SRC / "testdata" / "generated" / "sample.mp4"
PUBLISHER_SEED = "7"
# Starved share of played chunks a viewer can show on loopback.
STARVED_MAX = 0.05
# Seconds a node has to reach the health the test waits for.
READY_S = 40


def free_port(kind: int) -> int:
    """Return a loopback port that is free now, for `socket.SOCK_STREAM` or `SOCK_DGRAM`."""
    with socket.socket(socket.AF_INET, kind) as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def get(port: int, path: str, origin: str | None = None) -> tuple[int, bytes]:
    """Fetch a path from a player API on loopback.

    Returns:
        The HTTP status and the body. Status 0 means the request did not connect.
    """
    req = urllib.request.Request(f"http://127.0.0.1:{port}{path}")
    if origin:
        req.add_header("Origin", origin)
    try:
        with urllib.request.urlopen(req, timeout=3) as r:
            return r.status, r.read()
    except urllib.error.HTTPError as e:
        return e.code, b""
    except (OSError, urllib.error.URLError):
        return 0, b""


def status(port: int) -> dict:
    """Return the status of a node, or an empty map if the API does not answer."""
    code, body = get(port, "/api/status")
    return json.loads(body) if code == 200 else {}


def wait_health(port: int, want: str, seconds: float) -> dict:
    """Poll the status until `health` equals `want` or the time runs out. Return the last status."""
    deadline = time.time() + seconds
    st: dict = {}
    while time.time() < deadline:
        st = status(port)
        if st.get("health") == want:
            return st
        time.sleep(0.5)
    return st


def ws_frames(port: int, seconds: float) -> tuple[int, int]:
    """Open the player WebSocket and count the frames the server sends.

    Args:
        port: Port of the player API.
        seconds: Time to listen. The function returns early after the first binary frame.

    Returns:
        The count of text frames and the count of binary frames.
    """
    text = binary = 0
    key = base64.b64encode(os.urandom(16)).decode()
    with socket.create_connection(("127.0.0.1", port), timeout=3) as s:
        s.sendall(
            f"GET /ws HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n"
            f"Sec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n\r\n".encode()
        )
        buf = b""
        while b"\r\n\r\n" not in buf:
            part = s.recv(4096)
            if not part:
                return 0, 0
            buf += part
        head, buf = buf.split(b"\r\n\r\n", 1)
        if b" 101 " not in head.split(b"\r\n", 1)[0]:
            return 0, 0
        deadline = time.time() + seconds
        s.settimeout(0.5)

        def take(n: int) -> bytes | None:
            nonlocal buf
            while len(buf) < n:
                if time.time() > deadline:
                    return None
                try:
                    part = s.recv(65536)
                except socket.timeout:
                    continue
                if not part:
                    return None
                buf += part
            out, buf = buf[:n], buf[n:]
            return out

        while binary == 0:
            h = take(2)
            if h is None:
                break
            opcode, length = h[0] & 0x0F, h[1] & 0x7F
            if length == 126:
                ext = take(2)
                if ext is None:
                    break
                length = struct.unpack(">H", ext)[0]
            elif length == 127:
                ext = take(8)
                if ext is None:
                    break
                length = struct.unpack(">Q", ext)[0]
            if take(length) is None:
                break
            if opcode == 1:
                text += 1
            elif opcode == 2:
                binary += 1
    return text, binary


def main() -> int:
    """Run the smoke test and print one line per check."""
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--viewers", type=int, default=3)
    ap.add_argument("--keep", action="store_true", help="keep the work directory with logs and results")
    args = ap.parse_args()

    if not shutil.which("ffmpeg"):
        print("SKIP smoke: ffmpeg is not installed")
        return 0
    subprocess.run(["scripts/make_testdata.sh"], cwd=SRC, capture_output=True)
    if not SAMPLE.is_file():
        print(f"no test video at {SAMPLE}", file=sys.stderr)
        return 2
    build = subprocess.run(["cargo", "build", "-q", "--release", "-p", "bs-node"], cwd=SRC)
    if build.returncode != 0:
        return build.returncode

    work = pathlib.Path(tempfile.mkdtemp(prefix="bs-smoke-"))
    udp = free_port(socket.SOCK_DGRAM)
    pub_api = free_port(socket.SOCK_STREAM)
    procs: list[subprocess.Popen] = []
    failures: list[str] = []

    def check(name: str, ok: bool, detail: str = "") -> None:
        print(f"{'ok  ' if ok else 'FAIL'} {name}{'  ' + detail if detail else ''}", flush=True)
        if not ok:
            failures.append(name)

    def start(label: str, argv: list[str]) -> None:
        log = (work / f"{label}.log").open("w")
        procs.append(subprocess.Popen([str(BSNODE), "--sim-pow", *argv], cwd=SRC, stdout=log, stderr=subprocess.STDOUT))

    try:
        start("publisher", [
            "publish", "--listen", f"127.0.0.1:{udp}", "--seed", PUBLISHER_SEED,
            "--ingest", "ffmpeg", "--input", str(SAMPLE), "--input-args=-stream_loop -1",
            "--renditions", "240p:400,360p:800", "--player", f"127.0.0.1:{pub_api}",
            "--result", str(work / "publisher.json"),
        ])
        st = wait_health(pub_api, "publishing", READY_S)
        check("publisher reports publishing", st.get("health") == "publishing", str(st.get("health_reason") or st.get("health") or "no answer"))

        apis = []
        for i in range(args.viewers):
            api = free_port(socket.SOCK_STREAM)
            apis.append(api)
            start(f"viewer-{i}", [
                "watch", "--listen", "127.0.0.1:0", "--bootstrap", f"127.0.0.1:{udp}",
                "--publisher-seed", PUBLISHER_SEED, "--publisher-port", str(udp),
                "--top-layer", "1", "--player", f"127.0.0.1:{api}",
                "--out", str(work / f"viewer-{i}.bin"), "--result", str(work / f"viewer-{i}.json"),
            ])

        for i, api in enumerate(apis):
            st = wait_health(api, "playing", READY_S)
            check(f"viewer {i} reports playing", st.get("health") == "playing", str(st.get("health_reason") or st.get("health") or "no answer"))

        code, body = get(apis[0], "/")
        check("player page loads", code == 200 and b"<video" in body, f"status {code}")
        code, _ = get(apis[0], "/api/descriptor")
        check("descriptor is served", code == 200, f"status {code}")
        code, _ = get(apis[0], "/api/status", origin="http://attacker.example")
        check("foreign origin is refused", code in (401, 403), f"status {code}")
        text, binary = ws_frames(apis[0], 10)
        check("WebSocket delivers media", text >= 1 and binary >= 1, f"{text} text frames, {binary} binary frames")

        before = [status(a).get("played", 0) for a in apis]
        time.sleep(6)
        after = [status(a) for a in apis]
        for i, st in enumerate(after):
            played, starved = st.get("played", 0), st.get("starved", 0)
            check(f"viewer {i} keeps playing", played - before[i] >= 12, f"{played - before[i]} chunks in 6 s")
            share = starved / played if played else 1.0
            check(f"viewer {i} starved share", share <= STARVED_MAX, f"{starved} of {played} chunks")
            check(f"viewer {i} has every layer", st.get("layers_complete_last", 0) >= 2, f"{st.get('layers_complete_last')} of {st.get('layers')} layers")
    finally:
        for p in procs:
            if p.poll() is None:
                p.send_signal(signal.SIGINT)
        for p in procs:
            try:
                p.wait(timeout=10)
            except subprocess.TimeoutExpired:
                p.kill()
                failures.append("a node did not stop on SIGINT")
                print("FAIL a node did not stop on SIGINT")

    for i in range(args.viewers):
        result = work / f"viewer-{i}.json"
        report = json.loads(result.read_text()) if result.exists() else None
        check(f"viewer {i} wrote its report", report is not None)
        if report is not None:
            check(f"viewer {i} ended without an error", not report.get("last_error"), str(report.get("last_error") or ""))

    if failures or args.keep:
        print(f"logs and results: {work}")
    else:
        shutil.rmtree(work, ignore_errors=True)
    print("SMOKE " + ("FAILED" if failures else "PASSED"))
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
