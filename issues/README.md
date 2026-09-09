# Protocol Issues

All known design issues identified through protocol review. Each file is a self-contained issue with full context, impact analysis, a proposed fix, and — once applied — a resolution note listing the exact spec edits.

The canonical specification is [`../protocol/`](../protocol/).

---

## Open Issues

*None. All 18 tracked issues have been resolved in the specification.*

New issues should be filed as `ISSUE-019-<slug>.md` following the existing format.

---

## Resolved Issues

### Cold-start and small-swarm behaviour (ISSUE-001 – ISSUE-011)

| ID | Title | Priority | Resolution |
|---|---|---|---|
| [ISSUE-001](ISSUE-001-joining-state-threshold.md) | JOINING state machine blocks streams with fewer than 5 peers | **Critical** | Adaptive threshold `max(1, min(4, N-1))` in ch1.3 lifecycle docs |
| [ISSUE-002](ISSUE-002-hardcoded-forest-size-m6.md) | Hardcoded M=6 forces source to bear full CDN load during cold-start | **High** | Dynamic M ladder (1→6 over N=6..30) + signed `MANIFEST_UPDATE` with 5s migration window |
| [ISSUE-004](ISSUE-004-late-joiner-live-edge-sync-undefined.md) | Late-joiner live-edge sync is undefined in the protocol | **High** | Publisher Stream Record in DHT (`live_edge_segment_id` + `swarm_size`, republished 1/s) + join sequence in ch4.3 |
| [ISSUE-007](ISSUE-007-orthogonal-placement-limits-super-nodes.md) | Orthogonal Placement Rule restricts super nodes to 1 of 6 trees | **High** | Capacity-proportional multi-tree assignment `max_trees = min(M, floor(u_v/B))` (ch1.2 §1.3) |
| [ISSUE-003](ISSUE-003-tft-join-queue-simultaneous-viewers.md) | TFT optimistic unchoke creates a join queue for simultaneous viewers | **Medium** | Adaptive optimistic slots `min(ceil(new_joiners/3), 4)` (ch5.1) |
| [ISSUE-006](ISSUE-006-empty-passive-set-early-growth.md) | Empty passive set during early growth makes churn recovery slow | **Medium** | Cached DISCOVERY peer list as CHURN_REPAIR fallback when \|P\|<3 (ch3.3) |
| [ISSUE-009](ISSUE-009-active-set-gossip-does-not-scale-with-fan-out.md) | Active set gossip cap (c_a=8) does not scale with tree fan-out | **Medium** | Scaled `c_a_eff = min(64, K_v/10)` for relay nodes (ch3.1) |
| [ISSUE-010](ISSUE-010-collusion-detection-false-positives-small-n.md) | Collusion detection false positives flag legitimate peers at small N | **Medium** | `N_collusion=20` guard + degree-weighted confidence (ch5.3) |
| [ISSUE-005](ISSUE-005-relay-warmup-delay.md) | New relay nodes have no data to relay for ~1 second after joining | **Low** | Warm-up gating: advertise `K_avail=0` until first verified segment (ch1.2 §2.1) |
| [ISSUE-008](ISSUE-008-capacity-score-log-compression.md) | CapacityScore log compression undersells high-bandwidth nodes | **Low** | `sqrt(K_avail)` compression (ch1.2 §2.1; validate in ch8 sim) |
| [ISSUE-011](ISSUE-011-pow-join-friction-scale-invariant.md) | Proof-of-Work join cost is identical at N=2 and N=1,000,000 | **Low** | Adaptive C2 (8/10/12/14 by swarm size) + same-subnet reconnect discount (ch2.2) |

### Structural and cross-cutting design gaps (ISSUE-012 – ISSUE-018)

| ID | Title | Priority | Resolution |
|---|---|---|---|
| [ISSUE-015](ISSUE-015-swarm-sustainability-no-fallback.md) | Swarm sustainability condition has no fallback (Σu < N·B undefined) | **Critical** | New ch1.1 §5 Capacity Adaptation: σ math, SVC layer shedding, D_max overflow, source base-layer reserve |
| [ISSUE-012](ISSUE-012-wire-format-split-brain.md) | Wire format split-brain: gRPC schema contradicts binary frame diagrams | **High** | New Appendix D frame registry; proto demoted to non-normative; 12 missing frames specified; 80→152 byte fix |
| [ISSUE-013](ISSUE-013-block-symbol-relationship-undefined.md) | 16KB Merkle block vs 1024B RaptorQ symbol relationship undefined | **High** | One source block per Merkle block (K=16), block is the verification boundary, decode→verify→re-encode relaying |
| [ISSUE-014](ISSUE-014-two-churn-recovery-algorithms.md) | Two competing churn-recovery algorithms for the same 250ms budget | **High** | Passive-set promotion is canonical; sibling election becomes a coordination layer with roster, tie-break, and 45ms fallback |
| [ISSUE-016](ISSUE-016-node-class-policy-contradictions.md) | Mobile and privacy policies contradict PoW, placement rule, and TFT axiom | **High** | New ch1.2 §5 node classes (RELAY/LEAF/LEAF_PRIVATE): universal PoW, leaf status priced in QoS |
| [ISSUE-017](ISSUE-017-xdp-filter-mismatch.md) | XDP filter doesn't match its prose and would drop legitimate traffic | **Medium** | Two-tier default-deny allowlist + real token bucket, sized to 750 pps steady state |
| [ISSUE-018](ISSUE-018-doc-generations-unreconciled.md) | Three unreconciled documentation generations (README/thoughts/protocol) | **Medium** | `protocol/` declared canonical; README rewritten; thoughts/ banners; µs timestamps; τ_sched vs τ_gossip |

---

## What Changed in the Specification

The fixes above introduced three new specification documents and one new appendix:

- **[`protocol/appendix_d_frame_registry.md`](../protocol/appendix_d_frame_registry.md)** — the canonical frame-type registry: common header, transport mapping, and byte layouts for every frame.
- **[`protocol/chapter1/1.1_scale_latency/5_capacity_adaptation.md`](../protocol/chapter1/1.1_scale_latency/5_capacity_adaptation.md)** — degraded operation when the swarm cannot sustain full bitrate.
- **[`protocol/chapter1/1.2_multi_forest_overlays/5_node_classes.md`](../protocol/chapter1/1.2_multi_forest_overlays/5_node_classes.md)** — the RELAY/LEAF/LEAF_PRIVATE taxonomy.
- **[`protocol/schemas/p2p_live.proto`](../protocol/schemas/p2p_live.proto)** — rewritten as a non-normative field reference aligned with Appendix D.

---

## Remaining Future Work

Not defects, but design work the specification defers:

- **Simulation validation (Chapter 8).** Several fixes introduce tuning parameters that should be validated before deployment: the `sqrt` capacity exponent (ISSUE-008), the M ladder thresholds (ISSUE-002), `c_a_eff` scaling (ISSUE-009), and the shed/hysteresis constants (ISSUE-015).
- **Geographic locality.** Vivaldi synthetic coordinates and ASN-aware peer selection are described in `thoughts/main-issues.md` (Risk 9) but not yet specified.
- **Multi-source ingest.** Active-active encoders with signed broadcaster handover (Risk 7).
- **Relay-broadcast bitfield channel.** Documented as a future option in ch3.1; the scaled active set is the mandated baseline.
