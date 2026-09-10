"""Plots from a simulator run's events.jsonl. Requires the [analysis] extra."""
from __future__ import annotations

import json
import pathlib


def load_events(path: pathlib.Path):
    """Load events.jsonl into a pandas DataFrame (None if pandas is missing)."""
    try:
        import pandas as pd
    except ImportError:
        return None
    rows = [json.loads(line) for line in path.read_text().splitlines() if line.strip()]
    return pd.DataFrame(rows)


def plot_run(run_dir: pathlib.Path, out_dir: pathlib.Path | None = None) -> list[pathlib.Path]:
    """Write PSR-over-time and depth-histogram PNGs. Returns the files written.

    Expects events with kind == "starved" (fields: t_us, node, segment) and
    kind == "depth" (fields: t_us, node, tree, depth).
    """
    try:
        import matplotlib
        matplotlib.use("Agg")
        import matplotlib.pyplot as plt
    except ImportError:
        return []
    df = load_events(run_dir / "events.jsonl")
    if df is None or df.empty:
        return []
    out_dir = out_dir or run_dir
    written = []

    if "kind" in df:
        starved = df[df["kind"] == "starved"]
        if not starved.empty:
            s = starved.assign(sec=(starved["t_us"] // 1_000_000)).groupby("sec").size()
            fig, ax = plt.subplots()
            s.plot(ax=ax)
            ax.set_xlabel("virtual time (s)")
            ax.set_ylabel("starved playout intervals")
            ax.set_title("Starvation over time")
            p = out_dir / "psr_over_time.png"
            fig.savefig(p)
            plt.close(fig)
            written.append(p)

        depth = df[df["kind"] == "depth"]
        if not depth.empty:
            latest = depth.sort_values("t_us").groupby(["node", "tree"]).tail(1)
            fig, ax = plt.subplots()
            latest["depth"].plot(kind="hist", bins=range(0, 12), ax=ax)
            ax.set_xlabel("hop depth")
            ax.set_title("Final hop depth distribution")
            p = out_dir / "depth_hist.png"
            fig.savefig(p)
            plt.close(fig)
            written.append(p)
    return written
