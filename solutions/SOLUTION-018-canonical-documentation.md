# SOLUTION-018: One Canonical Specification

**Closes:** ISSUE-018 (Medium)
**Lives in:** `README.md`, `thoughts/thoughts.md`, `thoughts/main-issues.md`, `protocol/appendix_b_parameters.md`, `protocol/INDEX.md`, `protocol/chapter4/4.3_hybrid_push_pull/3_rarest_first_heuristics.md`, `protocol/chapter7/7.1_source_pinning/2_replay_attack_prevention.md`
**Class:** Repo-wide authority and consistency

---

## The problem in one line

Three design generations coexisted and contradicted each other — `thoughts/` prescribed an unstructured mesh, `protocol/` proved mesh-pull cannot meet the latency bound and mandated trees, and the top-level README described a third architecture (latency clusters with super-peers) that appears nowhere in the spec — with latency targets of &lt;30 s, 2–10 s, and 3–5 s respectively.

## The decision

**`protocol/` is canonical.** Everything else defers to it explicitly.

Crucially, the older documents were **not deleted**. They carry banners naming the specific divergences and linking to the sections that supersede them. The reasoning in `thoughts/` is why the spec looks the way it does — "Tree Structures Are Fragile" is a real objection, and the answer is not that it was wrong but that sub-250 ms healing addresses it without abandoning trees. Deleting the objection would delete the justification for the answer.

`thoughts/main-issues.md` goes further, sorting its own mitigations three ways: **adopted**, **changed by the spec** (with the reason), and **not yet adopted** (open future work). That structure is worth keeping as new work lands — a risk catalogue whose disposition is tracked is an asset; one that silently drifts is a liability.

## The substantive fixes underneath the banners

Three were genuine defects rather than editorial drift:

**Timestamp units.** Every frame layout carries a 64-bit **microsecond** timestamp, while Ch7's replay validator compared `int(time.time())` — seconds — against a ±2.0 drift bound. Off by $10^6$: the check would reject every legitimate manifest. Now microseconds on both sides.

**Unbounded replay cache.** `verified_segments_cache` grew without limit for the life of a broadcast — at 1 segment/s, an 8-hour stream accumulates ~29,000 entries and never releases them. Now bounded to a 120-segment window.

**Membership alone cannot reject a replay.** With eviction added, a membership test becomes *wrong*: an attacker replays a segment old enough to have been evicted, finds it absent from the cache, and it validates. An explicit ordering watermark (`highest_verified_seq`) was required alongside the bound. Fixing the leak created the hole; both were needed.

**Naming a collision.** The 100 ms scheduler cycle was described in one place as "the same as $\tau_{\text{gossip}}$", which is 1000 ms. They are deliberately different — the scheduler re-evaluates urgency ten times per gossip round against availability it already holds, so a block that becomes urgent mid-round is pulled immediately. Now named $\tau_{\text{sched}}$, with the 10× ratio explained rather than looking like a typo.

## One stale reference found during verification

`INDEX.md` still described `2_systematic_dispersion.md` as "Distributing 10% parity across multi-forest parents" — the exact FEC figure this issue corrected everywhere else. Index and README summary lines are easy to miss because they duplicate content without being the content. Corrected here and in the chapter README.

## The generalisable lesson

**A repository needs exactly one document that is allowed to be right.** Once that is declared, every other document's job changes from "describe the system" to "point at the spec and record what it used to say and why." The failure mode this prevents is not disagreement — it is *undetectable* disagreement, where a reader has no way to tell which of three answers is current.

The corollary worth acting on: **derived text drifts silently.** Index entries, README summaries, and chapter tables of contents restate facts without owning them, so they are never updated when the fact changes and never fail a review of the owning document. Any change to a parameter should be grepped for across the whole repo, not just edited where it is defined.

## Standing practice this establishes

* `protocol/` is normative; `schemas/` is a non-normative field reference (SOLUTION-012); `thoughts/` is historical; `issues/` tracks open defects; `solutions/` records why resolved ones were solved the way they were.
* Every parameter lives in `appendix_b_parameters.md` with a cross-reference to the section that uses it.
* All timestamps are unsigned 64-bit microseconds, everywhere, with no exceptions.
