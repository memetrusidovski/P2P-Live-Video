#!/usr/bin/env python3
"""Run the Docker fleet over its profiles several times and compare with the baseline.

    fleet/matrix.py run                    every case, 3 runs each
    fleet/matrix.py run --only lossy-churn --runs 5
    fleet/matrix.py accept                 make the last run the baseline
    fleet/matrix.py show

Each case is judged on the median of its runs, as the simulator gates are
(scripts/gates.py). A case marked `gating: False` is reported and never fails
the matrix.
"""
from __future__ import annotations

import argparse
import datetime
import json
import os
import pathlib
import shutil
import subprocess
import sys
import time

FLEET = pathlib.Path(__file__).resolve().parent
SRC = FLEET.parent
sys.path.insert(0, str(SRC / "scripts"))
import gates  # noqa: E402

BASELINE = SRC / "baselines" / "fleet.json"
ACCEPTED = SRC / "baselines" / "fleet_history.jsonl"
OUT = SRC / "results" / "fleet-matrix"

# RULE: A file-ingest case bounds the stream with --max-bytes so the stream ends
#       before the viewers do. The hash check is exact only then.
FILE_ARGS = ["--max-bytes", "25000000", "--duration", "90"]
CASES = {
    "clean": {
        "up": ["--viewers", "20", "--profile", "clean", *FILE_ARGS],
        "wait_s": 110,
        "gating": True,
    },
    # Ch8 §8.3 scenario A on real nodes.
    "lossy-churn": {
        "up": ["--viewers", "20", "--profile", "lossy", *FILE_ARGS],
        "churn": ["--fraction", "0.3", "--at", "30"],
        "wait_s": 80,
        "gating": True,
    },
    # A capped, lossy, far uplink with encoded video in two renditions.
    "cellular": {
        "up": ["--viewers", "12", "--profile", "cellular", "--ingest", "ffmpeg",
               "--renditions", "240p:400,360p:800", "--duration", "70"],
        "env": {"BS_UPLOAD_KBPS": "7000", "BS_PUB_UPLOAD_KBPS": "7000"},
        "wait_s": 85,
        # TODO: Gate the case when the cellular profile meets its starvation limit.
        "gating": False,
    },
}

# Fleet runs are not repeatable, so the floors are wider than the simulator's.
FLEET_FLOOR = {"psr": 0.005, "sjl_mean_s": 0.5}


def run_case(name: str, case: dict, sample: pathlib.Path, index: int) -> dict:
    """Bring the fleet up for one case, wait, tear it down and return the verdict.

    Args:
        name: Case name, a key of CASES.
        case: The case record.
        sample: Video file the publisher streams.
        index: Run number, from 1, used in the log file name.

    Returns:
        The verdict report.py wrote, or `{"error": text}` if a step failed.
    """
    env = {**os.environ, **case.get("env", {})}
    log_path = OUT / f"{name}-{index}.log"
    with log_path.open("w") as log:
        def step(cmd: list[str]) -> bool:
            return subprocess.run(cmd, cwd=SRC, env=env, stdout=log, stderr=subprocess.STDOUT).returncode == 0

        try:
            if not step(["fleet/up.sh", *case["up"], "--file", str(sample), "--no-ports"]):
                return {"error": f"fleet/up.sh failed, see {log_path}"}
            if "churn" in case and not step(["fleet/churn.sh", *case["churn"]]):
                return {"error": f"fleet/churn.sh failed, see {log_path}"}
            time.sleep(case["wait_s"])
            # The publisher writes its report when stopped.
            step(["fleet/down.sh", "--keep-results"])
            verdict_dir = OUT / f"{name}-{index}"
            # The results of an earlier matrix must not mix with the results of this run.
            shutil.rmtree(verdict_dir, ignore_errors=True)
            step([sys.executable, "fleet/report.py", "--out", str(verdict_dir)])
            verdict = verdict_dir / "verdict.json"
            if not verdict.exists():
                return {"error": f"fleet/report.py wrote no verdict, see {log_path}"}
            return json.loads(verdict.read_text())
        finally:
            step(["fleet/down.sh"])


def cmd_run(args: argparse.Namespace) -> int:
    """Run the cases, print one line per case, and write `results/fleet-matrix/last.json`."""
    names = args.only or list(CASES)
    unknown = [n for n in names if n not in CASES]
    if unknown:
        print(f"unknown case: {', '.join(unknown)}; cases are {', '.join(CASES)}", file=sys.stderr)
        return 2
    sample = pathlib.Path(os.environ.get("BS_FLEET_FILE", FLEET / "media" / "sample.mp4")).resolve()
    if not sample.is_file():
        print(f"no sample file {sample}; set BS_FLEET_FILE", file=sys.stderr)
        return 2
    if subprocess.run(["docker", "info"], capture_output=True).returncode != 0:
        print("docker is not available", file=sys.stderr)
        return 2
    OUT.mkdir(parents=True, exist_ok=True)
    baseline = json.loads(BASELINE.read_text()) if BASELINE.exists() else {"scenarios": {}}
    gates.DRIFT_FLOOR.update(FLEET_FLOOR)
    record = {"commit": gates.git_commit(), "date": datetime.datetime.now().isoformat(timespec="seconds"),
              "runs": args.runs, "scenarios": {}}
    failed = drifted = 0
    for name in names:
        case = CASES[name]
        verdicts = []
        for i in range(1, args.runs + 1):
            v = run_case(name, case, sample, i)
            verdicts.append(v)
            psr = v["kpis"]["psr"]["value"] if "kpis" in v else float("nan")
            print(f"  {name} run {i}: {'error' if 'error' in v else 'pass' if v['passed'] else 'fail'}  psr {psr:.4f}", flush=True)
        errors = [v["error"] for v in verdicts if "error" in v]
        good = [v for v in verdicts if "error" not in v]
        kpis = gates.fold(good) if good else {}
        misses = [f"{k} {r['judged']:.4g} {'>' if r['op'] == '<=' else '<'} {r['threshold']:.4g}  runs {[round(x, 4) for x in r['values']]}"
                  for k, r in kpis.items() if not r["pass"]]
        base = baseline["scenarios"].get(name, {})
        drifts = [d for k, r in kpis.items() if k in base and r["pass"] for d in [gates.drift(k, r, base[k])] if d]
        status = "FAIL" if (errors or misses) else "DRIFT" if drifts else "PASS"
        gating = case["gating"]
        if gating and status == "FAIL":
            failed += 1
        elif gating and status == "DRIFT":
            drifted += 1
        print(f"{status:5} {name}{'' if gating else '  (not gating)'}")
        for line in errors + misses:
            print(f"        fail:  {line}")
        for line in drifts:
            print(f"        drift: {line}")
        record["scenarios"][name] = {"status": status, "gating": gating,
                                     "kpis": {k: {"op": r["op"], "threshold": r["threshold"], "values": r["values"], "judged": r["judged"]} for k, r in kpis.items()}}
    record["passed"] = failed == 0 and (drifted == 0 or args.no_drift)
    (OUT / "last.json").write_text(json.dumps(record, indent=1))
    print(f"\n{len(names)} fleet cases, {args.runs} runs each: {failed} failed, {drifted} drifted")
    return 0 if record["passed"] else 1


def cmd_accept(args: argparse.Namespace) -> int:
    """Write the last matrix run to the baseline file."""
    last_path = OUT / "last.json"
    if not last_path.exists():
        print("no run to accept: run `fleet/matrix.py run` first", file=sys.stderr)
        return 2
    last = json.loads(last_path.read_text())
    if last["runs"] < 3:
        print("the last matrix had fewer than 3 runs per case; a baseline needs the spread", file=sys.stderr)
        return 2
    baseline = json.loads(BASELINE.read_text()) if BASELINE.exists() else {"scenarios": {}}
    for n, s in last["scenarios"].items():
        if s["kpis"]:
            baseline["scenarios"][n] = {k: {"op": r["op"], "values": [round(x, 6) for x in r["values"]]} for k, r in s["kpis"].items()}
    baseline["commit"], baseline["date"] = last["commit"], last["date"]
    BASELINE.parent.mkdir(parents=True, exist_ok=True)
    BASELINE.write_text(json.dumps(baseline, indent=1, sort_keys=True) + "\n")
    with ACCEPTED.open("a") as h:
        compact = {"commit": last["commit"], "date": last["date"], "runs": last["runs"], "note": args.note,
                   "scenarios": {n: {k: round(r["judged"], 6) for k, r in s["kpis"].items()} for n, s in last["scenarios"].items()}}
        h.write(json.dumps(compact, sort_keys=True) + "\n")
    print(f"baseline updated for {len(last['scenarios'])} cases: {BASELINE.relative_to(SRC)}")
    return 0


def cmd_show(args: argparse.Namespace) -> int:
    """Print the baseline runs of each KPI beside the last matrix."""
    baseline = (json.loads(BASELINE.read_text()) if BASELINE.exists() else {"scenarios": {}})["scenarios"]
    last_path = OUT / "last.json"
    last = json.loads(last_path.read_text())["scenarios"] if last_path.exists() else {}
    for n in sorted(set(baseline) | set(last)):
        print(n)
        base, cur = baseline.get(n, {}), last.get(n, {}).get("kpis", {})
        for k in sorted(set(base) | set(cur)):
            b = [round(x, 4) for x in base[k]["values"]] if k in base else "-"
            c = [round(x, 4) for x in cur[k]["values"]] if k in cur else "-"
            print(f"  {k:24} last {c}  baseline {b}")
    return 0


def main() -> int:
    """Parse the command line and run the chosen command."""
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    r = sub.add_parser("run", help="run the fleet cases")
    r.add_argument("--runs", type=int, default=3, help="runs per case")
    r.add_argument("--only", nargs="+", metavar="CASE")
    r.add_argument("--no-drift", action="store_true", help="report drift without failing the matrix")
    r.set_defaults(fn=cmd_run)
    a = sub.add_parser("accept", help="make the last run the baseline")
    a.add_argument("--note", default="", help="why the baseline moved, kept in baselines/fleet_history.jsonl")
    a.set_defaults(fn=cmd_accept)
    s = sub.add_parser("show", help="baseline beside the last run")
    s.set_defaults(fn=cmd_show)
    args = ap.parse_args()
    return args.fn(args)


if __name__ == "__main__":
    sys.exit(main())
