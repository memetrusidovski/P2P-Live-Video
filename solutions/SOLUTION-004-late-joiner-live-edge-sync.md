# SOLUTION-004: Late-Joiner Live-Edge Synchronization

**Closes:** ISSUE-004 (High)
**Lives in:** `protocol/chapter2/2.3_stream_registration/1_publisher_genesis_key.md`, `2_store_get_rpcs.md`, `protocol/chapter4/4.3_hybrid_push_pull/1_buffer_sliding_timeline.md`, `protocol/chapter1/1.3_peer_lifecycle/1_transition_model.md`, `schemas/p2p_live.proto`
**Class:** Media-plane bootstrap

---

## The problem in one line

Both buffer zones are defined relative to a segment sequence number, and a joining peer had no way to learn what that number currently is — so it would start at segment 0, i.e. two hours behind a two-hour stream.

## The decision

The publisher maintains one authoritative **Stream Record** at $K_s = \text{Blake3}(\text{StreamID})$, signed with $SK_{\text{Publisher}}$ and republished every 1.0 s:

```json
{ "live_edge_segment_id": 3847, "live_edge_manifest_hash": "...",
  "live_edge_timestamp": 1750123456000000, "swarm_size": 1834,
  "num_trees": 6, "manifest_version": 4, "tree_mapping": [ ... ] }
```

It rides along in the `GET_PEERS` response the joiner already makes during DISCOVERY, so live-edge sync costs **zero extra round-trips**.

## Why one record rather than a new frame

The alternative — a `GET_LIVE_EDGE` RPC, or asking the first connected parent — was rejected for a reason that generalises: **a parent is not a trustworthy source of the live edge.** A malicious or merely lagging parent that reports a low edge pins the victim behind the swarm indefinitely, and the victim has no second opinion to check it against. Anchoring on a publisher-signed record makes the anchor forgeable only with $SK_{\text{Publisher}}$.

The same record turned out to be the natural home for three other values that different fixes each needed independently — `swarm_size` (SOLUTION-001's $\theta_{\text{join}}$, SOLUTION-002's $M$ ladder, and the adaptive PoW tier), and `num_trees`/`tree_mapping` (SOLUTION-002's resizing). One signed, republished record now carries every swarm-global scalar. That consolidation is worth preserving: **anything the whole swarm must agree on belongs in the Stream Record, not in gossip**, because gossip has no authority to settle disagreements.

## The defect found while verifying: staleness was permanent

The applied fix anchored correctly but treated the record as ground truth. Its error is one-directional — always *behind* the true edge — and it stacks:

| Source of lag | Magnitude |
| :--- | :--- |
| Record age at the guardian (1 s cadence) | 0–1.0 s |
| `GET_PEERS` over $O(\log N)$ hops at $N=10^6$ | ~0.3–0.5 s |
| Parent selection + QUIC handshake | ~0.2 s |

A peer anchoring at $X$ sets playout 4.0 s behind an edge already ~1.7 s stale — and then **never re-measures**, because it simply follows PUSH from $X{+}1$. The offset is inherited for the life of the session, and it differs per viewer according to how slow their DHT lookup happened to be. Two people watching the same match in the same room end up seconds apart, permanently. For the interactive use cases this protocol names, that is a product failure, not a rounding error.

### Resolution: the record is a hint; the push path is the authority

$$X_{\text{edge}} = \max\left(\texttt{live\_edge\_segment\_id},\ \max_{p \in \text{parents}} \text{HighestPushedSeq}(p)\right)$$

* **Re-anchor on entry to ACTIVE** against $X_{\text{edge}}$, discarding the record's estimate. Viewers converge on one offset regardless of lookup latency.
* **Correct drift continuously.** More than one segment beyond the 4.0 s target ⇒ skip forward. Those segments are past their deadline and the scheduler would drop them anyway; waiting only makes the offset permanent.
* **Never re-anchor backward.** $X_{\text{edge}}$ is monotonically non-decreasing — a parent reporting a lower sequence number is stale or hostile. This is the same monotonicity the manifest validator already enforces (Ch7 §7.1.2), applied to the anchor.

The general principle: **a bootstrap value obtained once, from a lagging source, must be replaced by a continuously measured one as soon as measurement becomes possible.** Anywhere the spec anchors on a DHT read, the same question applies.

## Second defect: the retention window did not cover leaf anchors

Late joiners are served from peers' retained segments, specified at 8–10 s. But leaf-class peers deliberately anchor 5–10 s *behind* the edge (Ch1 §1.2.5) — so a leaf joining at the maximum offset needs segments sitting exactly at the retention boundary, with no margin for its own PULL round-trips. Two independently-reasonable numbers, set in different chapters, that do not compose.

Retention is now **derived** rather than fixed: $\Delta_{\text{leaf}}^{\max} + 4\text{ s}$ (14 s, ~10.5 MB at 6 Mbps). Same failure mode as SOLUTION-003's slot budget — constants set locally that must satisfy a global relation.

## Validation owed (Chapter 8)

* Playout-offset spread across viewers joining at different times — the metric the re-anchoring rule exists to control. Should be near zero; without re-anchoring it would be the spread of DHT lookup latency.
* Whether skip-forward drift correction is perceptible, or whether playback-rate adjustment is the better correction for small offsets.
* Memory cost of 14 s retention on constrained devices, and whether leaf-class peers should be exempt from *serving* late joiners.
