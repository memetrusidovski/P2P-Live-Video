#!/usr/bin/env python3
"""Live fleet dashboard.

Polls every node's /api/status and checks, chunk by chunk, that each viewer's
per-layer Blake3 checkpoints equal the publisher's. The default targets are the
docker fleet: the publisher on host port 8079 and viewers on the 8080-8199 range.
Any list of host:lo-hi ranges reaches a swarm on other machines. Serves a page at
http://localhost:9000/ that refreshes every two seconds. Each row links to the
viewer's own player page, where video plays in ffmpeg mode.

Usage:
  fleet/dashboard.py [--port 9000] [--publisher localhost:8079] [--viewers localhost:8080-8199]
  fleet/dashboard.py --publisher 192.168.1.10:8079 --viewers 192.168.1.10:8080-8089,192.168.1.11:8080-8099
"""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
import threading
import time
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

STATE = {"publisher": None, "publisher_url": "", "viewers": [], "updated": 0.0, "error": None}


def fetch(url: str, timeout: float = 1.5):
    try:
        with urllib.request.urlopen(url, timeout=timeout) as r:
            return json.loads(r.read().decode())
    except Exception:  # noqa: BLE001 - a down container is a normal state here
        return None


def viewer_targets(spec: str) -> list[tuple[str, int]]:
    """The (host, port) pairs to poll.

    spec is a comma-separated list of host:port or host:lo-hi. For the compose default
    (localhost) docker's view of the published ports is preferred over probing the range.
    """
    targets: list[tuple[str, int]] = []
    for item in spec.split(","):
        item = item.strip()
        if not item:
            continue
        host, rng = item.rsplit(":", 1)
        if host == "localhost":
            ports = docker_ports()
            if ports:
                targets.extend((host, p) for p in ports)
                continue
        lo, _, hi = rng.partition("-")
        for p in range(int(lo), int(hi or lo) + 1):
            targets.append((host, p))
    return sorted(set(targets))


def docker_ports() -> list[int]:
    """Host ports docker compose published for the viewer service, or [] without a fleet."""
    ports: list[int] = []
    try:
        out = subprocess.run(
            ["docker", "compose", "ps", "--format", "json"],
            capture_output=True, text=True, check=True, cwd=__import__("pathlib").Path(__file__).resolve().parent,
        ).stdout
        for line in out.splitlines():
            try:
                c = json.loads(line)
            except json.JSONDecodeError:
                continue
            if "viewer" not in c.get("Service", ""):
                continue
            for p in str(c.get("Publishers") or c.get("Ports") or "").split(","):
                p = p.strip()
                if "->8080" in p:
                    host = p.split("->")[0].rsplit(":", 1)[-1]
                    if host.isdigit():
                        ports.append(int(host))
    except Exception:  # noqa: BLE001
        pass
    return sorted(set(ports))


def compare(pub: dict | None, viewer: dict) -> dict:
    """Per-chunk hash comparison against the publisher's checkpoints."""
    if not pub:
        return {"checked": 0, "match": 0, "mismatch": 0, "verdict": "no publisher"}
    pub_cp = {(c["segment"], c["chunk"]): c["layer_hashes"] for c in pub.get("checkpoints", [])}
    match = mismatch = 0
    for c in viewer.get("checkpoints", []):
        ref = pub_cp.get((c["segment"], c["chunk"]))
        if ref is None:
            continue
        for l, h in enumerate(c["layer_hashes"]):
            if l < len(ref):
                if ref[l] == h:
                    match += 1
                else:
                    mismatch += 1
    verdict = "OK" if mismatch == 0 and match > 0 else ("MISMATCH" if mismatch else "no overlap yet")
    return {"checked": match + mismatch, "match": match, "mismatch": mismatch, "verdict": verdict}


def poll(pub_addr: str, viewers_spec: str, interval: float) -> None:
    while True:
        pub = fetch(f"http://{pub_addr}/api/status")
        rows = []
        for host, port in viewer_targets(viewers_spec):
            st = fetch(f"http://{host}:{port}/api/status", timeout=0.8)
            if st is None:
                continue
            row = {"port": port, "host": host, "url": f"http://{host}:{port}/", **{k: st.get(k) for k in (
                "node_id", "state", "depth", "played", "starved", "chunks_full", "chunks_ok", "blocks_rejected",
                "parent_lost", "bytes", "hash_so_far", "first_chunk_s", "layers", "layers_complete_last", "descriptor_version")}}
            row["integrity"] = compare(pub, st)
            rows.append(row)
        STATE.update(publisher=pub, publisher_url=f"http://{pub_addr}/", viewers=rows, updated=time.time())
        time.sleep(interval)


PAGE = """<!doctype html><meta charset=utf-8><title>BitStream fleet</title>
<style>body{font:14px system-ui;margin:20px;background:#111;color:#ddd}table{border-collapse:collapse}td,th{padding:4px 10px;border-bottom:1px solid #333;text-align:left;font-variant-numeric:tabular-nums}
.ok{color:#5c5}.bad{color:#f55}.warn{color:#fc5}a{color:#8cf}code{color:#aaa}</style>
<h2>BitStream fleet <small id=upd></small></h2>
<div id=pub></div>
<table id=t><thead><tr><th>viewer</th><th>state</th><th>depth</th><th>layers</th><th>played</th><th>starved</th><th>rejected</th><th>parent lost</th><th>bytes</th><th>hash so far</th><th>integrity vs publisher</th></tr></thead><tbody></tbody></table>
<p>Integrity compares each viewer's per-chunk, per-layer Blake3 checkpoints with the publisher's for the same segment and chunk. Every block was already Merkle-verified against the publisher's signed manifest on arrival; this is the independent end-to-end witness.</p>
<script>
async function tick(){const r=await fetch('/api/fleet');const d=await r.json();
document.getElementById('upd').textContent=new Date(d.updated*1000).toLocaleTimeString();
const p=d.publisher;document.getElementById('pub').innerHTML=p?`<b>publisher</b> ${p.state} · published bytes ${p.bytes} · hash so far <code>${(p.hash_so_far||'').slice(0,16)}</code>${p.source_hash?` · source file <code>${p.source_hash.slice(0,16)}</code>`:''} · <a href="${d.publisher_url}">status</a>`:'<b>publisher</b> unreachable';
const tb=document.querySelector('#t tbody');tb.innerHTML='';
for(const v of d.viewers){const i=v.integrity;const cls=i.verdict==='OK'?'ok':(i.verdict==='MISMATCH'?'bad':'warn');
tb.insertAdjacentHTML('beforeend',`<tr><td><a href="${v.url}" target=_blank>${v.host==='localhost'?'':v.host}:${v.port}</a> <code>${(v.node_id||'').slice(0,8)}</code></td><td>${v.state}</td><td>${(v.depth||[]).map(x=>x===255?'-':x).join(' ')}</td><td>${v.layers_complete_last}/${v.layers}</td><td>${v.played}</td><td class="${v.starved?'bad':''}">${v.starved}</td><td class="${v.blocks_rejected?'bad':''}">${v.blocks_rejected}</td><td>${v.parent_lost}</td><td>${v.bytes}</td><td><code>${(v.hash_so_far||'').slice(0,16)}</code></td><td class="${cls}">${i.verdict} (${i.match} layer-chunks)</td></tr>`)}}
tick();setInterval(tick,2000);
</script>"""


class H(BaseHTTPRequestHandler):
    def log_message(self, *a):  # quiet
        pass

    def do_GET(self):
        if self.path.startswith("/api/fleet"):
            body = json.dumps(STATE).encode()
            ctype = "application/json"
        else:
            body = PAGE.encode()
            ctype = "text/html; charset=utf-8"
        self.send_response(200)
        self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", type=int, default=9000)
    ap.add_argument("--publisher", default="localhost:8079", help="host:port of the publisher's status API")
    ap.add_argument("--viewers", default="localhost:8080-8199",
                    help="comma-separated host:lo-hi ranges of viewer status APIs, one per machine")
    ap.add_argument("--interval", type=float, default=2.0)
    a = ap.parse_args()
    threading.Thread(target=poll, args=(a.publisher, a.viewers, a.interval), daemon=True).start()
    print(f"dashboard: http://localhost:{a.port}/  (publisher {a.publisher}, viewers {a.viewers})")
    ThreadingHTTPServer(("0.0.0.0", a.port), H).serve_forever()
    return 0


if __name__ == "__main__":
    sys.exit(main())
