# Protocol Issues

All known design issues identified through protocol review. Each file is a self-contained issue with full context, impact analysis, and a proposed fix.

---

## Open Issues

| ID | Title | Priority | Component | Affects |
|---|---|---|---|---|
| [ISSUE-001](ISSUE-001-joining-state-threshold.md) | JOINING state machine blocks streams with fewer than 5 peers | **Critical** | Ch1 — Peer Lifecycle | N < 5 (every stream at launch) |
| [ISSUE-002](ISSUE-002-hardcoded-forest-size-m6.md) | Hardcoded M=6 forces source to bear full CDN load during cold-start | **High** | Ch1 — Multi-Forest | N < ~18 |
| [ISSUE-004](ISSUE-004-late-joiner-live-edge-sync-undefined.md) | Late-joiner live-edge sync is undefined in the protocol | **High** | Ch2/Ch4 — Registration / Media | Every viewer joining mid-stream |
| [ISSUE-007](ISSUE-007-orthogonal-placement-limits-super-nodes.md) | Orthogonal Placement Rule restricts super nodes to 1 of 6 trees | **High** | Ch1 — Multi-Forest | 1 Gbps+ nodes, 10 Gbps servers |
| [ISSUE-003](ISSUE-003-tft-join-queue-simultaneous-viewers.md) | TFT optimistic unchoke creates a join queue for simultaneous viewers | **Medium** | Ch5 — TFT Incentives | Stream launches, shared links |
| [ISSUE-006](ISSUE-006-empty-passive-set-early-growth.md) | Empty passive set during early growth makes churn recovery slow | **Medium** | Ch3 — HyParView | N < ~35 |
| [ISSUE-009](ISSUE-009-active-set-gossip-does-not-scale-with-fan-out.md) | Active set gossip cap (c_a=8) does not scale with tree fan-out | **Medium** | Ch3/Ch4 — Gossip / Media | Super nodes with K_v >> 8 |
| [ISSUE-010](ISSUE-010-collusion-detection-false-positives-small-n.md) | Collusion detection false positives flag legitimate peers at small N | **Medium** | Ch5 — Reputation Auditing | N < ~20 |
| [ISSUE-005](ISSUE-005-relay-warmup-delay.md) | New relay nodes have no data to relay for ~1 second after joining | **Low** | Ch1/Ch4 — Forest / Media | Rapid growth phases |
| [ISSUE-008](ISSUE-008-capacity-score-log-compression.md) | CapacityScore log compression undersells high-bandwidth nodes | **Low** | Ch1 — Parent Selection | Mixed-bandwidth swarms |
| [ISSUE-011](ISSUE-011-pow-join-friction-scale-invariant.md) | Proof-of-Work join cost is identical at N=2 and N=1,000,000 | **Low** | Ch2 — Crypto Node IDs | Mobile viewers, small streams |

---

## Fix Dependency Map

Some fixes share work or should be implemented together:

```
ISSUE-001 (JOINING threshold)
    └── prerequisite for everything else — fix first

ISSUE-002 (dynamic M) + ISSUE-004 (live-edge sync)
    └── both require extending the DHT stream registration record
    └── implement together to avoid touching the record format twice

ISSUE-007 (multi-tree super nodes)
    └── depends on ISSUE-002 (dynamic M) being stable first
    └── changes tree assignment and DHT peer indexing

ISSUE-010 (collusion false positives)
    └── depends on DHT swarm_size field from ISSUE-004

ISSUE-011 (adaptive PoW)
    └── depends on DHT swarm_size field from ISSUE-004
```

---

## Priority Order for Implementation

1. **ISSUE-001** — Hard blocker. Fix before anything else.
2. **ISSUE-002 + ISSUE-004** — Implement together (shared DHT record changes).
3. **ISSUE-003** — Quick fix, high user-facing impact at launch events.
4. **ISSUE-010** — Prevents false evictions at small N; low effort.
5. **ISSUE-005** — Very low effort, prevents relay warm-up confusion.
6. **ISSUE-006** — Fixes churn recovery at small N; aligns with ISSUE-002 work.
7. **ISSUE-007** — Architectural change for super node support; do after core is stable.
8. **ISSUE-009** — Optimisation for large super node deployments.
9. **ISSUE-008** — Tuning change; validate in simulation (Ch8) before deploying.
10. **ISSUE-011** — Mobile UX improvement; low urgency.
