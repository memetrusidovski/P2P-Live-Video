//! Event log (JSON lines), verdict and KPI reports.

use std::collections::{HashMap, VecDeque};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use bs_core::{Event, Instant};
use bs_wire::NodeId;
use serde::{Deserialize, Serialize};

use crate::kpi::{percentile, Kpis};
use crate::scenario::Scenario;
use crate::sim::Sim;

/// Writes `events.jsonl` and keeps a short per-node tail for failure reports.
pub struct EventLog {
    w: Option<BufWriter<File>>,
    tails: HashMap<usize, VecDeque<String>>,
    /// Lines written.
    pub lines: u64,
}

impl EventLog {
    /// New log writing to `path` (or nowhere).
    pub fn new(path: Option<&Path>) -> anyhow::Result<Self> {
        let w = match path {
            Some(p) => Some(BufWriter::with_capacity(1 << 20, File::create(p)?)),
            None => None,
        };
        Ok(Self {
            w,
            tails: HashMap::new(),
            lines: 0,
        })
    }
    fn push(&mut self, node: usize, line: String) {
        let t = self.tails.entry(node).or_default();
        if t.len() >= 40 {
            t.pop_front();
        }
        t.push_back(line.clone());
        if let Some(w) = self.w.as_mut() {
            let _ = writeln!(w, "{line}");
        }
        self.lines += 1;
    }
    /// A node event.
    pub fn event(&mut self, now: Instant, node: usize, id: &NodeId, e: &Event) {
        let mut v = serde_json::to_value(e).unwrap_or(serde_json::Value::Null);
        if let serde_json::Value::Object(m) = &mut v {
            m.insert("t_us".into(), now.as_micros().into());
            m.insert("node".into(), node.into());
            m.insert("id".into(), id.short().into());
        }
        self.push(node, v.to_string());
        // Derived lines for the Python analyzer.
        match e {
            Event::ChunkPlayed {
                ok: false, segment, ..
            } => {
                self.push(
                    node,
                    format!(
                        r#"{{"kind":"starved","t_us":{},"node":{},"segment":{}}}"#,
                        now.as_micros(),
                        node,
                        segment
                    ),
                );
            }
            Event::ParentAttached { tree, depth, .. } => {
                self.push(
                    node,
                    format!(
                        r#"{{"kind":"depth","t_us":{},"node":{},"tree":{},"depth":{}}}"#,
                        now.as_micros(),
                        node,
                        tree,
                        depth
                    ),
                );
            }
            _ => {}
        }
    }
    /// A free-form note.
    pub fn note(&mut self, now: Instant, node: usize, text: &str) {
        let line =
            serde_json::json!({"kind":"note","t_us":now.as_micros(),"node":node,"text":text})
                .to_string();
        self.push(node, line);
    }
    /// Last events of a node.
    pub fn tail(&self, node: usize) -> Vec<String> {
        self.tails
            .get(&node)
            .map(|t| t.iter().cloned().collect())
            .unwrap_or_default()
    }
    /// Flush.
    pub fn flush(&mut self) {
        if let Some(w) = self.w.as_mut() {
            let _ = w.flush();
        }
    }
}

/// One KPI line in the verdict.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KpiLine {
    /// Measured.
    pub value: f64,
    /// Threshold.
    pub threshold: f64,
    /// "<=" or ">=".
    pub op: String,
    /// Pass.
    pub pass: bool,
}

/// The verdict (schema shared with `py/bitstream_tools/report.py` and `fleet/report.py`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Verdict {
    /// Scenario name.
    pub scenario: String,
    /// Seed.
    pub seed: u64,
    /// Overall.
    pub passed: bool,
    /// KPIs.
    pub kpis: HashMap<String, KpiLine>,
    /// Hard and soft violations.
    pub invariant_violations: Vec<crate::invariants::Violation>,
    /// Repro command.
    pub repro: String,
    /// Extra facts.
    pub facts: HashMap<String, serde_json::Value>,
    /// Failure explanation for agents.
    pub failure_hints: Vec<String>,
}

fn kpi(value: f64, threshold: f64, le: bool) -> KpiLine {
    let pass = if le {
        value <= threshold
    } else {
        value >= threshold
    };
    KpiLine {
        value,
        threshold,
        op: if le { "<=" } else { ">=" }.into(),
        pass,
    }
}

/// Build the verdict from a finished simulation.
pub fn verdict(sim: &mut Sim, scenario_path: &Path, out: &Path) -> Verdict {
    let nodes = std::mem::take(&mut sim.nodes);
    sim.kpis.finalize_nodes(&nodes);
    let s: &Scenario = &sim.scenario;
    let k: &Kpis = &sim.kpis;
    let th = &s.kpis;
    let mut kpis = HashMap::new();
    let mut facts = HashMap::new();
    let (psr, starved, played) = k.psr();
    kpis.insert("psr".into(), kpi(psr, th.psr_max, true));
    let sjl = k.sjl();
    let sjl_mean = if sjl.is_empty() {
        f64::INFINITY
    } else {
        sjl.iter().sum::<f64>() / sjl.len() as f64
    };
    kpis.insert("sjl_mean_s".into(), kpi(sjl_mean, th.sjl_mean_max_s, true));
    kpis.insert(
        "sjl_p95_s".into(),
        kpi(
            if sjl.is_empty() {
                f64::INFINITY
            } else {
                percentile(&sjl, 0.95)
            },
            th.sjl_p95_max_s,
            true,
        ),
    );
    let last = k.depth_series.last().copied().unwrap_or((0, 0.0, 0, 0, 0));
    kpis.insert("depth_mean".into(), kpi(last.1, th.depth_mean_max, true));
    kpis.insert(
        "depth_max".into(),
        kpi(last.2 as f64, th.depth_max as f64, true),
    );
    let (cdo, control, media) = k.cdo();
    kpis.insert("cdo".into(), kpi(cdo, th.cdo_max, true));
    let mut rep: Vec<f64> = k.repairs_us.iter().map(|u| *u as f64 / 1e6).collect();
    rep.sort_by(|a, b| a.partial_cmp(b).unwrap());
    kpis.insert(
        "repair_p95_s".into(),
        kpi(
            if rep.is_empty() {
                0.0
            } else {
                percentile(&rep, 0.95)
            },
            th.repair_p95_max_s,
            true,
        ),
    );
    kpis.insert(
        "repair_median_s".into(),
        kpi(
            if rep.is_empty() {
                0.0
            } else {
                percentile(&rep, 0.5)
            },
            th.repair_median_max_s,
            true,
        ),
    );
    facts.insert("repairs".into(), serde_json::json!(rep.len()));
    let mismatches: u64 = k.nodes.iter().map(|n| n.hash_mismatches).sum();
    let checked: u64 = k.nodes.iter().map(|n| n.hash_checked).sum();
    kpis.insert(
        "hash_mismatches".into(),
        kpi(
            mismatches as f64,
            if th.require_hash_match {
                0.0
            } else {
                f64::INFINITY
            },
            true,
        ),
    );
    kpis.insert(
        "active_fraction".into(),
        kpi(k.active_fraction(&nodes), th.active_fraction_min, false),
    );
    let hard = sim.invariants.violations.iter().filter(|v| v.hard).count();
    kpis.insert(
        "hard_invariant_violations".into(),
        kpi(hard as f64, 0.0, true),
    );

    facts.insert("nodes".into(), serde_json::json!(nodes.len()));
    facts.insert(
        "alive_at_end".into(),
        serde_json::json!(nodes.iter().filter(|n| n.alive).count()),
    );
    facts.insert(
        "chunks_published".into(),
        serde_json::json!(k.chunks_published),
    );
    facts.insert("chunks_played".into(), serde_json::json!(played));
    facts.insert("chunks_starved".into(), serde_json::json!(starved));
    facts.insert(
        "layer_chunks_hash_checked".into(),
        serde_json::json!(checked),
    );
    facts.insert("bytes_control".into(), serde_json::json!(control));
    facts.insert("bytes_media".into(), serde_json::json!(media));
    facts.insert(
        "sessions_open".into(),
        serde_json::json!(sim.network.session_count()),
    );
    facts.insert(
        "no_session_drops".into(),
        serde_json::json!(k.no_session_drops),
    );
    facts.insert(
        "no_session_by_type".into(),
        serde_json::json!(k.no_session_by_type),
    );
    facts.insert("freeload_drops".into(), serde_json::json!(k.freeload_drops));
    facts.insert(
        "receipts_rejected_by_reason".into(),
        serde_json::json!(k.receipt_reject_reasons),
    );
    {
        let sum = |f: fn(&crate::kpi::NodeStats) -> u64| k.nodes.iter().map(f).sum::<u64>();
        facts.insert(
            "receipts_issued".into(),
            serde_json::json!(sum(|n| n.receipts_issued)),
        );
        facts.insert(
            "receipts_accepted".into(),
            serde_json::json!(sum(|n| n.receipts_accepted)),
        );
        facts.insert(
            "receipts_rejected".into(),
            serde_json::json!(sum(|n| n.receipts_rejected)),
        );
        facts.insert(
            "receipts_missed".into(),
            serde_json::json!(sum(|n| n.receipts_missed)),
        );
        facts.insert("chokes".into(), serde_json::json!(sum(|n| n.chokes)));
        facts.insert("unchokes".into(), serde_json::json!(sum(|n| n.unchokes)));
        facts.insert(
            "optimistic_unchokes".into(),
            serde_json::json!(sum(|n| n.optimistic)),
        );
        facts.insert(
            "pulls_choked".into(),
            serde_json::json!(sum(|n| n.pulls_choked)),
        );
        facts.insert(
            "rank_proofs_sent".into(),
            serde_json::json!(sum(|n| n.rank_proofs_sent)),
        );
        facts.insert(
            "rank_proofs_verified".into(),
            serde_json::json!(sum(|n| n.rank_proofs_verified)),
        );
        facts.insert(
            "rank_proofs_rejected_by_reason".into(),
            serde_json::json!(k.rank_reject_reasons),
        );
        facts.insert(
            "preemptions".into(),
            serde_json::json!(sum(|n| n.preemptions)),
        );
        facts.insert(
            "accusations_made".into(),
            serde_json::json!(sum(|n| n.accusations_made)),
        );
        facts.insert(
            "accusations_counted".into(),
            serde_json::json!(sum(|n| n.accusations_counted)),
        );
        facts.insert(
            "accusations_dropped_by_reason".into(),
            serde_json::json!(k.accusation_drop_reasons),
        );
        facts.insert(
            "forest_updates".into(),
            serde_json::json!(sum(|n| n.forest_updates)),
        );
        facts.insert(
            "forest_switches".into(),
            serde_json::json!(sum(|n| n.forest_switches)),
        );
        facts.insert(
            "relay_reassignments".into(),
            serde_json::json!(sum(|n| n.relay_reassignments)),
        );
        facts.insert(
            "punches_sent".into(),
            serde_json::json!(sum(|n| n.punches_sent)),
        );
        facts.insert(
            "bridges_proposed".into(),
            serde_json::json!(sum(|n| n.bridges_proposed)),
        );
        facts.insert(
            "bridges_bound".into(),
            serde_json::json!(sum(|n| n.bridges_bound)),
        );
        facts.insert(
            "bridges_rejected".into(),
            serde_json::json!(sum(|n| n.bridges_rejected)),
        );
        facts.insert(
            "bridges_served".into(),
            serde_json::json!(sum(|n| n.bridges_served)),
        );
        facts.insert(
            "bridges_expired".into(),
            serde_json::json!(sum(|n| n.bridges_expired)),
        );
        facts.insert(
            "ingress_bound".into(),
            serde_json::json!(sum(|n| n.ingress_bound)),
        );
        facts.insert("nat_drops".into(), serde_json::json!(sim.network.nat_drops));
        if let Some(m) = nodes.first().and_then(|n| n.node.matrix()) {
            facts.insert("forest_m".into(), serde_json::json!(m.m()));
            facts.insert("forest_layers".into(), serde_json::json!(m.layer_count));
            facts.insert("forest_version".into(), serde_json::json!(m.version));
        }
        facts.insert(
            "subnet_penalties".into(),
            serde_json::json!(sum(|n| n.subnet_penalties)),
        );
        // Consensus evictions by the label of the evicted peer.
        let id_label: std::collections::HashMap<bs_wire::NodeId, String> = nodes
            .iter()
            .map(|n| (n.node.node_id(), n.label.clone()))
            .collect();
        let mut ev_by_label: std::collections::BTreeMap<String, u64> = nodes
            .iter()
            .filter(|n| !n.node.is_source())
            .map(|n| (n.label.clone(), 0))
            .collect();
        for id in &k.consensus_evictions {
            let l = id_label
                .get(id)
                .cloned()
                .unwrap_or_else(|| "unknown".into());
            *ev_by_label.entry(l).or_default() += 1;
        }
        facts.insert(
            "consensus_evictions".into(),
            serde_json::json!(k.consensus_evictions.len()),
        );
        for (l, c) in ev_by_label {
            facts.insert(format!("consensus_evictions:{l}"), serde_json::json!(c));
        }
        // Per label: the fraction of viewers that end the run with a parent in every
        // tree of their configured quality. A shed layer counts as missing.
        let mut full: std::collections::BTreeMap<String, (u64, u64)> = Default::default();
        for n in nodes.iter().filter(|n| !n.node.is_source() && n.alive) {
            let e = full.entry(n.label.clone()).or_default();
            let top = n.node.configured_top_layer();
            let ok = n
                .node
                .trees()
                .iter()
                .filter(|t| {
                    n.node
                        .matrix()
                        .and_then(|m| m.tree(t.id))
                        .map(|i| i.layer <= top)
                        .unwrap_or(false)
                })
                .all(|t| t.parent.is_some());
            e.0 += ok as u64;
            e.1 += 1;
        }
        for (l, (a, t)) in full {
            facts.insert(
                format!("attached_all:{l}"),
                serde_json::json!(if t == 0 { 0.0 } else { a as f64 / t as f64 }),
            );
        }
        let (ps, pn) = (sum(|n| n.parity_sum), sum(|n| n.parity_n));
        facts.insert(
            "parity_mean".into(),
            serde_json::json!(if pn > 0 {
                ps as f64 / pn as f64
            } else {
                s.params.default_parity as f64
            }),
        );
        let evictions: u64 = nodes.iter().map(|n| n.node.deadline_evictions()).sum();
        facts.insert("deadline_evictions".into(), serde_json::json!(evictions));
        // Per-label steady-state starvation, so an adversarial group can be excluded
        // from the honest gate (`facts_max = { "psr:honest" = 0.001 }`).
        let mut labels: std::collections::BTreeMap<String, (u64, u64)> = Default::default();
        for n in k.nodes.iter().filter(|n| !n.is_publisher) {
            let e = labels.entry(n.label.clone()).or_default();
            e.0 += n.steady_starved;
            e.1 += n.steady_played;
        }
        for (l, (st, pl)) in labels {
            facts.insert(
                format!("psr:{l}"),
                serde_json::json!(if pl == 0 { 1.0 } else { st as f64 / pl as f64 }),
            );
        }
        let mut active: std::collections::BTreeMap<String, (u64, u64)> = Default::default();
        for n in k.nodes.iter().filter(|n| !n.is_publisher) {
            let e = active.entry(n.label.clone()).or_default();
            e.0 += n.active_us.is_some() as u64;
            e.1 += 1;
        }
        for (l, (a, t)) in active {
            facts.insert(
                format!("active_fraction:{l}"),
                serde_json::json!(if t == 0 { 0.0 } else { a as f64 / t as f64 }),
            );
        }
    }
    let (_, _, qdrops, ldrops) = sim.network.totals();
    facts.insert("queue_drops".into(), serde_json::json!(qdrops));
    {
        let top: Vec<serde_json::Value> = sim
            .network
            .top_queue_drops(6)
            .into_iter()
            .map(|(addr, drops, up)| {
                let n = nodes.iter().position(|x| x.addr == addr);
                let detail = n.map(|i| {
                    let x = &nodes[i];
                    serde_json::json!({
                        "node": i, "label": x.label, "upload_kbps": up, "drops": drops, "class": format!("{:?}", x.node.node_class()),
                        "trees": x.node.trees().iter().map(|t| serde_json::json!({"tree": t.id.0, "children": t.children.len(), "k_v": t.k_v, "assigned": t.assigned})).collect::<Vec<_>>(),
                        "active_set": x.node.active_set_len(),
                    })
                });
                detail.unwrap_or(serde_json::json!({"addr": addr.to_string(), "drops": drops}))
            })
            .collect();
        facts.insert("top_queue_drops".into(), serde_json::json!(top));
    }
    facts.insert("loss_drops".into(), serde_json::json!(ldrops));
    facts.insert(
        "oracle_queries".into(),
        serde_json::json!(sim.oracle.queries),
    );
    facts.insert("events_logged".into(), serde_json::json!(sim.log.lines));
    facts.insert(
        "backfill_played".into(),
        serde_json::json!(k.nodes.iter().map(|n| n.backfill_played).sum::<u64>()),
    );
    facts.insert(
        "backfill_ok".into(),
        serde_json::json!(k.nodes.iter().map(|n| n.backfill_ok).sum::<u64>()),
    );
    facts.insert(
        "pulls_served".into(),
        serde_json::json!(k.nodes.iter().map(|n| n.pulls_served).sum::<u64>()),
    );
    facts.insert(
        "pulls_refused".into(),
        serde_json::json!(k.nodes.iter().map(|n| n.pulls_refused).sum::<u64>()),
    );
    facts.insert(
        "pulls_unheld".into(),
        serde_json::json!(k.nodes.iter().map(|n| n.pulls_unheld).sum::<u64>()),
    );
    {
        let lookups: u64 = k.nodes.iter().map(|n| n.dht_lookups).sum();
        let hops: u64 = k.nodes.iter().map(|n| n.dht_hops).sum();
        let ms: u64 = k.nodes.iter().map(|n| n.dht_lookup_ms).sum();
        let queried: u64 = k.nodes.iter().map(|n| n.dht_queried).sum();
        let mean = |x: u64| {
            if lookups > 0 {
                x as f64 / lookups as f64
            } else {
                0.0
            }
        };
        facts.insert("dht_lookups".into(), serde_json::json!(lookups));
        facts.insert("dht_hops_mean".into(), serde_json::json!(mean(hops)));
        facts.insert("dht_lookup_ms_mean".into(), serde_json::json!(mean(ms)));
        facts.insert("dht_queried_mean".into(), serde_json::json!(mean(queried)));
        facts.insert(
            "dht_rpc_served".into(),
            serde_json::json!(k.nodes.iter().map(|n| n.dht_served).sum::<u64>()),
        );
        facts.insert(
            "dht_rpc_served_max".into(),
            serde_json::json!(k.nodes.iter().map(|n| n.dht_served).max().unwrap_or(0)),
        );
        facts.insert(
            "dht_get_peers_served".into(),
            serde_json::json!(k.nodes.iter().map(|n| n.dht_get_peers_served).sum::<u64>()),
        );
    }
    facts.insert(
        "pulls_sent".into(),
        serde_json::json!(k.nodes.iter().map(|n| n.pulls_sent).sum::<u64>()),
    );
    facts.insert(
        "blocks_pulled".into(),
        serde_json::json!(k.nodes.iter().map(|n| n.blocks_pulled).sum::<u64>()),
    );
    facts.insert(
        "soft_violations".into(),
        serde_json::json!(sim.invariants.soft_counts()),
    );
    let mut by_type: Vec<(String, u64, u64)> = k
        .bytes
        .iter()
        .map(|(t, (c, b))| (t.name().to_string(), *c, *b))
        .collect();
    by_type.sort_by_key(|x| std::cmp::Reverse(x.2));
    facts.insert("frames_by_type".into(), serde_json::json!(by_type));

    // Scenario-declared bounds on facts.
    for (name, thr) in &th.facts_max {
        let v = facts
            .get(name)
            .and_then(|v| v.as_f64())
            .unwrap_or(f64::INFINITY);
        kpis.insert(format!("fact:{name}"), kpi(v, *thr, true));
    }
    for (name, thr) in &th.facts_min {
        let v = facts
            .get(name)
            .and_then(|v| v.as_f64())
            .unwrap_or(f64::NEG_INFINITY);
        kpis.insert(format!("fact:{name}"), kpi(v, *thr, false));
    }
    let passed = kpis.values().all(|l| l.pass);
    let mut hints = Vec::new();
    if !passed {
        for (name, l) in &kpis {
            if !l.pass {
                hints.push(format!(
                    "{name}: {:.4} {} {:.4} failed",
                    l.value,
                    if l.op == "<=" { "exceeds" } else { "below" },
                    l.threshold
                ));
            }
        }
        // Worst viewers by starvation.
        let mut worst: Vec<(usize, &crate::kpi::NodeStats)> = k
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| !n.is_publisher && n.steady_played > 0)
            .collect();
        worst.sort_by(|a, b| {
            (b.1.steady_starved * 1000 / b.1.steady_played.max(1))
                .cmp(&(a.1.steady_starved * 1000 / a.1.steady_played.max(1)))
        });
        for (i, n) in worst.iter().take(3) {
            if n.steady_starved > 0 {
                hints.push(format!("node {i} ({}) starved {}/{} steady-state chunks; depth {:?}; parent_lost {}; run: bssim explain --out {} --node {i}", n.label, n.steady_starved, n.steady_played, n.depth, n.parent_lost, out.display()));
            }
        }
        let never: Vec<usize> = k
            .nodes
            .iter()
            .enumerate()
            .filter(|(i, n)| {
                *i > 0
                    && n.started_us.is_some()
                    && n.first_chunk_us.is_none()
                    && n.died_us.is_none()
            })
            .map(|(i, _)| i)
            .collect();
        if !never.is_empty() {
            hints.push(format!(
                "{} viewer(s) never played a chunk: {:?}",
                never.len(),
                &never[..never.len().min(10)]
            ));
        }
        for v in sim.invariants.violations.iter().filter(|v| v.hard).take(3) {
            hints.push(format!(
                "hard invariant {} at node {} t={:.3}s: {}",
                v.rule,
                v.node,
                v.t_us as f64 / 1e6,
                v.detail
            ));
            for line in sim.log.tail(v.node) {
                hints.push(format!("  {line}"));
            }
        }
    }
    let repro = format!(
        "cargo run -q -p bs-sim --profile sim -- run {} --seed {} --out {}",
        scenario_path.display(),
        s.seed,
        out.display()
    );
    sim.nodes = nodes;
    Verdict {
        scenario: s.name.clone(),
        seed: s.seed,
        passed,
        kpis,
        invariant_violations: sim.invariants.violations.clone(),
        repro,
        facts,
        failure_hints: hints,
    }
}

/// Write verdict.json, kpis.json and topology.json.
pub fn write_reports(sim: &Sim, v: &Verdict, out: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(out)?;
    std::fs::write(out.join("verdict.json"), serde_json::to_string_pretty(v)?)?;
    let kp = serde_json::json!({
        "nodes": sim.kpis.nodes,
        "depth_series": sim.kpis.depth_series.iter().map(|(t, m, mx, a, v)| serde_json::json!({"t_us": t, "depth_mean": m, "depth_max": mx, "active": a, "viewers": v})).collect::<Vec<_>>(),
        "repairs_us": sim.kpis.repairs_us,
    });
    std::fs::write(out.join("kpis.json"), serde_json::to_string_pretty(&kp)?)?;
    let topo: Vec<serde_json::Value> = sim
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| {
            serde_json::json!({
                "node": i, "label": n.label, "id": n.node.node_id().short(), "addr": n.addr.to_string(), "alive": n.alive,
                "state": format!("{:?}", n.node.state()), "class": format!("{:?}", n.node.node_class()), "top_layer": n.node.top_layer(),
                "trees": n.node.trees().iter().map(|t| serde_json::json!({
                    "tree": t.id.0, "assigned": t.assigned, "subscribed": t.subscribed, "depth": t.depth, "k_v": t.k_v,
                    "parent": t.parent.as_ref().map(|p| p.node_id.short()), "children": t.children.len(),
                    "verified_segments": t.verified_segments, "live_edge": t.live_edge.0,
                })).collect::<Vec<_>>(),
                "dht_contacts": n.node.dht_contacts(), "dht_registrations": n.node.dht_registrations(), "dht_rpc_served": n.node.dht_rpc_served(),
            })
        })
        .collect();
    std::fs::write(
        out.join("topology.json"),
        serde_json::to_string_pretty(&topo)?,
    )?;
    Ok(())
}

/// Human summary.
pub fn print_summary(v: &Verdict) {
    println!(
        "scenario {}  seed {}  => {}",
        v.scenario,
        v.seed,
        if v.passed { "PASS" } else { "FAIL" }
    );
    let mut names: Vec<&String> = v.kpis.keys().collect();
    names.sort();
    for n in names {
        let l = &v.kpis[n];
        println!(
            "  {:<26} {:>12.4} {} {:<10} {}",
            n,
            l.value,
            l.op,
            format!("{:.4}", l.threshold),
            if l.pass { "ok" } else { "FAIL" }
        );
    }
    let mut facts: Vec<(&String, &serde_json::Value)> = v
        .facts
        .iter()
        .filter(|(k, _)| *k != "frames_by_type")
        .collect();
    facts.sort_by(|a, b| a.0.cmp(b.0));
    for (k, val) in facts {
        println!("  {k}: {val}");
    }
    for h in &v.failure_hints {
        println!("  hint: {h}");
    }
    println!("  repro: {}", v.repro);
}
