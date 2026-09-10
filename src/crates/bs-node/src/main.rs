//! `bsnode` — publish or watch a BitStream over a real network.

#![forbid(unsafe_code)]

use std::net::SocketAddr;
use std::path::PathBuf;

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
}

#[derive(Subcommand)]
enum Cmd {
    /// Publish a file (or synthetic bytes) and act as the bootstrap guardian.
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
    },
    /// Join a stream through a bootstrap guardian and write the reassembled bytes.
    Watch {
        /// Bootstrap guardian address (host:port).
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

fn params(spec_pow: bool) -> Params {
    if spec_pow {
        Params::default()
    } else {
        Params::for_simulation()
    }
}

/// The stream id of a publisher started with `seed` on `port` (identity derivation
/// mirrors `bs_core::Node::new`).
fn publisher_stream_id(seed: u64, port: u16, spec_pow: bool) -> StreamId {
    let p = params(spec_pow);
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


/// Stop the node gracefully on Ctrl-C or SIGTERM (docker stop), so result.json is
/// written and child processes (ffmpeg) are reaped.
fn stop_on_signal(stop: tokio::sync::watch::Sender<bool>) {
    tokio::spawn(async move {
        #[cfg(unix)]
        {
            let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).expect("sigterm handler");
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
        let _ = stop.send(true);
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
            ingest,
            input,
            renditions,
            fps,
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
                listen,
                advertise_ip,
                bootstrap: None,
                stream_id: None,
                upload_kbps,
                ladder: lad,
                media,
                params: params(cli.spec_pow),
                seed,
                duration_s,
                start_delay_s,
                result_path: result,
            };
            let h = bs_node::start(cfg).await?;
            eprintln!(
                "publishing stream {} on {} (node {})",
                h.stream_id,
                h.local_addr,
                h.node_id.short()
            );
            stop_on_signal(h.stop.clone());
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
            top_layer,
            upload_kbps,
            seed,
            duration_s,
            result,
            player,
        } => {
            let bootstrap = tokio::net::lookup_host(&bootstrap)
                .await?
                .next()
                .with_context(|| format!("resolving {bootstrap}"))?;
            let stream_id = match stream {
                Some(h) => {
                    let v = hex::decode(h).context("stream id hex")?;
                    StreamId(
                        v.try_into()
                            .map_err(|_| anyhow::anyhow!("stream id must be 32 bytes"))?,
                    )
                }
                None => publisher_stream_id(publisher_seed, publisher_port, cli.spec_pow),
            };
            let cfg = RunConfig {
                role: Role::Viewer { top_layer },
                node_class: if class.eq_ignore_ascii_case("leaf") {
                    NodeClass::Leaf
                } else {
                    NodeClass::Relay
                },
                listen,
                advertise_ip,
                bootstrap: Some(bootstrap),
                stream_id: Some(stream_id),
                upload_kbps,
                ladder: Ladder::reference(),
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
                params: params(cli.spec_pow),
                seed,
                duration_s,
                start_delay_s: 0.0,
                result_path: result,
            };
            let h = bs_node::start(cfg).await?;
            eprintln!(
                "watching stream {} via {} on {} (node {})",
                h.stream_id,
                bootstrap,
                h.local_addr,
                h.node_id.short()
            );
            stop_on_signal(h.stop.clone());
            let report = h.done.await??;
            println!("{}", serde_json::to_string_pretty(&report)?);
            if report.hash_match == Some(false) {
                std::process::exit(3);
            }
        }
    }
    Ok(())
}
