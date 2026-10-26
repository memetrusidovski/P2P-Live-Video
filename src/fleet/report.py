#!/usr/bin/env python3
"""Collect per-container result.json files from the fleet's results volume into
one verdict.json in the same schema the simulator writes, then print it.

Each container writes /results/<hostname>/result.json in the bs_node::RunReport
schema (role, output_hash, source_hash, steady_starved, steady_played,
first_chunk_s, first_segment, parent_lost, repairs, ...).

Usage: fleet/report.py [--out results/fleet] [--psr-max 0.001] [--sjl-max-ms 1500]
"""
from __future__ import annotations

RUN_ID_FILTER: str | None = None

import argparse
import json
import pathlib
import subprocess
import sys


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


def churned_hosts() -> set:
    """Containers churn.sh killed in the last run; they wrote no result and are not scored."""
    p = pathlib.Path(__file__).resolve().parent / ".churned"
    if not p.exists():
        return set()
    return {line.strip() for line in p.read_text().splitlines() if line.strip()}


def build_verdict(raw: pathlib.Path, psr_max: float, sjl_max_ms: float, seed: int, profile: str, start_delay_s: float = 0.0, churned: set | None = None) -> dict:
    """Fold bsnode result.json files (bs_node::RunReport) into the simulator's verdict schema.

    Args:
        raw: Directory the results volume was copied to.
        psr_max: Steady-state starvation bound.
        sjl_max_ms: Mean join latency bound in milliseconds.
        seed: Publisher seed of the run.
        profile: Netem profile name.
        start_delay_s: Publisher start delay subtracted from join latency.
        churned: Container names killed mid-run; absent results for them are expected.
    """
    churned = churned or set()
    results = []
    # Results are laid out /results/<run-id>/<hostname>/result.json; report the newest
    # run only (or the one named by --run-id).
    runs = sorted(d for d in raw.iterdir() if d.is_dir() and any(d.glob("*/result.json")))
    chosen = RUN_ID_FILTER or (runs[-1].name if runs else None)
    for p in sorted(raw.glob(f"{chosen}/*/result.json")) if chosen else []:
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
    # A starved chunk is a hole in the output, so only a viewer that played every chunk
    # from segment 1 must hash equal to the source. A mismatch there is corruption; a
    # mismatch with starvation is the starvation the PSR gate already scores.
    # A shed layer leaves a chunk playable but shorter, so only chunks played with
    # every layer (`chunks_full`) can match the source byte for byte.
    exact = [
        v
        for v in full
        if int(v.get("steady_starved", 0)) == 0
        and v.get("chunks_full") == v.get("chunks_played")
    ]
    mismatches = sum(1 for v in exact if source_hash and v.get("output_hash") != source_hash)
    never = sum(1 for v in viewers if v.get("first_chunk_s") is None)
    kpis = {
        "psr": {"value": psr, "threshold": psr_max, "op": "<=", "pass": psr <= psr_max},
        "sjl_mean_s": {"value": sjl_mean / 1000.0, "threshold": sjl_max_ms / 1000.0, "op": "<=", "pass": sjl_mean <= sjl_max_ms},
        "hash_mismatches": {"value": float(mismatches), "threshold": 0.0, "op": "<=", "pass": mismatches == 0},
        "viewers_never_played": {"value": float(never), "threshold": 0.0, "op": "<=", "pass": never == 0},
    }
    if churned:
        # The survivors must keep playing: a survivor that loses a tenth of its chunks
        # after the churn has stalled.
        stalled = sum(1 for v in viewers if int(v.get("steady_starved", 0)) * 10 > int(v.get("steady_played", 0)))
        kpis["survivors_stalled"] = {"value": float(stalled), "threshold": 0.0, "op": "<=", "pass": stalled == 0}
    scenario = f"fleet-{profile}" + ("-churn" if churned else "")
    repro = f"fleet/up.sh --viewers {len(viewers) + len(churned)} --profile {profile} --seed {seed}"
    if churned:
        repro += f" && fleet/churn.sh --fraction {len(churned) / max(1, len(viewers) + len(churned)):.2f} --at 30"
    return {
        "scenario": scenario,
        "seed": seed,
        "passed": all(k["pass"] for k in kpis.values()),
        "kpis": kpis,
        "invariant_violations": [],
        "repro": repro,
        "facts": {
            "publishers": len(pubs),
            "viewers": len(viewers),
            "source_hash": source_hash,
            "viewers_from_start": len(full),
            "viewers_exact": len(exact),
            "per_viewer": {
                v["container"]: {
                    "first_segment": v.get("first_segment"),
                    "played": v.get("steady_played"),
                    "starved": v.get("steady_starved"),
                    "parent_lost": v.get("parent_lost"),
                    "repairs": v.get("repairs"),
                    "chunks_full": v.get("chunks_full"),
                    "bytes_out": v.get("bytes_out"),
                    "hash_ok": bool(source_hash) and v.get("output_hash") == source_hash,
                }
                for v in viewers
            },
            "viewer_hashes": {v["container"]: v.get("output_hash") for v in viewers},
            "parent_lost": sum(int(v.get("parent_lost", 0)) for v in viewers),
            "repairs": sum(int(v.get("repairs", 0)) for v in viewers),
            "churned": sorted(churned),
        },
    }

# Steady-state starvation bound per netem profile: the same numbers the simulator gates
# use for the matching scenarios (clean = baseline_100, lossy = lossy_links).
PROFILE_PSR_MAX = {"clean": 0.001, "lossy": 0.02, "cellular": 0.05}


def last_run_env() -> dict:
    """Defaults written by up.sh for the most recent run."""
    p = pathlib.Path(__file__).resolve().parent / ".last_run.env"
    env = {}
    if p.exists():
        for line in p.read_text().splitlines():
            if "=" in line:
                k, v = line.split("=", 1)
                env[k.strip()] = v.strip()
    return env


def main() -> int:
    last = last_run_env()
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default="results/fleet")
    ap.add_argument("--psr-max", type=float, default=None, help="default: per profile, see PROFILE_PSR_MAX")
    ap.add_argument(
        "--start-delay-s",
        type=float,
        default=float(__import__("os").environ.get("BS_START_DELAY_S", last.get("BS_START_DELAY_S", "0"))),
        help="publisher start delay, subtracted from every viewer's first-chunk time",
    )
    # Startup join latency is Δ_buffer-bound (3 s) until late-join backfill is tuned; the
    # simulator gates use 3.6 s mean, and a real link adds DHT and QUIC round trips.
    ap.add_argument("--sjl-max-ms", type=float, default=4000.0)
    ap.add_argument("--seed", type=int, default=int(last.get("BS_SEED", "1")))
    ap.add_argument("--profile", default=last.get("BS_NETEM_PROFILE", "clean"))
    ap.add_argument("--raw", help="already-copied results dir (skip docker copy)")
    ap.add_argument("--run-id", default=last.get("BS_RUN_ID"), help="report this run id (default: the last up.sh run)")
    a = ap.parse_args()
    if a.psr_max is None:
        a.psr_max = PROFILE_PSR_MAX.get(a.profile, 0.001)
    global RUN_ID_FILTER
    RUN_ID_FILTER = a.run_id
    out = pathlib.Path(a.out)
    if a.raw:
        raw = pathlib.Path(a.raw)
    else:
        # Kept beside the verdict so a failed run can be read after the fleet is down.
        raw = (out / "raw").resolve()
        copy_results(raw)
    verdict = build_verdict(raw, a.psr_max, a.sjl_max_ms, a.seed, a.profile, a.start_delay_s, churned_hosts())
    out.mkdir(parents=True, exist_ok=True)
    (out / "verdict.json").write_text(json.dumps(verdict, indent=2))
    print(json.dumps({k: v for k, v in verdict.items() if k != "nodes"}, indent=2))
    return 0 if verdict["passed"] else 1


if __name__ == "__main__":
    sys.exit(main())
