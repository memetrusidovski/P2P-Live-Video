# bitstream-tools (Python)

Support tooling for the Rust implementation. No protocol logic lives here.

- `bitstream_tools/wire.py`: an independent decoder written from the Appendix D
  byte diagrams. `tests/test_golden.py` checks it against the Rust golden
  vectors in `../crates/bs-wire/tests/vectors/frames.json`, so the two
  implementations cross-check each other.
- `bitstream_tools/report.py`: `bs-report results/` summarises simulator and
  fleet verdicts and exits non-zero on any failure.
- `bitstream_tools/analyze.py`: plots from `events.jsonl` (needs `uv sync --extra analysis`).

```
uv sync
uv run pytest -q
uv run bs-report ../results
```
