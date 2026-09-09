# Fixer Agent

Operating instructions for an agent resolving tracked issues in this specification.
Load this before touching `issues/`.

---

## Mission

You maintain the design specification for a decentralized live-video streaming
protocol — 1,000,000 concurrent viewers, 3–5 s playout deadline, tokenless,
trackerless. This is a specification repo, not an implementation. Your job is to
work through `issues/`, fix them properly in `protocol/`, and record the reasoning
in `solutions/`.

**This will one day be a real system.** Every change must hold from $N = 2$ through
$N = 10^6$, on hardware ranging from a battery-constrained phone to a gigabit-class
super node.

---

## Repository layout and authority

| Path | Role |
|---|---|
| `protocol/` | **Canonical.** Start at `protocol/INDEX.md`. If anything disagrees with `protocol/`, `protocol/` wins. |
| `protocol/appendix_b_parameters.md` | Every tunable constant, with a cross-reference to the section using it. New constants go here. |
| `protocol/appendix_d_frame_registry.md` | Normative wire-format registry. For a frame, the **byte diagram is the specification**; prose is commentary. |
| `protocol/schemas/p2p_live.proto` | Non-normative field reference only. Never normative. |
| `issues/` | Open defects. `issues/README.md` is the index. |
| `solutions/` | Why resolved problems were solved that way. `solutions/README.md` carries the accumulated design principles. |
| `thoughts/` | Historical exploratory notes. Superseded, retained for their reasoning. |

**Read `solutions/README.md` before starting.** It lists twelve design principles
derived from previously closed issues, and the recurring failure patterns that
produced them. Most new findings are instances of one of those patterns.

Current state: ISSUE-001–018 closed, SOLUTION-001–018 recorded. ISSUE-019–039 open.

---

## Step 0 — Map the open set before fixing anything

**Do this first, once, before starting any individual issue.**

Issues overlap. Several describe different symptoms of one underlying gap, and
several have fixes that touch the same section, formula, or frame. An agent that
starts at the lowest number and works upward will do redundant work, and worse,
will produce two partial fixes to the same mechanism that do not compose.

1. Read the whole of `protocol/` once.
2. Read **every** open issue file — titles and summaries at minimum.
3. Build an explicit overlap map. For each issue record:
   - which `protocol/` files its fix would touch
   - which parameters, formulas, or frames it would change
   - which other issues share any of those
4. Group the overlapping ones into clusters. **Resolve a cluster together, as one
   coherent design change**, then write one solution file per issue or a single
   shared solution referenced by each.
5. Write the cluster map into your working notes and state it to the user before
   you begin fixing.

Clusters already identified in `issues/README.md` — verify these and find the rest
yourself, do not assume the list is complete:

- **020 / 028** — tree-aware peer records
- **026 / 038** — forest-ladder inputs
- **034 / 035 / 039** — identity cost feeding eviction

Also check for issues that are *prerequisites* rather than duplicates: ISSUE-019
(tree→layer mapping) underpins anything touching per-tree bitrate, shed order, or
$K_v$, so it likely has to be settled before its dependants.

Within a cluster, still work one issue at a time when writing — but design the fix
for the whole cluster up front.

---

## Working loop

1. Pick the next cluster (or standalone issue) from your Step 0 map.
2. Read the issue file in full.
3. **Verify against the spec. Never trust a `Status: Resolved` header.**
4. Decide:
   - Genuinely and completely fixed → write the solution, delete the issue file.
   - Partially fixed → complete it in `protocol/`, then solution, then delete.
   - Not fixed → fix it, verify, solution, delete.
5. If you discover a *different* problem, **file a new `ISSUE-NNN-<slug>.md`**
   rather than expanding scope. Note which issue's verification surfaced it, and
   re-check your overlap map — a new issue may join an existing cluster.
6. Update `issues/README.md` and `solutions/README.md`.
7. Verify your own change (below) before moving on.

---

## Verification standard

Prose review is not verification. Of the first eighteen issues, **all** were marked
resolved; fourteen fixes were incomplete and two were regressions that left the spec
worse than before the fix. Assume the same rate.

- **Compute the arithmetic.** Evaluate every formula across its whole input range,
  not just the extreme it was designed for. A dropped `max()` once made a scaling
  rule return 1 where the baseline was 8, for every input in 17–79.
- **Read code as code.** A previous XDP listing contained an unsigned underflow
  making the rate limiter fail *open*, an IP-options misparse allowing trivial
  filter evasion, and a `PERCPU_ARRAY` making a "global" ceiling 32× its stated
  value. None of those are visible in prose.
- **Check byte layouts sum correctly**, and that every field the prose depends on
  actually exists in the diagram.
- **Trace degraded and fallback modes through every state machine they touch.**
  Layer shedding was internally complete and still left peers stuck in `CONNECTING`.
- **Check every described trigger has a real wire encoding.**
- **Check link integrity and derived text.** INDEX entries and README summary lines
  restate facts without owning them, so they drift silently.

After fixing, re-run the same checks against your own change.

---

## Quality bar

Before accepting any fix, ask:

- What happens at $N = 2$? At $N = 10^6$? At $K_v = 1$? At $K_v = 10{,}000$?
- Is this input attacker-controlled? Can a hostile peer or DHT guardian forge,
  replay, or withhold it?
- Does it compose with fixes made in *other* chapters at *other* scales?
- Is this constant actually a *share* of something that moves? ($K_v$ moves with
  $M$, which moves with $N$.)
- Does relaxing this constraint remove a property nobody wrote down?

---

## Recurring defect patterns

Check these explicitly; they account for most findings.

1. Constants that must satisfy a joint relation, set independently in different
   chapters. Ask: does any single expression own the total?
2. A fix validated at one scale composed with a fix validated at another.
3. Prose amended, byte layout not.
4. A trigger described but never given a wire encoding.
5. A rule derived from small-$N$ intuition, written as unconditional.
6. Sentinel values conflating transient with permanent causes.
7. Fallback chains written as substitutions instead of cost-ordered unions.
8. Weights not re-derived after changing the transform they weight.

---

## File formats

**Issue** — `ISSUE-NNN-<slug>.md`

Header: Status, Priority, Component, Affects, File. Then Summary, Detailed
Description (with worked numbers), Impact, Proposed Fix, Effort. Cross-reference
related issues and name the cluster if it belongs to one.

**Solution** — `SOLUTION-NNN-<slug>.md`

Header: Closes, Lives in, Class. Then:

- The problem in one line
- The decision
- **Why this and not the alternatives** — the rejected options and the reason
- Defects found during verification, with numbers
- The generalisable lesson
- Residual risk
- **Validation owed (Chapter 8)** — anything chosen by argument rather than measurement

A solution captures *research reasoning*, not a changelog. A reader must be able to
tell what would break if they reverted the decision.

---

## Language

ASD-STE100 (Simplified Technical English) is encouraged in issue text and reasoning
where it improves precision: short sentences, one instruction per sentence,
consistent terminology, active voice, no ambiguous pronouns. Do not apply it to
mathematical derivations or narrative passages in `protocol/`, where it flattens
meaning.

---

## Constraints

- **Do not commit or push** unless explicitly asked. Leave changes staged for review.
- Do not expand scope beyond the issue or cluster in hand; file a ticket instead.
- Report honestly. If a fix is partial, or a claim is unverified, say so plainly.
- State plainly when a prior fix was wrong. Do not soften it.
- Deleting an issue file is the *last* step, after the fix is verified and the
  solution is written.
