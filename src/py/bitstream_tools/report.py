"""Summarise simulator / fleet verdicts.

Usage: bs-report results/   (reads every results/*/verdict.json)
Exit code is non-zero if any scenario failed or no verdicts were found.
"""
from __future__ import annotations

import json
import pathlib
import sys


def load_verdicts(root: pathlib.Path) -> list[dict]:
    if not root.is_dir():
        return []
    out = []
    for p in sorted(root.glob("*/verdict.json")):
        try:
            out.append(json.loads(p.read_text()))
        except (OSError, json.JSONDecodeError) as e:
            out.append({"scenario": p.parent.name, "passed": False, "error": str(e), "kpis": {}})
    return out


def format_table(verdicts: list[dict]) -> str:
    lines = []
    for v in verdicts:
        status = "PASS" if v.get("passed") else "FAIL"
        lines.append(f"{status}  {v.get('scenario', '?'):<28} seed={v.get('seed', '?')}")
        for name, k in sorted(v.get("kpis", {}).items()):
            mark = "ok " if k.get("pass") else "BAD"
            lines.append(f"      {mark} {name:<20} {k.get('value'):>12.6g} {k.get('op', '')} {k.get('threshold')}")
        viol = v.get("invariant_violations") or []
        if viol:
            lines.append(f"      invariant violations: {len(viol)}")
            for x in viol[:5]:
                lines.append(f"        - {x}")
        if "error" in v:
            lines.append(f"      error: {v['error']}")
        if not v.get("passed") and v.get("repro"):
            lines.append(f"      repro: {v['repro']}")
    return "\n".join(lines)


def main(argv: list[str] | None = None) -> int:
    argv = sys.argv[1:] if argv is None else argv
    root = pathlib.Path(argv[0] if argv else "results")
    verdicts = load_verdicts(root)
    if not verdicts:
        print(f"no verdicts under {root}")
        return 1
    print(format_table(verdicts))
    failed = [v for v in verdicts if not v.get("passed")]
    print(f"\n{len(verdicts) - len(failed)}/{len(verdicts)} scenarios passed")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
