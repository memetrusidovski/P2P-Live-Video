#!/usr/bin/env python3
"""Run the simulator gates over several seeds and compare them with the baseline.

    scripts/gates.py run                 every scenario on its own seed (quick)
    scripts/gates.py run --seeds 5       seeds 1 to 5, judged on the median (full)
    scripts/gates.py run --only lossy_links churn_storm --seeds 5
    scripts/gates.py accept              make the last multi-seed run the baseline
    scripts/gates.py show [SCENARIO]     baseline beside the last run

A gate fails when a KPI misses its threshold. A gate drifts when a KPI moves
past the spread the baseline seeds show, toward its threshold. Drift fails a
multi-seed run and is a warning on a single seed.

A scenario that misses a threshold on its own seed is run over 5 seeds at once.
The miss fails the run only if the 5 seeds fail or drift too.

`baselines/known.json` lists two kinds of exception, each with its reason. Both
apply to multi-seed runs only.
  seed_bound      a check that holds only on the seed of the scenario file; skipped
  known_failures  a fault not yet fixed; reported as KNOWN, with a note when the
                  KPI passes, so the entry is removed with the fix
"""
from __future__ import annotations

import argparse
import concurrent.futures
import datetime
import json
import os
import pathlib
import statistics
import subprocess
import sys
import time

SRC = pathlib.Path(__file__).resolve().parent.parent
SCENARIOS = SRC / "scenarios"
BASELINE = SRC / "baselines" / "sim.json"
ACCEPTED = SRC / "baselines" / "history.jsonl"
KNOWN = SRC / "baselines" / "known.json"
OUT = SRC / "results" / "gates"
SIM = SRC / "target" / "sim" / "bs-sim"

# RULE: These KPIs count faults. One seed with a fault fails the gate; the median
#       must not hide it.
EXACT = {"hard_invariant_violations", "hash_mismatches", "viewers_never_played", "survivors_stalled"}

# Facts tracked beside the KPIs. A fact has no threshold, so a move is reported
# and never fails the run. Lower is better for each of them.
FACTS = ("psr_spec", "layer_miss", "soft_violations")

# Share of the baseline spread a KPI can move past the worst baseline seed.
DRIFT_SPREAD = 0.5
# Share of the baseline median a KPI can move past the worst baseline seed.
DRIFT_RELATIVE = 0.10
# Smallest move that counts as drift, per KPI, in the unit of the KPI.
DRIFT_FLOOR = {
    "psr": 0.002,
    "psr_spec": 0.005,
    "cdo": 0.005,
    "active_fraction": 0.01,
    "depth_max": 1.0,
    "depth_mean": 0.3,
    "sjl_mean_s": 0.2,
    "sjl_p95_s": 0.5,
    "repair_median_s": 0.2,
    "repair_p95_s": 1.0,
}
DRIFT_FLOOR_DEFAULT = 0.01


def git_commit() -> str:
    """Return the short commit hash of the work tree, with a mark if it is dirty."""
    try:
        head = subprocess.run(["git", "rev-parse", "--short", "HEAD"], cwd=SRC, capture_output=True, text=True, check=True).stdout.strip()
        dirty = subprocess.run(["git", "status", "--porcelain"], cwd=SRC, capture_output=True, text=True, check=True).stdout.strip()
        return head + ("+dirty" if dirty else "")
    except (OSError, subprocess.CalledProcessError):
        return "unknown"


def run_one(scenario: str, seed: int | None, reuse: bool = False) -> dict:
    """Run one scenario on one seed.

    Args:
        scenario: Scenario name, without the directory and the extension.
        seed: Seed to run. None runs the seed the scenario file states.
        reuse: Read the verdict of an earlier run if one exists, and do not run.

    Returns:
        The verdict of the run with `seconds` added, or `{"error": text}` if the
        simulator wrote no verdict.
    """
    if seed is None:
        out = SRC / "results" / scenario
        cmd = [str(SIM), "run", str(SCENARIOS / f"{scenario}.toml"), "--out", str(out), "-q"]
    else:
        out = OUT / scenario / f"seed{seed}"
        cmd = [str(SIM), "run", str(SCENARIOS / f"{scenario}.toml"), "--seed", str(seed), "--no-events", "--out", str(out), "-q"]
    verdict = out / "verdict.json"
    if reuse and verdict.exists():
        return {**json.loads(verdict.read_text()), "seconds": 0.0}
    verdict.unlink(missing_ok=True)
    start = time.time()
    p = subprocess.run(cmd, cwd=SRC, capture_output=True, text=True)
    secs = time.time() - start
    if not verdict.exists():
        return {"error": (p.stderr or p.stdout)[-2000:], "seconds": secs}
    v = json.loads(verdict.read_text())
    v["seconds"] = secs
    return v


def worse(op: str, a: float, b: float) -> bool:
    """Return True if value `a` is further from the goal of the KPI than `b`."""
    return a > b if op == "<=" else a < b


def fold(verdicts: list[dict]) -> dict:
    """Fold the verdicts of one scenario into one record per KPI.

    Args:
        verdicts: One verdict per seed, each from a run that wrote a verdict.

    Returns:
        Map of KPI name to `{op, threshold, values, judged, pass}`. `judged` is the
        median over the seeds, or the worst seed for an EXACT KPI. Tracked facts
        have no threshold and always pass.
    """
    kpis: dict[str, dict] = {}
    for name, first in verdicts[0]["kpis"].items():
        values = [v["kpis"][name]["value"] for v in verdicts if name in v["kpis"]]
        op, threshold = first["op"], first["threshold"]
        if name in EXACT:
            judged = max(values) if op == "<=" else min(values)
        else:
            judged = statistics.median(values)
        ok = judged <= threshold if op == "<=" else judged >= threshold
        kpis[name] = {"op": op, "threshold": threshold, "values": values, "judged": judged, "pass": ok}
    for name in FACTS:
        values = [v["facts"][name] for v in verdicts if isinstance(v.get("facts", {}).get(name), (int, float))]
        if len(values) == len(verdicts):
            kpis[name] = {"op": "<=", "threshold": None, "values": values, "judged": statistics.median(values), "pass": True}
    return kpis


def drift(name: str, cur: dict, base: dict) -> str | None:
    """Describe the drift of one KPI against its baseline record.

    Args:
        name: KPI name.
        cur: Record of the KPI from `fold` for the present run.
        base: Baseline record of the KPI: `{op, values}`.

    Returns:
        One line of text if the judged value is past the worst baseline seed by
        more than the allowance. None if it is not.
    """
    op, values = base["op"], base["values"]
    worst = max(values) if op == "<=" else min(values)
    spread = max(values) - min(values)
    allowance = max(
        DRIFT_SPREAD * spread,
        DRIFT_RELATIVE * abs(statistics.median(values)),
        DRIFT_FLOOR.get(name, DRIFT_FLOOR_DEFAULT),
    )
    limit = worst + allowance if op == "<=" else worst - allowance
    if not worse(op, cur["judged"], limit):
        return None
    return (
        f"{name} {cur['judged']:.4g}, baseline median {statistics.median(values):.4g} "
        f"range {min(values):.4g} to {max(values):.4g}, allowed to {limit:.4g}"
    )


def load_baseline() -> dict:
    """Return the committed baseline, or an empty one if the file does not exist."""
    if BASELINE.exists():
        return json.loads(BASELINE.read_text())
    return {"scenarios": {}}


def cmd_run(args: argparse.Namespace) -> int:
    """Run the gates, print one line per scenario, and write `results/gates/last.json`."""
    names = sorted(p.stem for p in SCENARIOS.glob("*.toml"))
    if args.only:
        unknown = [n for n in args.only if n not in names]
        if unknown:
            print(f"unknown scenario: {', '.join(unknown)}", file=sys.stderr)
            return 2
        names = args.only
    seeds: list[int | None] = [None] if args.seeds <= 1 else list(range(args.first_seed, args.first_seed + args.seeds))
    multi = seeds != [None]

    build = subprocess.run(["cargo", "build", "-q", "-p", "bs-sim", "--profile", "sim"], cwd=SRC)
    if build.returncode != 0:
        return build.returncode

    OUT.mkdir(parents=True, exist_ok=True)
    start = time.time()
    jobs = [(n, s) for n in names for s in seeds]
    verdicts: dict[str, list[dict]] = {n: [] for n in names}
    with concurrent.futures.ThreadPoolExecutor(max_workers=args.jobs) as pool:
        futures = {pool.submit(run_one, n, s, args.reuse): (n, s) for n, s in jobs}
        done = 0
        for f in concurrent.futures.as_completed(futures):
            n, s = futures[f]
            v = f.result()
            v["run_seed"] = s
            verdicts[n].append(v)
            done += 1
            if sys.stderr.isatty():
                print(f"\r  {done}/{len(jobs)} runs", end="", file=sys.stderr)
    if sys.stderr.isatty():
        print("\r" + " " * 20 + "\r", end="", file=sys.stderr)

    baseline = load_baseline()
    known = json.loads(KNOWN.read_text()) if multi and KNOWN.exists() else {}
    failed = drifted = known_count = 0
    failed_names: list[str] = []
    errored = False
    record = {"commit": git_commit(), "date": datetime.datetime.now().isoformat(timespec="seconds"),
              "seeds": [s for s in seeds if s is not None], "scenarios": {}}
    for n in names:
        errors = [v for v in verdicts[n] if "error" in v]
        good = [v for v in verdicts[n] if "error" not in v]
        lines = [f"simulator wrote no verdict on seed {v['run_seed']}: {v['error'].strip().splitlines()[-1] if v['error'].strip() else 'no output'}" for v in errors]
        kpis = fold(good) if good else {}
        for k in known.get("seed_bound", {}).get(n, {}):
            kpis.pop(k, None)
        expected = known.get("known_failures", {}).get(n, {})
        fixed = [f"{k} passes on these seeds: if the fault is fixed, remove it from {KNOWN.relative_to(SRC)}" for k in expected if k in kpis and kpis[k]["pass"]]
        still = [f"{k} {kpis[k]['judged']:.4g}, limit {kpis[k]['threshold']:.4g}: {why}" for k, why in expected.items() if k in kpis and not kpis[k]["pass"]]
        for k in expected:
            if k in kpis:
                kpis[k]["pass"] = True
        misses = [f"{k} {r['judged']:.4g} {'>' if r['op'] == '<=' else '<'} {r['threshold']:.4g}" + (f"  seeds {[round(x, 4) for x in r['values']]}" if multi else "")
                  for k, r in kpis.items() if not r["pass"]]
        base = baseline["scenarios"].get(n, {})
        drifts, notes = [], []
        for k, r in kpis.items():
            if k in base and r["pass"]:
                d = drift(k, r, base[k])
                if d:
                    (notes if r["threshold"] is None else drifts).append(d)
        status = "FAIL" if (errors or misses) else "DRIFT" if drifts else "KNOWN" if still else "PASS"
        if status == "FAIL":
            failed += 1
            failed_names.append(n)
            errored = errored or bool(errors)
        elif status == "DRIFT":
            drifted += 1
        elif status == "KNOWN":
            known_count += 1
        new = "" if base or not baseline["scenarios"] else "  (no baseline)"
        secs = max((v["seconds"] for v in verdicts[n]), default=0.0)
        print(f"{status:5} {n:28} {secs:5.0f}s{new}")
        for line in lines + misses:
            print(f"        fail:  {line}")
        for line in drifts:
            print(f"        drift: {line}")
        for line in still:
            print(f"        known: {line}")
        for line in fixed + notes:
            print(f"        note:  {line}")
        record["scenarios"][n] = {"status": status, "kpis": {k: {"op": r["op"], "threshold": r["threshold"], "values": r["values"], "judged": r["judged"]} for k, r in kpis.items()}}

    drift_fails = multi and not args.no_drift
    record["passed"] = failed == 0 and not (drift_fails and drifted)
    (OUT / "last.json").write_text(json.dumps(record, indent=1))
    with (OUT / "history.jsonl").open("a") as h:
        compact = {"commit": record["commit"], "date": record["date"], "seeds": record["seeds"],
                   "scenarios": {n: {k: r["judged"] for k, r in s["kpis"].items()} for n, s in record["scenarios"].items()}}
        h.write(json.dumps(compact) + "\n")
    seed_text = f"seeds {seeds[0]} to {seeds[-1]}" if multi else "own seed"
    print(f"\n{len(names)} gates, {seed_text}: {failed} failed, {drifted} drifted, {known_count} known failures, {time.time() - start:.0f} s")
    if drifted and not drift_fails:
        print("drift is a warning on a single seed; run with --seeds 5 to judge it")
    if not baseline["scenarios"]:
        print("no baseline yet: run with --seeds 5, then `scripts/gates.py accept`")
    if failed and not multi and not errored and not args.no_recheck:
        # RULE: One seed can land on the tail of a KPI that the code did not move.
        #       A miss on the own seed fails the run only if five seeds confirm it.
        print(f"\nchecking {', '.join(failed_names)} over 5 seeds: a miss on one seed can be the seed")
        sys.stdout.flush()
        again = subprocess.run([sys.executable, __file__, "run", "--seeds", "5", "--jobs", str(args.jobs), "--only", *failed_names])
        if again.returncode == 0:
            print("the own-seed misses above are seed noise: five seeds pass with no drift")
        return again.returncode
    return 0 if record["passed"] else 1


def cmd_accept(args: argparse.Namespace) -> int:
    """Write the last multi-seed run to the baseline file."""
    last_path = OUT / "last.json"
    if not last_path.exists():
        print("no run to accept: run `scripts/gates.py run --seeds 5` first", file=sys.stderr)
        return 2
    last = json.loads(last_path.read_text())
    if len(last["seeds"]) < 3:
        print("the last run had fewer than 3 seeds; a baseline needs the spread", file=sys.stderr)
        return 2
    failing = [n for n, s in last["scenarios"].items() if s["status"] == "FAIL"]
    if failing and not args.force:
        print(f"the last run failed {', '.join(failing)}; fix them or pass --force", file=sys.stderr)
        return 2
    baseline = load_baseline()
    # A run of some scenarios replaces those scenarios only.
    for n, s in last["scenarios"].items():
        baseline["scenarios"][n] = {k: {"op": r["op"], "values": [round(x, 6) for x in r["values"]]} for k, r in s["kpis"].items()}
    known = {p.stem for p in SCENARIOS.glob("*.toml")}
    baseline["scenarios"] = {n: baseline["scenarios"][n] for n in sorted(baseline["scenarios"]) if n in known}
    baseline["commit"], baseline["date"], baseline["seeds"] = last["commit"], last["date"], last["seeds"]
    BASELINE.parent.mkdir(parents=True, exist_ok=True)
    BASELINE.write_text(json.dumps(baseline, indent=1, sort_keys=True) + "\n")
    with ACCEPTED.open("a") as h:
        compact = {"commit": last["commit"], "date": last["date"], "seeds": last["seeds"], "note": args.note,
                   "scenarios": {n: {k: round(r["judged"], 6) for k, r in s["kpis"].items()} for n, s in last["scenarios"].items()}}
        h.write(json.dumps(compact, sort_keys=True) + "\n")
    print(f"baseline updated for {len(last['scenarios'])} scenarios: {BASELINE.relative_to(SRC)}")
    return 0


def cmd_show(args: argparse.Namespace) -> int:
    """Print the baseline median and range of each KPI beside the last run."""
    baseline = load_baseline()["scenarios"]
    last_path = OUT / "last.json"
    last = json.loads(last_path.read_text())["scenarios"] if last_path.exists() else {}
    names = [args.scenario] if args.scenario else sorted(set(baseline) | set(last))
    for n in names:
        print(n)
        base, cur = baseline.get(n, {}), last.get(n, {}).get("kpis", {})
        for k in sorted(set(base) | set(cur)):
            b = base.get(k)
            c = cur.get(k)
            btxt = f"{statistics.median(b['values']):9.4g}  [{min(b['values']):.4g}, {max(b['values']):.4g}]" if b else "        -"
            ctxt = f"{c['judged']:9.4g}" if c else "        -"
            limit = c["threshold"] if c else None
            ltxt = f"{c['op']} {limit:.4g}" if limit is not None else ""
            print(f"  {k:28} last {ctxt}  baseline {btxt}  {ltxt}")
    return 0


def main() -> int:
    """Parse the command line and run the chosen command."""
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    r = sub.add_parser("run", help="run the gates")
    r.add_argument("--seeds", type=int, default=1, help="seeds per scenario; 1 runs the seed in the scenario file")
    r.add_argument("--first-seed", type=int, default=1)
    r.add_argument("--only", nargs="+", metavar="SCENARIO")
    r.add_argument("--jobs", type=int, default=max(1, (os.cpu_count() or 4) - 2), help="simulator runs at the same time")
    r.add_argument("--reuse", action="store_true", help="judge the verdicts of the last run again without a new run")
    r.add_argument("--no-recheck", action="store_true", help="do not run a scenario over 5 seeds when it misses on its own seed")
    r.add_argument("--no-drift", action="store_true", help="report drift without failing the run")
    r.set_defaults(fn=cmd_run)
    a = sub.add_parser("accept", help="make the last multi-seed run the baseline")
    a.add_argument("--force", action="store_true", help="accept a run with failed gates")
    a.add_argument("--note", default="", help="why the baseline moved, kept in baselines/history.jsonl")
    a.set_defaults(fn=cmd_accept)
    s = sub.add_parser("show", help="baseline beside the last run")
    s.add_argument("scenario", nargs="?")
    s.set_defaults(fn=cmd_show)
    args = ap.parse_args()
    return args.fn(args)


if __name__ == "__main__":
    sys.exit(main())
