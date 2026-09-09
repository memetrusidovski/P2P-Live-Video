# ISSUE-018: Three Unreconciled Documentation Generations (README / thoughts / protocol)

**Status:** Resolved  
**Priority:** Medium  
**Component:** Repo-wide — README.md, thoughts/, protocol/ appendices  
**Affects:** Anyone reading the repo; spec authority is ambiguous  
**File:** `README.md`, `thoughts/thoughts.md`, `thoughts/main-issues.md`, `protocol/appendix_b_parameters.md`

---

## Summary

The repo contains three design generations that contradict each other:

- **Topology:** `thoughts/thoughts.md` ("Tree Structures Are Fragile") prescribes a mesh; `protocol/` ch1.1.4 proves mesh-pull fails low-latency bounds and mandates structured trees; the top-level README describes a third architecture (latency clusters of 50–200 peers with super-peers) that appears nowhere in the spec.
- **Latency targets:** README 2–10 s; thoughts.md < 30 s; protocol/ 3–5 s playout with a 4.0 s buffer.
- **FEC overhead:** Appendix B hardcodes a 10% ceiling; ch4.2.2 specifies adaptive 5–30%.
- **Timestamp units:** microseconds in the validation/PoU/STREAM_END frames vs `int(time.time())` seconds (±2.0 s drift) in ch7.1's validator.
- **Parameters:** τ_ttl = 180 s (Appendix B) vs "3 to 5 minutes" (main-issues Risk 5); τ_gossip = 1000 ms (Appendix B) vs the ch4.3.3 scheduler's 100 ms cycle being called "same as τ_gossip" elsewhere.

## Proposed Fix

Declare **`protocol/` the canonical spec**; make everything else defer to it:

1. Add a status banner to `README.md` and `thoughts/*.md`: historical/exploratory notes, superseded by `protocol/` where they disagree; update README's architecture summary and latency budget (3–5 s) to match the spec.
2. FEC: Appendix B becomes "adaptive 5–30%, default 10%" matching ch4.2.2.
3. Timestamps: microseconds (`uint64`) everywhere; fix ch7.1's validator to compare µs with ±2,000,000 µs drift.
4. Parameters: fix stale τ_ttl/τ_gossip mentions; distinguish the 100 ms scheduler cycle (`τ_sched`) from the 1000 ms bitfield gossip (`τ_gossip`).
5. Note in thoughts/main-issues.md which mitigations were adopted into protocol/ and which remain future work (Vivaldi coordinates, multi-source ingest, 80/20 random links, delta gossip, hysteresis timers, zero-knowledge chunks).

---

## Resolution

`protocol/` is now declared canonical and the other generations defer to it:

- `README.md` — rewritten to describe the actual specified architecture (multi-forest, S/Kademlia, HyParView, hybrid push-pull, RaptorQ, TFT/PoU, emergent relays) instead of the superseded cluster/super-peer mesh; latency objective corrected to the 3–5 s playout deadline; a prominent banner points to `protocol/INDEX.md` as canonical and marks `thoughts/` as historical; attack table, implementation directions, and future work updated to match the spec.
- `thoughts/thoughts.md` — historical banner added, explicitly flagging the two divergences that matter (mesh-vs-tree topology, "<30 s" latency) with links to the spec sections that supersede them.
- `thoughts/main-issues.md` — historical banner added with a three-way breakdown: which mitigations were adopted, which the spec changed (mobile PoW exemption superseded by node classes; zero-knowledge chunks not adopted; FEC 10% → adaptive 5–30%; TTL wording → 180 s), and which remain open future work (Vivaldi/ASN locality, multi-source ingest, 80/20 random links, delta gossip, routing hysteresis).
- `protocol/appendix_b_parameters.md` — FEC overhead corrected to adaptive 5–30% (fixed under ISSUE-013); τ_sched = 100 ms added as distinct from τ_gossip = 1000 ms.
- `protocol/chapter4/4.3_hybrid_push_pull/3_rarest_first_heuristics.md` — scheduler cycle named τ_sched with an explanation of why it is 10× faster than the gossip interval.
- `protocol/chapter7/7.1_source_pinning/2_replay_attack_prevention.md` — validator converted to microsecond timestamps (±2,000,000 µs drift) matching every frame layout; the unbounded `verified_segments_cache` is now bounded with an eviction window, and an explicit ordering check was added since membership alone cannot reject an evicted old segment.
