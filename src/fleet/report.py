#!/usr/bin/env python3
"""Collect per-container result.json files from the fleet's results volume into
one verdict.json in the same schema the simulator writes, then print it.

Each container writes /results/<hostname>/result.json in the bs_node::RunReport
schema (role, output_hash, source_hash, steady_starved, steady_played,
first_chunk_s, first_segment, parent_lost, repairs, ...).

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


def build_verdict(raw: pathlib.Path, psr_max: float, sjl_max_ms: float, seed: int, profile: str, start_delay_s: float = 0.0) -> dict:
    """Fold bsnode result.json files (bs_node::RunReport) into the simulator's verdict schema."""
    results = []
    for p in sorted(raw.glob("*/result.json")):
        try:
            r = json.loads(p.read_text())
            r["container"] = p.parent.name
            results.append(r)
        except (OSError, json.JSONDecodeError):
            continue
    pubs = [r for r in results if r.get("role") == "publisher"]
    viewers = [r for r in results if r.get("role") == "viewer"]
    source_hash = pubs[0].get("source_hash") if pubs else None
    starved = sum(int(v.get("steady_starved", 0)) for v in viewers)
    played = sum(int(v.get("steady_played", 0)) for v in viewers)
    psr = (starved / played) if played else 1.0
    # Viewers that start before the publisher emits its first chunk wait out the
    # publisher's start delay; that wait is not join latency.
    sjl = [max(0.0, float(v["first_chunk_s"]) - start_delay_s) * 1000.0 for v in viewers if v.get("first_chunk_s") is not None]
    sjl_mean = sum(sjl) / len(sjl) if sjl else float("inf")
    full = [v for v in viewers if v.get("first_segment") == 1]
    mismatches = sum(1 for v in full if source_hash and v.get("output_hash") != source_hash)
    never = sum(1 for v in viewers if v.get("first_chunk_s") is None)
    kpis = {
        "psr": {"value": psr, "threshold": psr_max, "op": "<=", "pass": psr <= psr_max},
        "sjl_mean_s": {"value": sjl_mean / 1000.0, "threshold": sjl_max_ms / 1000.0, "op": "<=", "pass": sjl_mean <= sjl_max_ms},
        "hash_mismatches": {"value": float(mismatches), "threshold": 0.0, "op": "<=", "pass": mismatches == 0},
        "viewers_never_played": {"value": float(never), "threshold": 0.0, "op": "<=", "pass": never == 0},
    }
    return {
        "scenario": f"fleet-{profile}",
        "seed": seed,
        "passed": all(k["pass"] for k in kpis.values()),
        "kpis": kpis,
        "invariant_violations": [],
        "repro": f"fleet/up.sh --viewers {len(viewers)} --profile {profile} --seed {seed}",
        "facts": {
            "publishers": len(pubs),
            "viewers": len(viewers),
            "source_hash": source_hash,
            "viewers_from_start": len(full),
            "viewer_hashes": {v["container"]: v.get("output_hash") for v in viewers},
            "parent_lost": sum(int(v.get("parent_lost", 0)) for v in viewers),
            "repairs": sum(int(v.get("repairs", 0)) for v in viewers),
        },
    }

def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default="results/fleet")
    ap.add_argument("--psr-max", type=float, default=0.001)
    ap.add_argument("--start-delay-s", type=float, default=float(__import__("os").environ.get("BS_START_DELAY_S", "0")))
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
    verdict = build_verdict(raw, a.psr_max, a.sjl_max_ms, a.seed, a.profile, a.start_delay_s)
    out.mkdir(parents=True, exist_ok=True)
    (out / "verdict.json").write_text(json.dumps(verdict, indent=2))
    print(json.dumps({k: v for k, v in verdict.items() if k != "nodes"}, indent=2))
    return 0 if verdict["passed"] else 1


if __name__ == "__main__":
    sys.exit(main())
