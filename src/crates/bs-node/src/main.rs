//! `bsnode` — publish or watch a BitStream over a real network.

#![forbid(unsafe_code)]

use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Context;
use bs_core::{Params, Role};
use bs_media::Ladder;
use bs_node::{Media, RunConfig};
use bs_wire::{NodeClass, StreamId};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "bsnode", about = "BitStream peer", version)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
    /// Use the specification's proof-of-work difficulty (default: cheap simulation values).
    #[arg(long, global = true)]
    spec_pow: bool,
    /// RaptorQ parity symbols per 16 KB block on every link (1..=5). Adaptive per-link
    /// parity arrives with PROOF_OF_UPLOAD loss reports (M3); until then 1 suits clean
    /// links and 4-5 lossy ones.
    #[arg(long, global = true, default_value_t = 1)]
    parity: u16,
}

#[derive(Subcommand)]
enum Cmd {
    /// Publish a file (or synthetic bytes) and seed the DHT for viewers.
    Publish {
        /// File to stream as opaque bytes. Omit for synthetic data.
        #[arg(long)]
        file: Option<PathBuf>,
        /// Only the first N bytes of the file.
        #[arg(long)]
        max_bytes: Option<usize>,
        /// Loop the file when exhausted.
        #[arg(long)]
        repeat: bool,
        /// UDP bind address (QUIC and plain frames share it).
        #[arg(long, default_value = "0.0.0.0:4000")]
        listen: SocketAddr,
        /// External IP to advertise (default: detected or the listen IP).
        #[arg(long)]
        advertise_ip: Option<std::net::IpAddr>,
        /// Ladder: "reference" (1.5+1.5+3.0 Mbps) or "single:<kbps>".
        #[arg(long, default_value = "reference")]
        ladder: String,
        /// Forest size M.
        #[arg(long, default_value_t = 3)]
        forest: u8,
        /// Upload capacity, kbps.
        #[arg(long, default_value_t = 100_000)]
        upload_kbps: u32,
        /// Seed (identity + RNG).
        #[arg(long, default_value_t = 1)]
        seed: u64,
        /// Seconds to wait before the first chunk so early viewers attach.
        #[arg(long, default_value_t = 3.0)]
        start_delay_s: f64,
        /// Stop after this many seconds (default: at end of file).
        #[arg(long)]
        duration_s: Option<f64>,
        /// Write result.json here.
        #[arg(long)]
        result: Option<PathBuf>,
        /// Serve the status page and API on this address (e.g. 0.0.0.0:8080).
        #[arg(long)]
        player: Option<SocketAddr>,
        /// Ingest mode: "file" (opaque bytes, default) or "ffmpeg" (live simulcast renditions).
        #[arg(long, default_value = "file")]
        ingest: String,
        /// ffmpeg input (file or URL) when `--ingest ffmpeg`.
        #[arg(long)]
        input: Option<PathBuf>,
        /// Renditions for `--ingest ffmpeg`, lowest first: e.g. 480p:1500,720p:3000,1080p:6000.
        #[arg(long, default_value = "480p:1500,720p:3000,1080p:6000")]
        renditions: String,
        /// Frame rate of the input (default: probe with ffprobe).
        #[arg(long)]
        fps: Option<u32>,
        /// NAT class: public, cone or symmetric. A publisher behind a NAT sends through
        /// ingress relays, so viewers need another bootstrap contact.
        #[arg(long, default_value = "public")]
        nat: String,
    },
    /// Join a stream through a DHT bootstrap seed and write the reassembled bytes.
    Watch {
        /// DHT bootstrap seed address, normally the publisher (host:port).
        #[arg(long)]
        bootstrap: String,
        /// UDP bind address.
        #[arg(long, default_value = "0.0.0.0:0")]
        listen: SocketAddr,
        /// External IP to advertise (default: detected).
        #[arg(long)]
        advertise_ip: Option<std::net::IpAddr>,
        /// Stream id (hex). Default: the publisher's, derived from `--publisher-seed`.
        #[arg(long)]
        stream: Option<String>,
        /// Seed the publisher was started with (to derive the stream id).
        #[arg(long, default_value_t = 1)]
        publisher_seed: u64,
        /// Publisher listen port (part of its identity derivation).
        #[arg(long, default_value_t = 4000)]
        publisher_port: u16,
        /// Output file.
        #[arg(long)]
        out: Option<PathBuf>,
        /// Expected Blake3 (hex) of the full stream.
        #[arg(long)]
        expect_hash: Option<String>,
        /// relay or leaf.
        #[arg(long, default_value = "relay")]
        class: String,
        /// NAT class: auto (from two PONG reflections), public, cone or symmetric.
        #[arg(long, default_value = "auto")]
        nat: String,
        /// Highest layer wanted.
        #[arg(long, default_value_t = 2)]
        top_layer: u8,
        /// Upload capacity, kbps.
        #[arg(long, default_value_t = 20_000)]
        upload_kbps: u32,
        /// Seed.
        #[arg(long, default_value_t = 2)]
        seed: u64,
        /// Stop after this many seconds (default: at STREAM_END).
        #[arg(long)]
        duration_s: Option<f64>,
        /// Write result.json here.
        #[arg(long)]
        result: Option<PathBuf>,
        /// Serve the web player and API on this address (e.g. 127.0.0.1:8080).
        #[arg(long)]
        player: Option<SocketAddr>,
    },
    /// Run many viewers in one process: one machine's share of a multi-machine swarm.
    Swarm {
        /// DHT bootstrap seed address, normally the publisher (host:port).
        #[arg(long)]
        bootstrap: String,
        /// Viewers to run.
        #[arg(long, default_value_t = 10)]
        count: u16,
        /// Stream id (hex). Default: the publisher's, derived from `--publisher-seed`.
        #[arg(long)]
        stream: Option<String>,
        /// Seed the publisher was started with (to derive the stream id).
        #[arg(long, default_value_t = 1)]
        publisher_seed: u64,
        /// Publisher listen port (part of its identity derivation).
        #[arg(long, default_value_t = 4000)]
        publisher_port: u16,
        /// IP every viewer binds; the OS picks each port.
        #[arg(long, default_value = "0.0.0.0")]
        listen_ip: IpAddr,
        /// External IP to advertise (default: detected toward the bootstrap).
        #[arg(long)]
        advertise_ip: Option<IpAddr>,
        /// relay or leaf.
        #[arg(long, default_value = "relay")]
        class: String,
        /// NAT class: auto (from two PONG reflections), public, cone or symmetric.
        #[arg(long, default_value = "auto")]
        nat: String,
        /// Highest layer wanted (default: every layer the stream has).
        #[arg(long, default_value_t = u8::MAX)]
        top_layer: u8,
        /// Upload capacity per viewer, kbps.
        #[arg(long, default_value_t = 20_000)]
        upload_kbps: u32,
        /// Seed of the first viewer; viewer i uses seed + i.
        #[arg(long, default_value_t = 1000)]
        seed: u64,
        /// Stop every viewer after this many seconds (default: at STREAM_END).
        #[arg(long)]
        duration_s: Option<f64>,
        /// Expected Blake3 (hex) of the full stream.
        #[arg(long)]
        expect_hash: Option<String>,
        /// Serve viewer i's player page and API on `--player-ip`:(port + i). Omit for none.
        #[arg(long)]
        player_base: Option<u16>,
        /// Address the player pages bind.
        #[arg(long, default_value = "0.0.0.0")]
        player_ip: IpAddr,
        /// Write result.json files here, one per viewer under <label>-<i>/.
        #[arg(long)]
        results: Option<PathBuf>,
        /// This machine's name in result paths and the summary (default: hostname).
        #[arg(long)]
        label: Option<String>,
        /// Milliseconds between viewer starts.
        #[arg(long, default_value_t = 250)]
        stagger_ms: u64,
    },
    /// Print the Blake3 hash of a file (what `watch --expect-hash` wants).
    Hash {
        /// File.
        file: PathBuf,
        /// Only the first N bytes.
        #[arg(long)]
        max_bytes: Option<usize>,
    },
    /// Print the stream id a publisher with this seed and port will have.
    StreamId {
        /// Seed.
        #[arg(long, default_value_t = 1)]
        seed: u64,
        /// Port.
        #[arg(long, default_value_t = 4000)]
        port: u16,
    },
}

fn ladder(spec: &str) -> anyhow::Result<Ladder> {
    if spec == "reference" {
        return Ok(Ladder::reference());
    }
    if let Some(k) = spec.strip_prefix("single:") {
        return Ok(Ladder::single(k.parse().context("ladder kbps")?));
    }
    anyhow::bail!("unknown ladder {spec}")
}

fn params(spec_pow: bool, parity: u16) -> Params {
    let mut p = if spec_pow {
        Params::default()
    } else {
        Params::for_simulation()
    };
    p.default_parity = parity.clamp(1, 5);
    p
}

/// The stream id of a publisher started with `seed` on `port` (identity derivation
/// mirrors `bs_core::Node::new`).
fn publisher_stream_id(seed: u64, port: u16, spec_pow: bool) -> StreamId {
    let p = params(spec_pow, 1);
    let mut s = [0u8; 32];
    s[..8].copy_from_slice(&seed.to_le_bytes());
    s[8..16].copy_from_slice(&port.to_le_bytes()[..].repeat(4)[..8]);
    let id = bs_crypto::Identity::from_seed(
        s,
        bs_crypto::Difficulty {
            c1: p.c1,
            c2_min: p.c2_min,
        },
    );
    bs_crypto::stream_id(&id.public_key())
}

/// `--nat` value to a reachability; `auto` and unknown words give `None`.
fn reachability(nat: &str) -> Option<bs_wire::Reachability> {
    match nat.to_ascii_lowercase().as_str() {
        "public" => Some(bs_wire::Reachability::Public),
        "cone" => Some(bs_wire::Reachability::Cone),
        "symmetric" => Some(bs_wire::Reachability::Symmetric),
        _ => None,
    }
}

/// The first address `host:port` resolves to.
///
/// A loopback bootstrap gets a warning. The viewer then binds its identity to
/// 127.0.0.1, and a publisher that advertises a LAN address rejects that.
async fn resolve_bootstrap(spec: &str) -> anyhow::Result<SocketAddr> {
    let addr = tokio::net::lookup_host(spec)
        .await?
        .next()
        .with_context(|| format!("resolving {spec}"))?;
    if addr.ip().is_loopback() {
        eprintln!(
            "bootstrap {addr} is loopback: that works only when the publisher listens on \
             127.0.0.1 too. With other machines in the swarm, use the address the \
             publisher printed."
        );
    }
    Ok(addr)
}

/// The stream id to join: `--stream` if given, else derived from the publisher's seed
/// and port.
fn resolve_stream(
    stream: Option<&str>,
    seed: u64,
    port: u16,
    spec_pow: bool,
) -> anyhow::Result<StreamId> {
    match stream {
        Some(h) => h.trim().parse::<StreamId>().context("--stream"),
        None => Ok(publisher_stream_id(seed, port, spec_pow)),
    }
}

/// What `watch` and `swarm` share when they build a viewer.
struct ViewerArgs {
    bootstrap: SocketAddr,
    stream_id: StreamId,
    listen: SocketAddr,
    advertise_ip: Option<IpAddr>,
    class: String,
    nat: String,
    top_layer: u8,
    upload_kbps: u32,
    seed: u64,
    duration_s: Option<f64>,
    result: Option<PathBuf>,
    media: Media,
    params: Params,
}

impl ViewerArgs {
    fn into_config(self) -> RunConfig {
        RunConfig {
            role: Role::Viewer {
                top_layer: self.top_layer,
            },
            node_class: if self.class.eq_ignore_ascii_case("leaf") {
                NodeClass::Leaf
            } else {
                NodeClass::Relay
            },
            reachability: reachability(&self.nat),
            listen: self.listen,
            advertise_ip: self.advertise_ip,
            bootstrap: Some(self.bootstrap),
            stream_id: Some(self.stream_id),
            upload_kbps: self.upload_kbps,
            ladder: Ladder::reference(),
            media: self.media,
            params: self.params,
            seed: self.seed,
            duration_s: self.duration_s,
            start_delay_s: 0.0,
            result_path: self.result,
            player: None,
        }
    }
}

/// This machine's hostname up to the first dot, or "node" if the OS does not say.
fn hostname() -> String {
    std::process::Command::new("hostname")
        .output()
        .ok()
        .map(|o| {
            let h = String::from_utf8_lossy(&o.stdout);
            h.trim().split('.').next().unwrap_or_default().to_string()
        })
        .filter(|h| !h.is_empty())
        .unwrap_or_else(|| "node".to_string())
}

/// The commands another machine runs to join this publisher.
fn print_join_hint(h: &bs_node::Handle) {
    eprintln!("join from another machine:");
    eprintln!(
        "  bsnode watch --bootstrap {} --stream {} --player 0.0.0.0:8080",
        h.advertise_addr, h.stream_id
    );
    eprintln!(
        "  bsnode swarm --bootstrap {} --stream {} --count 10 --player-base 8080",
        h.advertise_addr, h.stream_id
    );
}

/// One line per viewer plus the swarm totals, in the terms the fleet report uses.
fn print_swarm_summary(label: &str, reports: &[Option<bs_node::RunReport>]) -> bool {
    println!(
        "{:<14} {:<11} {:>8} {:>7} {:>7} {:>6} {:>7} {:>5}",
        "viewer", "state", "join s", "played", "starved", "full", "p.lost", "hash"
    );
    let (mut played, mut starved, mut never, mut mismatched) = (0u64, 0u64, 0usize, 0usize);
    let mut joins = Vec::new();
    for (i, r) in reports.iter().enumerate() {
        let name = format!("{label}-{i}");
        let Some(r) = r else {
            println!("{name:<14} {:<11}", "failed");
            never += 1;
            continue;
        };
        played += r.steady_played;
        starved += r.steady_starved;
        match r.first_chunk_s {
            Some(t) => joins.push(t),
            None => never += 1,
        }
        if r.hash_match == Some(false) {
            mismatched += 1;
        }
        let hash = match r.hash_match {
            Some(true) => "ok",
            Some(false) => "BAD",
            None => "-",
        };
        println!(
            "{name:<14} {:<11} {:>8} {:>7} {:>7} {:>6} {:>7} {:>5}",
            r.state,
            r.first_chunk_s
                .map(|t| format!("{t:.1}"))
                .unwrap_or_else(|| "-".into()),
            r.steady_played,
            r.steady_starved,
            r.chunks_full,
            r.parent_lost,
            hash
        );
    }
    joins.sort_by(|a, b| a.total_cmp(b));
    let pct = |q: f64| {
        joins
            .get(((joins.len() as f64 - 1.0) * q).round() as usize)
            .copied()
    };
    let psr = if played > 0 {
        starved as f64 / played as f64
    } else {
        1.0
    };
    println!(
        "{label}: {} viewers, {} played, {} never played, {} hash mismatches, starvation {:.3} %, join median {} p90 {}",
        reports.len(),
        joins.len(),
        never,
        mismatched,
        psr * 100.0,
        pct(0.5).map(|t| format!("{t:.1} s")).unwrap_or_else(|| "-".into()),
        pct(0.9).map(|t| format!("{t:.1} s")).unwrap_or_else(|| "-".into()),
    );
    never == 0 && mismatched == 0
}

/// Stop every node gracefully on Ctrl-C or SIGTERM (docker stop), so result.json is
/// written and child processes (ffmpeg) are reaped.
fn stop_on_signal(stops: Vec<tokio::sync::watch::Sender<bool>>) {
    tokio::spawn(async move {
        #[cfg(unix)]
        {
            let mut term =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                    .expect("sigterm handler");
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                _ = term.recv() => {}
            }
        }
        #[cfg(not(unix))]
        {
            let _ = tokio::signal::ctrl_c().await;
        }
        eprintln!("stopping...");
        for stop in &stops {
            let _ = stop.send(true);
        }
    });
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,quinn=warn".into()),
        )
        .init();
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Swarm {
            bootstrap,
            count,
            stream,
            publisher_seed,
            publisher_port,
            listen_ip,
            advertise_ip,
            class,
            nat,
            top_layer,
            upload_kbps,
            seed,
            duration_s,
            expect_hash,
            player_base,
            player_ip,
            results,
            label,
            stagger_ms,
        } => {
            let bootstrap = resolve_bootstrap(&bootstrap).await?;
            let stream_id = resolve_stream(
                stream.as_deref(),
                publisher_seed,
                publisher_port,
                cli.spec_pow,
            )?;
            let label = label.unwrap_or_else(hostname);
            let params = params(cli.spec_pow, cli.parity);
            let mut handles = Vec::with_capacity(count as usize);
            for i in 0..count {
                let result = results
                    .as_ref()
                    .map(|d| d.join(format!("{label}-{i}")).join("result.json"));
                let media = match player_base {
                    Some(base) => Media::Play {
                        addr: SocketAddr::new(player_ip, base + i),
                        path: None,
                        expect_hash: expect_hash.clone(),
                    },
                    None => Media::Output {
                        path: None,
                        expect_hash: expect_hash.clone(),
                    },
                };
                let cfg = ViewerArgs {
                    bootstrap,
                    stream_id,
                    listen: SocketAddr::new(listen_ip, 0),
                    advertise_ip,
                    class: class.clone(),
                    nat: nat.clone(),
                    top_layer,
                    upload_kbps,
                    seed: seed + u64::from(i),
                    duration_s,
                    result,
                    media,
                    params: params.clone(),
                }
                .into_config();
                let h = bs_node::start(cfg)
                    .await
                    .with_context(|| format!("viewer {label}-{i}"))?;
                eprintln!(
                    "{label}-{i}: node {} on {}{}",
                    h.node_id.short(),
                    h.local_addr,
                    h.player_addr
                        .map(|a| format!(", player http://{a}/"))
                        .unwrap_or_default()
                );
                handles.push(h);
                tokio::time::sleep(Duration::from_millis(stagger_ms)).await;
            }
            eprintln!("{label}: {count} viewers on stream {stream_id} via {bootstrap}");
            stop_on_signal(handles.iter().map(|h| h.stop.clone()).collect());
            let statuses: Vec<_> = handles.iter().map(|h| h.status.clone()).collect();
            let progress = tokio::spawn(async move {
                let mut tick = tokio::time::interval(Duration::from_secs(10));
                tick.tick().await;
                loop {
                    tick.tick().await;
                    let (mut playing, mut played, mut starved) = (0usize, 0u64, 0u64);
                    let mut errors = Vec::new();
                    for s in &statuses {
                        let s = s.borrow();
                        playing += usize::from(s.played > 0);
                        played += s.played;
                        starved += s.starved;
                        if let Some(e) = &s.last_error {
                            errors.push(e.clone());
                        }
                    }
                    errors.sort();
                    errors.dedup();
                    eprintln!(
                        "{playing}/{} playing, {played} chunks played, {starved} starved{}",
                        statuses.len(),
                        if errors.is_empty() {
                            String::new()
                        } else {
                            format!(", errors: {}", errors.join("; "))
                        }
                    );
                }
            });
            let mut reports = Vec::with_capacity(handles.len());
            for (i, h) in handles.into_iter().enumerate() {
                match h.done.await? {
                    Ok(r) => reports.push(Some(r)),
                    Err(e) => {
                        eprintln!("{label}-{i} failed: {e:#}");
                        reports.push(None);
                    }
                }
            }
            progress.abort();
            if !print_swarm_summary(&label, &reports) {
                std::process::exit(3);
            }
        }
        Cmd::Hash { file, max_bytes } => {
            println!("{}", bs_node::file_hash(&file, max_bytes)?);
        }
        Cmd::StreamId { seed, port } => {
            println!("{}", publisher_stream_id(seed, port, cli.spec_pow))
        }
        Cmd::Publish {
            file,
            max_bytes,
            repeat,
            listen,
            advertise_ip,
            ladder: l,
            forest,
            upload_kbps,
            seed,
            start_delay_s,
            duration_s,
            result,
            player,
            ingest,
            input,
            renditions,
            fps,
            nat,
        } => {
            let (media, lad) = if ingest.eq_ignore_ascii_case("ffmpeg") {
                let input = input
                    .or(file)
                    .context("--ingest ffmpeg needs --input <file>")?;
                let specs = bs_ingest::RenditionSpec::parse_list(&renditions)?;
                let lad = bs_ingest::RenditionSpec::ladder(&specs);
                (
                    Media::Ffmpeg {
                        input,
                        renditions: specs
                            .iter()
                            .map(|r| format!("{}:{}", r.label, r.kbps))
                            .collect(),
                        fps,
                    },
                    lad,
                )
            } else {
                let media = match file {
                    Some(path) => Media::File {
                        path,
                        repeat,
                        max_bytes,
                    },
                    None => Media::Synthetic { seed },
                };
                (media, ladder(&l)?)
            };
            let cfg = RunConfig {
                role: Role::Publisher {
                    ladder: lad.clone(),
                    forest_size: forest,
                },
                node_class: NodeClass::Relay,
                reachability: Some(reachability(&nat).unwrap_or(bs_wire::Reachability::Public)),
                listen,
                advertise_ip,
                bootstrap: None,
                stream_id: None,
                upload_kbps,
                ladder: lad,
                media,
                params: params(cli.spec_pow, cli.parity),
                seed,
                duration_s,
                start_delay_s,
                result_path: result,
                player,
            };
            let h = bs_node::start(cfg).await?;
            eprintln!(
                "publishing stream {} on {} (node {})",
                h.stream_id,
                h.local_addr,
                h.node_id.short()
            );
            print_join_hint(&h);
            stop_on_signal(vec![h.stop.clone()]);
            let report = h.done.await??;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Cmd::Watch {
            bootstrap,
            listen,
            advertise_ip,
            stream,
            publisher_seed,
            publisher_port,
            out,
            expect_hash,
            class,
            nat,
            top_layer,
            upload_kbps,
            seed,
            duration_s,
            result,
            player,
        } => {
            let bootstrap = resolve_bootstrap(&bootstrap).await?;
            let stream_id = resolve_stream(
                stream.as_deref(),
                publisher_seed,
                publisher_port,
                cli.spec_pow,
            )?;
            let cfg = ViewerArgs {
                bootstrap,
                stream_id,
                listen,
                advertise_ip,
                class,
                nat,
                top_layer,
                upload_kbps,
                seed,
                duration_s,
                result,
                media: match player {
                    Some(addr) => Media::Play {
                        addr,
                        path: out,
                        expect_hash,
                    },
                    None => Media::Output {
                        path: out,
                        expect_hash,
                    },
                },
                params: params(cli.spec_pow, cli.parity),
            }
            .into_config();
            let h = bs_node::start(cfg).await?;
            eprintln!(
                "watching stream {} via {} on {} (node {})",
                h.stream_id,
                bootstrap,
                h.local_addr,
                h.node_id.short()
            );
            stop_on_signal(vec![h.stop.clone()]);
            let report = h.done.await??;
            println!("{}", serde_json::to_string_pretty(&report)?);
            if report.hash_match == Some(false) {
                std::process::exit(3);
            }
        }
    }
    Ok(())
}
