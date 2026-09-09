# ISSUE-016: Mobile and Privacy Policies Contradict PoW, Placement Rule, and the TFT Axiom

**Status:** Resolved  
**Priority:** High  
**Component:** Ch1 §1.2 / Ch2 §2.2 / Ch7 §7.3 / thoughts (Risk 16)  
**Affects:** Mobile viewers, privacy-mode viewers, incentive integrity  
**File:** `protocol/chapter1/1.2_multi_forest_overlays/`, `protocol/chapter2/2.2_crypto_node_id/`, `protocol/chapter7/7.3_privacy_routing/`

---

## Summary

Four rules conflict: (1) `thoughts/main-issues.md` Risk 16 makes mobiles Leaf-Only and **PoW-exempt**; (2) ch2.2 requires PoW from everyone; (3) the Orthogonal Placement Rule assigns *every* node as interior relay in one tree with no leaf-only opt-out (while ch1.2 §4.4 lets a mobile voluntarily join only Tree 1); (4) ch7.3 locks onion-routed peers Leaf-Only and "exempted from tree relay obligations" — a sanctioned free-ride in a protocol whose foreword says contribution and playback performance are inextricably linked.

## Proposed Fix

New doc **`protocol/chapter1/1.2_multi_forest_overlays/5_node_classes.md`** defining a `NodeClass u8` carried in `PROBE_RESPONSE`, the child roster (ISSUE-014), and `NEIGHBOR` handshakes:

| Class | Relay duty | PoW | Entitlement |
|---|---|---|---|
| `RELAY` (0x00, default) | Orthogonal Placement applies | C1=16, adaptive C2 | Full quality by TFT rank |
| `LEAF` (0x01, self-declared: battery/metered) | Leaf in all M trees; never Deputy | C1=16 once; C2 tier −4; reconnect discount | Base layer (Tree 1) guaranteed via service floor; enhancement layers from surplus only; joins 5–10 s behind live edge |
| `LEAF_PRIVATE` (0x02, onion-routed per 7.3) | Leaf-only (inherent) | as LEAF | as LEAF + inherent onion latency; may earn TFT credit serving PULLs through its circuit |

Key points: **PoW is universal** (identity defense ≠ upload obligation; Risk 16's exemption is superseded — difficulty tiering addresses the mobile cost). **Leaf-only is not free-riding** because it is priced in QoS: base-layer-by-right, surplus-only enhancement, later live edge. The placement rule is scoped to RELAY class (relays per tree = N_relay/M, not N/M — feeds ISSUE-015's math); the hash assignment is computed but dormant for leaves and activates on class upgrade (e.g., plugged in + WiFi).

---

## Resolution

Applied the node-class taxonomy to the spec:

- **Created `protocol/chapter1/1.2_multi_forest_overlays/5_node_classes.md`** — RELAY/LEAF/LEAF_PRIVATE table with relay duty, PoW tier, and entitlement per class; the argument for why PoW is universal (identity defense ≠ upload obligation); placement rule scoped to RELAY class with the dormant-assignment rule and the N_relay/M density correction; class upgrade/downgrade semantics (self-demotion while holding children is churn); the "leaf-only is not free-riding" argument (quality ceiling + live-edge offset + no promotion, entitlement monotone in contribution with a base-layer floor).
- `protocol/chapter1/1.2_multi_forest_overlays/1_graph_theory_and_slicing.md` — Orthogonal Placement Rule explicitly scoped to RELAY class, with the capacity-density correction cross-referenced.
- `protocol/chapter1/1.2_multi_forest_overlays/4_stream_slicing_architecture.md` — the ad-hoc "mobile chooses Tree 1" example now references the LEAF class.
- `protocol/chapter2/2.2_crypto_node_id/1_static_dynamic_puzzles.md` — difficulty function takes node_class; leaf classes solve one tier lower, explicitly not exempt.
- `protocol/chapter7/7.3_privacy_routing/2_onion_latency_tradeoffs.md` — "exempted from tree relay obligations" replaced with LEAF_PRIVATE semantics: priced in QoS, PoW still required, may earn TFT credit by serving PULLs through its circuit.
- `protocol/chapter5/5.1_tit_for_tat/3_optimistic_exploration.md` — new "Universal Service Floor" section: base-layer floor for leaf peers capped at 20% of upload slots; enhancement layers never part of the floor.
- `protocol/appendix_b_parameters.md` — NodeClass codes, leaf live-edge offset 5–10 s, service floor cap 20%.
- `protocol/INDEX.md`, `protocol/chapter1/1.2_multi_forest_overlays/README.md` — new doc listed.
