//! `bssim explain`: render one node's timeline from `events.jsonl`.

use std::io::{BufRead, BufReader};
use std::path::Path;

/// Print the events of `node`, optionally narrowed to those mentioning `segment`
/// (plus lifecycle and topology events in the same window).
pub fn explain(out: &Path, node: usize, segment: Option<u32>, limit: usize) -> anyhow::Result<()> {
    let f = std::fs::File::open(out.join("events.jsonl"))?;
    let mut lines: Vec<serde_json::Value> = Vec::new();
    for l in BufReader::new(f).lines() {
        let l = l?;
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&l) else {
            continue;
        };
        if v.get("node").and_then(|n| n.as_u64()) != Some(node as u64) {
            continue;
        }
        if v.get("kind").and_then(|k| k.as_str()) == Some("depth")
            || v.get("kind").and_then(|k| k.as_str()) == Some("starved")
        {
            continue; // derived duplicates
        }
        lines.push(v);
    }
    let (lo, hi) = match segment {
        None => (0u64, u64::MAX),
        Some(s) => {
            let ts: Vec<u64> = lines
                .iter()
                .filter(|v| v.get("segment").and_then(|x| x.as_u64()) == Some(s as u64))
                .filter_map(|v| v.get("t_us").and_then(|x| x.as_u64()))
                .collect();
            match (ts.iter().min(), ts.iter().max()) {
                (Some(a), Some(b)) => (a.saturating_sub(4_000_000), *b + 500_000),
                _ => {
                    println!("no events for node {node} mention segment {s}");
                    (0, u64::MAX)
                }
            }
        }
    };
    let selected: Vec<&serde_json::Value> = lines
        .iter()
        .filter(|v| {
            let t = v.get("t_us").and_then(|x| x.as_u64()).unwrap_or(0);
            if t < lo || t > hi {
                return false;
            }
            match segment {
                None => true,
                Some(s) => {
                    let seg = v.get("segment").and_then(|x| x.as_u64());
                    seg == Some(s as u64) || seg.is_none()
                }
            }
        })
        .collect();
    println!(
        "node {node}: {} events{}",
        selected.len(),
        segment
            .map(|s| format!(" around segment {s}"))
            .unwrap_or_default()
    );
    let start = selected.len().saturating_sub(limit);
    for v in &selected[start..] {
        let t = v.get("t_us").and_then(|x| x.as_u64()).unwrap_or(0) as f64 / 1e6;
        let kind = v.get("kind").and_then(|x| x.as_str()).unwrap_or("?");
        let mut rest: Vec<String> = Vec::new();
        if let Some(m) = v.as_object() {
            let mut keys: Vec<&String> = m
                .keys()
                .filter(|k| !matches!(k.as_str(), "t_us" | "node" | "kind" | "id"))
                .collect();
            keys.sort();
            for k in keys {
                rest.push(format!("{k}={}", m[k]));
            }
        }
        println!("{t:>10.3}s  {kind:<18} {}", rest.join(" "));
    }
    Ok(())
}
