//! `bssim` — the BitStream discrete-event simulator.

#![forbid(unsafe_code)]

mod explain;
mod invariants;
mod kpi;
mod network;
mod oracle;
mod report;
mod scenario;
mod sim;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "bssim", about = "BitStream swarm simulator", version)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run a scenario and write verdict.json, kpis.json, topology.json and events.jsonl.
    Run {
        /// Scenario TOML.
        scenario: PathBuf,
        /// Output directory (default results/<name>).
        #[arg(long)]
        out: Option<PathBuf>,
        /// Override the seed.
        #[arg(long)]
        seed: Option<u64>,
        /// Override the duration (seconds).
        #[arg(long)]
        duration: Option<f64>,
        /// Do not halt on hard invariant violations.
        #[arg(long)]
        no_halt: bool,
        /// Skip events.jsonl (faster).
        #[arg(long)]
        no_events: bool,
        /// Quiet.
        #[arg(long, short)]
        quiet: bool,
    },
    /// Explain one node's timeline from a results directory.
    Explain {
        /// Results directory.
        #[arg(long)]
        out: PathBuf,
        /// Node index.
        #[arg(long)]
        node: usize,
        /// Segment to focus on.
        #[arg(long)]
        segment: Option<u32>,
        /// Max lines.
        #[arg(long, default_value_t = 200)]
        limit: usize,
    },
    /// Print the effective scenario (defaults filled in) as TOML.
    Show {
        /// Scenario TOML.
        scenario: PathBuf,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Run {
            scenario,
            out,
            seed,
            duration,
            no_halt,
            no_events,
            quiet,
        } => {
            let mut sc = scenario::Scenario::load(&scenario)?;
            if let Some(s) = seed {
                sc.seed = s;
            }
            if let Some(d) = duration {
                sc.duration_s = d;
            }
            let out = out.unwrap_or_else(|| PathBuf::from("results").join(&sc.name));
            std::fs::create_dir_all(&out)?;
            let events_path = out.join("events.jsonl");
            let log = report::EventLog::new(if no_events {
                None
            } else {
                Some(events_path.as_path())
            })?;
            let base = scenario
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| PathBuf::from("."));
            let t0 = std::time::Instant::now();
            let mut sim = sim::Sim::new(sc, &base, log)?;
            sim.invariants.halt_on_hard = !no_halt;
            let completed = sim.run();
            let wall = t0.elapsed();
            sim.log.flush();
            let v = report::verdict(&mut sim, &scenario, &out);
            report::write_reports(&sim, &v, &out)?;
            if !quiet {
                report::print_summary(&v);
                println!(
                    "  virtual {:.1}s in {:.2}s wall{}",
                    sim.now.as_secs_f64(),
                    wall.as_secs_f64(),
                    if completed {
                        ""
                    } else {
                        "  (HALTED by invariant)"
                    }
                );
            }
            if !v.passed {
                std::process::exit(1);
            }
        }
        Cmd::Explain {
            out,
            node,
            segment,
            limit,
        } => explain::explain(&out, node, segment, limit)?,
        Cmd::Show { scenario } => {
            let sc = scenario::Scenario::load(&scenario)?;
            println!("{}", toml::to_string_pretty(&sc)?);
        }
    }
    Ok(())
}
