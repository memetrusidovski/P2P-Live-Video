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

    let passed = kpis.values().all(|l| l.pass);
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
    let (_, _, qdrops, ldrops) = sim.network.totals();
    facts.insert("queue_drops".into(), serde_json::json!(qdrops));
    facts.insert("loss_drops".into(), serde_json::json!(ldrops));
    facts.insert(
        "oracle_queries".into(),
        serde_json::json!(sim.oracle.queries),
    );
    facts.insert("events_logged".into(), serde_json::json!(sim.log.lines));
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
