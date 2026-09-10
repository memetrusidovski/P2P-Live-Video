#!/usr/bin/env python3
"""Collect per-container result.json files from the fleet's results volume into
one verdict.json in the same schema the simulator writes, then print it.

Each container writes /results/<hostname>/result.json:
  {"role": "publisher"|"viewer", "hash": "<hex>", "starved_intervals": n,
   "playout_intervals": n, "join_latency_ms": x, "bytes_control": n, "bytes_media": n}

Usage: fleet/report.py [--out results/fleet] [--psr-max 0.001] [--sjl-max-ms 1500]
"""
from __future__ import annotations

import argparse
import json
import pathlib
import subprocess
import sys
import tempfile


def copy_results(dest: pathlib.Path) -> None:
    """Copy the compose results volume out through a throwaway container."""
    dest.mkdir(parents=True, exist_ok=True)
    proj = pathlib.Path(__file__).resolve().parent.name
    vol = f"{proj}_results"
    subprocess.run(
        ["docker", "run", "--rm", "-v", f"{vol}:/results:ro", "-v", f"{dest}:/out",
         "debian:bookworm-slim", "sh", "-c", "cp -r /results/. /out/"],
        check=True,
    )


def build_verdict(raw: pathlib.Path, psr_max: float, sjl_max_ms: float, seed: int, profile: str) -> dict:
    results = []
    for p in sorted(raw.glob("*/result.json")):
        try:
            r = json.loads(p.read_text())
            r["container"] = p.parent.name
            results.append(r)
        except (OSError, json.JSONDecodeError):
            results.append({"container": p.parent.name, "role": "unknown", "error": "unreadable"})
    pubs = [r for r in results if r.get("role") == "publisher"]
    viewers = [r for r in results if r.get("role") == "viewer"]
    pub_hash = pubs[0].get("hash") if pubs else None
    matched = sum(1 for v in viewers if v.get("hash") and v.get("hash") == pub_hash)
    starved = sum(v.get("starved_intervals", 0) for v in viewers)
    intervals = sum(v.get("playout_intervals", 0) for v in viewers)
    psr = starved / intervals if intervals else 1.0
    sjl = max((v.get("join_latency_ms", 0.0) for v in viewers), default=0.0)
    kpis = {
        "psr": {"value": psr, "threshold": psr_max, "op": "<=", "pass": psr <= psr_max},
        "sjl_ms": {"value": sjl, "threshold": sjl_max_ms, "op": "<=", "pass": sjl <= sjl_max_ms},
        "hash_match_fraction": {
            "value": matched / len(viewers) if viewers else 0.0,
            "threshold": 1.0, "op": ">=", "pass": bool(viewers) and matched == len(viewers),
        },
    }
    return {
        "scenario": f"fleet_{profile}_{len(viewers)}v",
        "seed": seed,
        "passed": all(k["pass"] for k in kpis.values()) and bool(pubs),
        "kpis": kpis,
        "invariant_violations": [] if pubs else ["no publisher result found"],
        "repro": f"fleet/up.sh --viewers {len(viewers)} --profile {profile} --seed {seed}",
        "nodes": results,
    }


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default="results/fleet")
    ap.add_argument("--psr-max", type=float, default=0.001)
    ap.add_argument("--sjl-max-ms", type=float, default=1500.0)
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--profile", default="clean")
    ap.add_argument("--raw", help="already-copied results dir (skip docker copy)")
    a = ap.parse_args()
    out = pathlib.Path(a.out)
    if a.raw:
        raw = pathlib.Path(a.raw)
    else:
        raw = pathlib.Path(tempfile.mkdtemp(prefix="bs-fleet-"))
        copy_results(raw)
    verdict = build_verdict(raw, a.psr_max, a.sjl_max_ms, a.seed, a.profile)
    out.mkdir(parents=True, exist_ok=True)
    (out / "verdict.json").write_text(json.dumps(verdict, indent=2))
    print(json.dumps({k: v for k, v in verdict.items() if k != "nodes"}, indent=2))
    return 0 if verdict["passed"] else 1


if __name__ == "__main__":
    sys.exit(main())
