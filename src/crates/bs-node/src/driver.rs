//! The real-network driver: runs one `bs_core::Node` over UDP + QUIC (quinn).
//!
//! Everything the node emits as [`Output`] is translated here; everything the
//! network delivers becomes an [`Input`]. Time is the tokio clock, expressed to
//! the node as microseconds since the driver started.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Context};
use bs_core::{
    Channel, Command, Event, Input, Instant, Lifecycle, Node, NodeConfig, Output, Params, Role,
};
use bs_crypto::pow::Difficulty;
use bs_media::source::Source;
use bs_media::{FileSource, Ladder, LayeredChunk, SyntheticSource};
use bs_wire::{Frame, FrameType, NodeClass, NodeId, StreamId};
use bytes::{Buf, Bytes, BytesMut};
use serde::{Deserialize, Serialize};
use tokio::io::AsyncReadExt;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use crate::tls;
use crate::transport::{DemuxSocket, PlainDatagram};
use quinn::AsyncUdpSocket as _;

/// How the node gets its media (publisher) or where it puts it (viewer).
#[derive(Debug, Clone)]
pub enum Media {
    /// Publisher: stream this file as opaque bytes at the ladder rate.
    File {
        /// Path.
        path: PathBuf,
        /// Loop when exhausted.
        repeat: bool,
        /// Only the first `n` bytes.
        max_bytes: Option<usize>,
    },
    /// Publisher: synthetic bytes.
    Synthetic {
        /// Seed.
        seed: u64,
    },
    /// Viewer: write reassembled layers here (concatenated in order).
    Output {
        /// Path, or `None` to only hash.
        path: Option<PathBuf>,
        /// Expected Blake3 of the whole stream, hex.
        expect_hash: Option<String>,
    },
    /// Publisher: live simulcast renditions from ffmpeg (layer `l` = rendition `l`).
    Ffmpeg {
        /// Input file or URL.
        input: PathBuf,
        /// Renditions, e.g. `["480p:1500", "720p:3000"]`.
        renditions: Vec<String>,
        /// Frame rate (`None` = probe).
        fps: Option<u32>,
    },
    /// Viewer: serve the player API (and optionally also write/hash the output).
    Play {
        /// Player API bind address.
        addr: SocketAddr,
        /// Optional output file.
        path: Option<PathBuf>,
        /// Expected hash of the whole stream, hex.
        expect_hash: Option<String>,
    },
}

/// Runtime configuration.
#[derive(Debug, Clone)]
pub struct RunConfig {
    /// Publisher or viewer.
    pub role: Role,
    /// Class.
    pub node_class: NodeClass,
    /// NAT reachability if known; `None` lets the node decide it from PONGs.
    pub reachability: Option<bs_wire::Reachability>,
    /// Bind address.
    pub listen: SocketAddr,
    /// External IP to advertise / bind PoW to (`None` = detect).
    pub advertise_ip: Option<std::net::IpAddr>,
    /// Bootstrap guardian (viewers).
    pub bootstrap: Option<SocketAddr>,
    /// Stream to join (viewers). Publishers derive it.
    pub stream_id: Option<StreamId>,
    /// Upload capacity.
    pub upload_kbps: u32,
    /// Ladder (publisher).
    pub ladder: Ladder,
    /// Media.
    pub media: Media,
    /// Protocol parameters.
    pub params: Params,
    /// Seed.
    pub seed: u64,
    /// Stop after this many seconds (`None` = until stream end).
    pub duration_s: Option<f64>,
    /// Publisher: wait this long before the first chunk so early viewers attach.
    pub start_delay_s: f64,
    /// Where to write result.json.
    pub result_path: Option<PathBuf>,
    /// Serve the player page / status API here (either role).
    pub player: Option<SocketAddr>,
}

/// What a run produced.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunReport {
    /// "publisher" or "viewer".
    pub role: String,
    /// NodeID.
    pub node_id: String,
    /// Stream id.
    pub stream_id: String,
    /// Bound address.
    pub listen: String,
    /// Final lifecycle state.
    pub state: String,
    /// Seconds run.
    pub seconds: f64,
    /// Chunks published (publisher).
    pub chunks_published: u64,
    /// Chunks played (viewer).
    pub chunks_played: u64,
    /// Chunks with the base layer complete.
    pub chunks_ok: u64,
    /// Chunks with every subscribed layer complete.
    pub chunks_full: u64,
    /// Steady-state starved.
    pub steady_starved: u64,
    /// Steady-state played.
    pub steady_played: u64,
    /// Time to first ok chunk.
    pub first_chunk_s: Option<f64>,
    /// First segment delivered fully.
    pub first_segment: Option<u32>,
    /// Bytes written to the output.
    pub bytes_out: u64,
    /// Blake3 of the output.
    pub output_hash: Option<String>,
    /// Expected hash, if given.
    pub expected_hash: Option<String>,
    /// Whether they match.
    pub hash_match: Option<bool>,
    /// Blake3 of the source (publisher).
    pub source_hash: Option<String>,
    /// Frames sent by type.
    pub sent: HashMap<String, u64>,
    /// Registrations this node holds as a DHT guardian.
    pub guardian_peers: Option<usize>,
    /// Parent losses.
    pub parent_lost: u64,
    /// Repairs.
    pub repairs: u64,
    /// Sessions accepted/opened.
    pub sessions: u64,
    /// Per-connection QUIC path statistics at exit (congestion, loss, MTU).
    pub connections: Vec<ConnStats>,
    /// QUIC datagrams the endpoint refused to queue (`send_datagram` errors).
    pub datagram_send_errors: u64,
    /// Incentive counters (Ch5): receipts issued / accepted / rejected by reason,
    /// deadline misses by our children, children evicted for missed receipts,
    /// choke-state changes toward peers, PULL_REQUESTs refused as choked.
    pub incentives: IncentiveCounters,
}

/// One QUIC connection's path statistics (quinn `ConnectionStats`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnStats {
    /// Remote address.
    pub peer: String,
    /// Smoothed RTT, ms.
    pub rtt_ms: f64,
    /// Congestion window, bytes.
    pub cwnd: u64,
    /// Congestion events.
    pub congestion_events: u64,
    /// Packets sent / lost.
    pub sent_packets: u64,
    /// Packets lost.
    pub lost_packets: u64,
    /// Current path MTU.
    pub mtu: u16,
    /// DATAGRAM frames sent / received.
    pub datagrams_tx: u64,
    /// DATAGRAM frames received.
    pub datagrams_rx: u64,
}

/// Incentive-layer counters in the run report.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IncentiveCounters {
    /// PROOF_OF_UPLOAD receipts we signed.
    pub receipts_issued: u64,
    /// Receipts accepted into our ledger.
    pub receipts_accepted: u64,
    /// Receipts rejected, by reason.
    pub receipts_rejected: HashMap<String, u64>,
    /// Receipt deadlines our children missed.
    pub receipts_missed: u64,
    /// Children evicted for three missed receipts.
    pub deadline_evictions: u64,
    /// Choke-state changes toward peers, by new state.
    pub choke_changes: HashMap<String, u64>,
    /// PULL_REQUESTs refused because the requester was choked.
    pub pulls_choked: u64,
    /// Receipts held as uploader at exit, and the rank they give.
    pub receipts_held: usize,
    /// Θ^rate / B at exit.
    pub rank: f64,
}

#[derive(Debug)]
enum Ev {
    /// A paced datagram could not be queued.
    DatagramSendError,
    Connected {
        peer: SocketAddr,
        node_id: NodeId,
        conn: quinn::Connection,
    },
    ConnectFailed {
        peer: SocketAddr,
        err: String,
    },
    StreamFrame {
        peer: SocketAddr,
        channel: Channel,
        frame: Frame,
    },
    Datagram {
        peer: SocketAddr,
        frame: Frame,
    },
    Closed {
        peer: SocketAddr,
    },
}

struct Conn {
    conn: quinn::Connection,
    #[allow(dead_code)]
    node_id: NodeId,
    writers: HashMap<u8, mpsc::UnboundedSender<Bytes>>,
}

fn channel_tag(c: Channel) -> u8 {
    match c {
        Channel::Control => 0,
        Channel::Tree(t) => t.0,
        Channel::Pull => 0xF0,
        _ => 0xFF,
    }
}
fn tag_channel(t: u8) -> Channel {
    match t {
        0 => Channel::Control,
        0xF0 => Channel::Pull,
        t => Channel::Tree(bs_wire::TreeId(t)),
    }
}

/// Senders into the player API: deliveries, status, descriptor.
type PlayerHandles = (
    tokio::sync::broadcast::Sender<bs_player_api::Delivered>,
    tokio::sync::watch::Sender<bs_player_api::Status>,
    tokio::sync::watch::Sender<Option<bs_player_api::DescriptorInfo>>,
);

/// A running node's handle.
pub struct Handle {
    /// Bound address.
    pub local_addr: SocketAddr,
    /// Stream id.
    pub stream_id: StreamId,
    /// NodeID.
    pub node_id: NodeId,
    /// Completion.
    pub done: tokio::task::JoinHandle<anyhow::Result<RunReport>>,
    /// Ask the node to stop.
    pub stop: tokio::sync::watch::Sender<bool>,
}

/// Start a node. Returns once the socket is bound.
pub async fn start(cfg: RunConfig) -> anyhow::Result<Handle> {
    let (sock, plain_rx) = DemuxSocket::bind(cfg.listen).context("bind")?;
    let local_addr = sock.local_addr()?;
    let advertise_ip = match cfg.advertise_ip {
        Some(ip) => ip,
        None => detect_ip(cfg.bootstrap, local_addr).await,
    };
    let addr = SocketAddr::new(advertise_ip, local_addr.port());

    let stream_id = cfg.stream_id.unwrap_or(StreamId::ZERO);
    let node = Node::new(NodeConfig {
        role: cfg.role.clone(),
        node_class: cfg.node_class,
        upload_kbps: cfg.upload_kbps,
        addr,
        stream_id,
        bootstrap: cfg.bootstrap.into_iter().collect(),
        discovery: bs_core::DiscoveryMode::Dht,
        reachability: cfg.reachability,
        params: cfg.params.clone(),
        seed: cfg.seed,
    });
    let stream_id = node.stream_id();
    let node_id = node.node_id();
    let identity = bs_crypto::Identity::from_seed(
        node_seed(&cfg, addr),
        Difficulty {
            c1: cfg.params.c1,
            c2_min: cfg.params.c2_min,
        },
    );
    debug_assert_eq!(identity.node_id(), node_id);

    let difficulty = Difficulty {
        c1: cfg.params.c1,
        c2_min: cfg.params.c2_min,
    };
    let (client_cfg, server_cfg) = tls::quic_configs(&identity, difficulty)?;
    // The plain-frame demux relies on the QUIC fixed bit (0x40); RFC 9287 greasing
    // would clear it, so it is disabled here (and, by not advertising it, on peers).
    let mut endpoint_cfg = quinn::EndpointConfig::default();
    endpoint_cfg.grease_quic_bit(false);
    let endpoint = quinn::Endpoint::new_with_abstract_socket(
        endpoint_cfg,
        Some(server_cfg),
        sock.clone(),
        Arc::new(quinn::TokioRuntime),
    )
    .context("endpoint")?;
    let mut endpoint = endpoint;
    endpoint.set_default_client_config(client_cfg);

    let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);
    let driver = Driver::new(
        cfg, node, identity, sock, endpoint, plain_rx, local_addr, stop_rx,
    )?;
    let done = tokio::spawn(driver.run());
    Ok(Handle {
        local_addr,
        stream_id,
        node_id,
        done,
        stop: stop_tx,
    })
}

/// Mirror of `Node::new`'s seed derivation so the TLS identity equals the node's.
fn node_seed(cfg: &RunConfig, addr: SocketAddr) -> [u8; 32] {
    let mut seed = [0u8; 32];
    seed[..8].copy_from_slice(&cfg.seed.to_le_bytes());
    seed[8..16].copy_from_slice(&addr.port().to_le_bytes()[..].repeat(4)[..8]);
    seed
}

/// The IP peers will see us from: the bind IP if explicit, else the source IP the
/// OS picks toward the bootstrap (viewers) or toward a public address (publisher).
/// `connect` on a UDP socket sends nothing; it only resolves the route.
async fn detect_ip(bootstrap: Option<SocketAddr>, local: SocketAddr) -> std::net::IpAddr {
    if !local.ip().is_unspecified() {
        return local.ip();
    }
    let target = bootstrap.unwrap_or_else(|| "1.1.1.1:53".parse().unwrap());
    if let Ok(s) = tokio::net::UdpSocket::bind("0.0.0.0:0").await {
        if s.connect(target).await.is_ok() {
            if let Ok(a) = s.local_addr() {
                if !a.ip().is_unspecified() {
                    return a.ip();
                }
            }
        }
    }
    std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)
}

struct Sink {
    file: Option<std::fs::File>,
    hasher: blake3::Hasher,
    bytes: u64,
    started: bool,
    first_segment: Option<u32>,
}

struct Driver {
    cfg: RunConfig,
    node: Node,
    sock: Arc<DemuxSocket>,
    endpoint: quinn::Endpoint,
    plain_rx: mpsc::UnboundedReceiver<PlainDatagram>,
    ev_tx: mpsc::UnboundedSender<Ev>,
    ev_rx: mpsc::UnboundedReceiver<Ev>,
    conns: HashMap<SocketAddr, Conn>,
    epoch: tokio::time::Instant,
    source: Option<Box<dyn Source + Send>>,
    source_hash: Option<bs_wire::Hash>,
    sink: Sink,
    stop_rx: tokio::sync::watch::Receiver<bool>,
    local_addr: SocketAddr,
    // stats
    sent: HashMap<FrameType, u64>,
    chunks_published: u64,
    played: u64,
    ok: u64,
    full: u64,
    steady_played: u64,
    steady_starved: u64,
    first_chunk: Option<f64>,
    parent_lost: u64,
    repairs: u64,
    sessions: u64,
    incentives: IncentiveCounters,
    datagram_send_errors: u64,
    ended: bool,
    checkpoints: std::collections::VecDeque<bs_player_api::Checkpoint>,
    descriptor_version: u32,
    blocks_rejected: u64,
    pub_hasher: blake3::Hasher,
    pub_bytes: u64,
    end_at: Option<tokio::time::Instant>,
    /// Live chunks from ffmpeg (publisher, `Media::Ffmpeg`).
    live_rx: Option<mpsc::Receiver<LayeredChunk>>,
    /// Player API senders (status, deliveries, descriptor).
    player: Option<PlayerHandles>,
    layers_complete_last: u8,
}

impl Driver {
    #[allow(clippy::too_many_arguments)]
    fn new(
        cfg: RunConfig,
        node: Node,
        identity: bs_crypto::Identity,
        sock: Arc<DemuxSocket>,
        endpoint: quinn::Endpoint,
        plain_rx: mpsc::UnboundedReceiver<PlainDatagram>,
        local_addr: SocketAddr,
        stop_rx: tokio::sync::watch::Receiver<bool>,
    ) -> anyhow::Result<Self> {
        let (ev_tx, ev_rx) = mpsc::unbounded_channel();
        let difficulty = Difficulty {
            c1: cfg.params.c1,
            c2_min: cfg.params.c2_min,
        };
        let is_pub = matches!(cfg.role, Role::Publisher { .. });
        let (source, source_hash): (Option<Box<dyn Source + Send>>, Option<bs_wire::Hash>) =
            match &cfg.media {
                Media::File {
                    path,
                    repeat: _,
                    max_bytes,
                } if is_pub => {
                    let mut data = std::fs::read(path)
                        .with_context(|| format!("reading {}", path.display()))?;
                    if let Some(n) = max_bytes {
                        data.truncate(*n);
                    }
                    let fs = FileSource::new(cfg.ladder.clone(), Bytes::from(data));
                    let h = fs.file_hash();
                    (Some(Box::new(fs)), Some(h))
                }
                Media::Synthetic { seed } if is_pub => (
                    Some(Box::new(SyntheticSource::new(
                        cfg.ladder.clone(),
                        *seed,
                        None,
                    ))),
                    None,
                ),
                _ => (None, None),
            };
        let sink = match &cfg.media {
            Media::Output { path: Some(p), .. } | Media::Play { path: Some(p), .. } => Sink {
                file: Some(
                    std::fs::File::create(p)
                        .with_context(|| format!("creating {}", p.display()))?,
                ),
                hasher: blake3::Hasher::new(),
                bytes: 0,
                started: false,
                first_segment: None,
            },
            _ => Sink {
                file: None,
                hasher: blake3::Hasher::new(),
                bytes: 0,
                started: false,
                first_segment: None,
            },
        };
        let _ = (difficulty, identity);
        Ok(Self {
            cfg,
            node,
            sock,
            endpoint,
            plain_rx,
            ev_tx,
            ev_rx,
            conns: HashMap::new(),
            epoch: tokio::time::Instant::now(),
            source,
            source_hash,
            sink,
            stop_rx,
            local_addr,
            sent: HashMap::new(),
            chunks_published: 0,
            played: 0,
            ok: 0,
            full: 0,
            steady_played: 0,
            steady_starved: 0,
            first_chunk: None,
            parent_lost: 0,
            incentives: IncentiveCounters::default(),
            datagram_send_errors: 0,
            repairs: 0,
            sessions: 0,
            ended: false,
            checkpoints: std::collections::VecDeque::new(),
            descriptor_version: 0,
            blocks_rejected: 0,
            pub_hasher: blake3::Hasher::new(),
            pub_bytes: 0,
            end_at: None,
            live_rx: None,
            player: None,
            layers_complete_last: 0,
        })
    }

    fn now(&self) -> Instant {
        Instant(self.epoch.elapsed().as_micros() as u64)
    }

    async fn run(mut self) -> anyhow::Result<RunReport> {
        // Accept loop.
        let ep = self.endpoint.clone();
        let tx = self.ev_tx.clone();
        let c1 = self.cfg.params.c1;
        tokio::spawn(async move {
            while let Some(incoming) = ep.accept().await {
                let tx = tx.clone();
                tokio::spawn(async move {
                    match incoming.await {
                        Ok(conn) => match tls::connection_peer(&conn, c1) {
                            Ok(pid) => {
                                let _ = tx.send(Ev::Connected {
                                    peer: conn.remote_address(),
                                    node_id: pid.node_id,
                                    conn,
                                });
                            }
                            Err(e) => warn!("rejecting peer: {e}"),
                        },
                        Err(e) => debug!("incoming handshake failed: {e}"),
                    }
                });
            }
        });

        let is_pub = matches!(self.cfg.role, Role::Publisher { .. });
        self.setup_live_media().await?;
        let live = self.live_rx.is_some();
        self.feed(Input::Cmd(Command::Start));
        let mut publish = tokio::time::interval(std::time::Duration::from_micros(
            self.cfg.params.chunk_period.as_micros(),
        ));
        publish.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let start_media_at =
            self.epoch + std::time::Duration::from_secs_f64(self.cfg.start_delay_s);
        let deadline = self
            .cfg
            .duration_s
            .map(|d| self.epoch + std::time::Duration::from_secs_f64(d));
        loop {
            let next_timer = self
                .node
                .next_timer()
                .map(|t| self.epoch + std::time::Duration::from_micros(t.0));
            let timer = async {
                match next_timer {
                    Some(t) => tokio::time::sleep_until(t).await,
                    None => std::future::pending::<()>().await,
                }
            };
            let end = async {
                match self.end_at.or(deadline) {
                    Some(t) => tokio::time::sleep_until(t).await,
                    None => std::future::pending::<()>().await,
                }
            };
            tokio::select! {
                Some(p) = self.plain_rx.recv() => self.on_plain(p),
                Some(ev) = self.ev_rx.recv() => self.on_ev(ev),
                _ = timer => self.feed(Input::Tick),
                _ = publish.tick(), if is_pub && !self.ended && !live => {
                    if tokio::time::Instant::now() >= start_media_at {
                        self.publish_tick();
                    }
                }
                c = async { self.live_rx.as_mut().unwrap().recv().await }, if is_pub && live && !self.ended => {
                    match c {
                        Some(c) => {
                            self.chunks_published += 1;
                            self.chunks_published += 1;
                            self.note_published(&c);
                            self.feed(Input::Cmd(Command::PublishChunk(c)));
                        }
                        None => {
                            info!("ffmpeg ingest ended after {} chunks; sending STREAM_END", self.chunks_published);
                            self.ended = true;
                            self.feed(Input::Cmd(Command::EndStream));
                        }
                    }
                }
                _ = self.stop_rx.changed() => { self.feed(Input::Cmd(Command::Quit)); break; }
                _ = end => break,
            }
            if self.node.state() == Lifecycle::Terminated && self.end_at.is_none() {
                // Give in-flight frames a moment to leave.
                self.end_at =
                    Some(tokio::time::Instant::now() + std::time::Duration::from_millis(500));
            }
        }
        let report = self.report().await;
        self.endpoint.close(0u32.into(), b"bye");
        if let Some(p) = &self.cfg.result_path {
            if let Some(dir) = p.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            std::fs::write(p, serde_json::to_string_pretty(&report)?)?;
        }
        Ok(report)
    }

    /// Start ffmpeg ingest (publisher, after `start_delay_s`) and the player API (viewer).
    async fn setup_live_media(&mut self) -> anyhow::Result<()> {
        let player_addr = self.cfg.player.or(match &self.cfg.media {
            Media::Play { addr, .. } => Some(*addr),
            _ => None,
        });
        if let Some(addr) = player_addr {
            let (state, dtx, stx, xtx) = bs_player_api::PlayerState::new();
            let layers = self
                .node
                .matrix()
                .map(|m| m.layer_count)
                .unwrap_or(self.cfg.ladder.len() as u8);
            stx.send_modify(|s| s.layers = layers);
            let (bound, _task) = bs_player_api::serve(addr, state).await?;
            info!("player: open http://{bound}/");
            self.player = Some((dtx, stx, xtx));
            self.publish_status();
        }
        match self.cfg.media.clone() {
            Media::Ffmpeg {
                input,
                renditions,
                fps,
            } if matches!(self.cfg.role, Role::Publisher { .. }) => {
                let specs = bs_ingest::RenditionSpec::parse_list(&renditions.join(","))?;
                let (tx, rx) = mpsc::channel::<LayeredChunk>(16);
                let delay = std::time::Duration::from_secs_f64(self.cfg.start_delay_s);
                tokio::spawn(async move {
                    tokio::time::sleep(delay).await;
                    let mut src = match bs_ingest::FfmpegSource::spawn(input, specs, fps).await {
                        Ok(s) => s,
                        Err(e) => {
                            warn!("ffmpeg ingest failed to start: {e:#}");
                            return;
                        }
                    };
                    while let Some(c) = src.next_chunk().await {
                        if tx.send(c).await.is_err() {
                            return;
                        }
                    }
                });
                self.live_rx = Some(rx);
            }
            _ => {}
        }
        Ok(())
    }

    /// Status refresh scheduled from an event (the counters update below, so refresh
    /// after `on_event` finishes by deferring to the next `deliver`/tick is enough
    /// for state and topology changes here).
    fn publish_status_after(&mut self, _e: &Event) {
        self.publish_status();
    }

    /// Record a per-chunk integrity checkpoint (bounded history).
    fn checkpoint(&mut self, segment: u32, chunk: u8, layers: &[Bytes]) {
        let layer_hashes = layers
            .iter()
            .map(|l| hex::encode(&blake3::hash(l).as_bytes()[..8]))
            .collect();
        self.checkpoints.push_back(bs_player_api::Checkpoint {
            segment,
            chunk,
            layer_hashes,
        });
        while self.checkpoints.len() > 96 {
            self.checkpoints.pop_front();
        }
    }

    fn publish_status(&mut self) {
        let Some((_, stx, _)) = self.player.as_ref() else {
            return;
        };
        let depth: Vec<u8> = self.node.trees().iter().map(|t| t.depth).collect();
        let is_pub = matches!(self.cfg.role, Role::Publisher { .. });
        let (bytes, hash_so_far) = if is_pub {
            (
                self.pub_bytes,
                if self.pub_bytes > 0 {
                    Some(hex::encode(self.pub_hasher.finalize().as_bytes()))
                } else {
                    None
                },
            )
        } else {
            (
                self.sink.bytes,
                if self.sink.started {
                    Some(hex::encode(self.sink.hasher.finalize().as_bytes()))
                } else {
                    None
                },
            )
        };
        let st = bs_player_api::Status {
            role: if is_pub {
                "publisher".into()
            } else {
                "viewer".into()
            },
            node_id: self.node.node_id().to_string(),
            chunks_ok: self.ok,
            blocks_rejected: self.blocks_rejected,
            parent_lost: self.parent_lost,
            bytes,
            hash_so_far,
            source_hash: self.source_hash.map(|h| h.to_string()),
            descriptor_version: self.descriptor_version,
            checkpoints: self.checkpoints.iter().cloned().collect(),
            state: format!("{:?}", self.node.state()),
            layers: self
                .node
                .matrix()
                .map(|m| m.layer_count)
                .unwrap_or(self.cfg.ladder.len() as u8),
            layers_complete_last: self.layers_complete_last,
            depth,
            played: self.played,
            starved: self.steady_starved,
            chunks_full: self.full,
            first_chunk_s: self.first_chunk,
        };
        let _ = stx.send(st);
    }

    fn feed(&mut self, input: Input) {
        let now = self.now();
        self.node.handle(input, now);
        while let Some(out) = self.node.poll_output() {
            self.on_output(out);
        }
    }

    /// Hand a descriptor to the player page (init segments ride kind-1 frames).
    fn publish_descriptor(&mut self, d: &bs_wire::frames::StreamDescriptor) {
        let Some((_, _, xtx)) = self.player.as_ref() else {
            return;
        };
        let info = bs_player_api::DescriptorInfo {
            version: d.version,
            content_type: format!("{:?}", d.content_type),
            layer_mode: format!("{:?}", d.layer_mode),
            layers: d
                .layers
                .iter()
                .map(|l| bs_player_api::DescriptorLayer {
                    codec: String::from_utf8_lossy(&l.codec_tag).into_owned(),
                    width: l.width,
                    height: l.height,
                    fps: l.frame_rate_milli as f64 / 1000.0,
                    kbps: l.bitrate_kbps,
                    has_init: !l.init_data.is_empty(),
                    init: l.init_data.clone(),
                })
                .collect(),
        };
        let _ = xtx.send(Some(info));
    }

    /// Build the STREAM_DESCRIPTOR (App D §D.4.20) from the first chunk: ffmpeg
    /// renditions carry their fMP4 initialisation segment as an INIT record in the
    /// container; opaque sources describe themselves as OPAQUE / `data`.
    fn build_descriptor(&self, c: &LayeredChunk) -> bs_wire::frames::StreamDescriptor {
        use bs_wire::frames::{ContentType, LayerDescriptor, LayerMode, StreamDescriptor};
        let ladder = &self.cfg.ladder;
        let (content_type, layer_mode) = match &self.cfg.media {
            Media::Ffmpeg { .. } => (ContentType::VideoCmaf, LayerMode::Simulcast),
            _ => (ContentType::Opaque, LayerMode::Independent),
        };
        let (specs, fps) = match &self.cfg.media {
            Media::Ffmpeg {
                renditions, fps, ..
            } => (
                bs_ingest::RenditionSpec::parse_list(&renditions.join(",")).unwrap_or_default(),
                fps.unwrap_or(0) as f64,
            ),
            _ => (Vec::new(), 0.0),
        };
        let layers = c
            .layers
            .iter()
            .enumerate()
            .map(|(l, payload)| {
                // The first chunk of an ffmpeg rendition carries its INIT segment and a
                // META record `{"fps":..,"height":..}` written by the ingest.
                let (init, meta) = if content_type == ContentType::VideoCmaf {
                    let recs = bs_ingest::container::decode(payload);
                    let init = recs
                        .iter()
                        .find(|r| r.kind == bs_ingest::container::RecordType::Init)
                        .map(|r| r.data.clone())
                        .filter(|b| b.len() <= LayerDescriptor::MAX_INIT)
                        .unwrap_or_default();
                    let meta = recs
                        .iter()
                        .find(|r| r.kind == bs_ingest::container::RecordType::Meta)
                        .and_then(|r| serde_json::from_slice::<serde_json::Value>(&r.data).ok());
                    (init, meta)
                } else {
                    (Bytes::new(), None)
                };
                let spec = specs.get(l);
                let meta_fps = meta.as_ref().and_then(|m| m["fps"].as_f64()).unwrap_or(fps);
                let height = meta
                    .as_ref()
                    .and_then(|m| m["height"].as_u64())
                    .map(|h| h as u16)
                    .or(spec.map(|s| s.height as u16))
                    .unwrap_or(0);
                LayerDescriptor {
                    codec_tag: if content_type == ContentType::VideoCmaf {
                        *b"avc1"
                    } else {
                        *b"data"
                    },
                    width: (height as u32 * 16 / 9) as u16,
                    height,
                    frame_rate_milli: (meta_fps * 1000.0) as u32,
                    bitrate_kbps: ladder
                        .layers
                        .get(l)
                        .map(|x| x.bitrate_kbps.min(u16::MAX as u32) as u16)
                        .unwrap_or(0),
                    init_data: init,
                }
            })
            .collect();
        StreamDescriptor {
            stream_id: self.node.stream_id(),
            version: 1,
            effective_segment: c.segment,
            content_type,
            layer_mode,
            layers,
            signature: bs_wire::SignatureBytes::ZERO,
        }
    }

    /// Publisher-side integrity bookkeeping for a chunk about to be published.
    fn note_published(&mut self, c: &LayeredChunk) {
        if self.descriptor_version == 0 {
            let d = self.build_descriptor(c);
            self.descriptor_version = d.version;
            self.publish_descriptor(&d);
            self.feed(Input::Cmd(Command::SetDescriptor(d)));
        }
        for l in &c.layers {
            self.pub_hasher.update(l);
            self.pub_bytes += l.len() as u64;
        }
        self.checkpoint(c.segment.0, c.chunk_index, &c.layers);
        self.publish_status();
    }

    fn publish_tick(&mut self) {
        let chunk = self.source.as_mut().and_then(|s| s.next_chunk());
        match chunk {
            Some(c) => {
                self.chunks_published += 1;
                self.note_published(&c);
                self.feed(Input::Cmd(Command::PublishChunk(c)));
            }
            None => {
                info!(
                    "source exhausted after {} chunks; sending STREAM_END",
                    self.chunks_published
                );
                self.ended = true;
                self.feed(Input::Cmd(Command::EndStream));
            }
        }
    }

    fn on_plain(&mut self, p: PlainDatagram) {
        let frame = match Frame::from_slice(&p.data) {
            Ok(f) => f,
            Err(e) => {
                debug!("bad plain frame from {}: {e}", p.from);
                return;
            }
        };
        // Every plain frame, DHT RPCs included, is the core's (Ch2 lives in bs-core).
        self.feed(Input::Frame {
            from: p.from,
            channel: Channel::Udp,
            frame,
        });
    }

    fn on_ev(&mut self, ev: Ev) {
        match ev {
            Ev::DatagramSendError => self.datagram_send_errors += 1,
            Ev::Connected {
                peer,
                node_id,
                conn,
            } => {
                self.sessions += 1;
                // Gossip HMAC key (Ch3 §3.2.2) from the TLS exporter: identical on both ends.
                let mut session_key = [0u8; 32];
                if conn
                    .export_keying_material(&mut session_key, b"bitstream gossip v1", &[])
                    .is_err()
                {
                    warn!("keying material export failed for {peer}");
                }
                self.spawn_readers(peer, conn.clone());
                self.conns.insert(
                    peer,
                    Conn {
                        conn,
                        node_id,
                        writers: HashMap::new(),
                    },
                );
                self.feed(Input::SessionOpened {
                    peer,
                    node_id,
                    session_key,
                });
            }
            Ev::ConnectFailed { peer, err } => {
                debug!("connect to {peer} failed: {err}");
                self.feed(Input::SessionClosed { peer });
            }
            Ev::StreamFrame {
                peer,
                channel,
                frame,
            } => self.feed(Input::Frame {
                from: peer,
                channel,
                frame,
            }),
            Ev::Datagram { peer, frame } => self.feed(Input::Frame {
                from: peer,
                channel: Channel::Datagram,
                frame,
            }),
            Ev::Closed { peer } => {
                self.conns.remove(&peer);
                self.feed(Input::SessionClosed { peer });
            }
        }
    }

    fn spawn_readers(&self, peer: SocketAddr, conn: quinn::Connection) {
        // Streams.
        let tx = self.ev_tx.clone();
        let c = conn.clone();
        tokio::spawn(async move {
            while let Ok((_send, mut recv)) = c.accept_bi().await {
                let tx = tx.clone();
                tokio::spawn(async move {
                    let mut tag = [0u8; 1];
                    if recv.read_exact(&mut tag).await.is_err() {
                        return;
                    }
                    let channel = tag_channel(tag[0]);
                    let mut buf = BytesMut::with_capacity(64 * 1024);
                    loop {
                        let mut cur = &buf[..];
                        match Frame::decode(&mut cur) {
                            Ok(frame) => {
                                let consumed = buf.len() - cur.len();
                                buf.advance(consumed);
                                if tx
                                    .send(Ev::StreamFrame {
                                        peer,
                                        channel,
                                        frame,
                                    })
                                    .is_err()
                                {
                                    return;
                                }
                                continue;
                            }
                            Err(bs_wire::WireError::Truncated { .. }) => {}
                            Err(e) => {
                                debug!("stream decode error from {peer}: {e}");
                                return;
                            }
                        }
                        match recv.read_buf(&mut buf).await {
                            Ok(0) => return,
                            Ok(_) => {}
                            Err(_) => return,
                        }
                    }
                });
            }
        });
        // Datagrams.
        let tx = self.ev_tx.clone();
        let c = conn.clone();
        tokio::spawn(async move {
            while let Ok(d) = c.read_datagram().await {
                match Frame::from_slice(&d) {
                    Ok(frame) => {
                        if tx.send(Ev::Datagram { peer, frame }).is_err() {
                            return;
                        }
                    }
                    Err(e) => debug!("datagram decode error from {peer}: {e}"),
                }
            }
        });
        // Close.
        let tx = self.ev_tx.clone();
        tokio::spawn(async move {
            let _ = conn.closed().await;
            let _ = tx.send(Ev::Closed { peer });
        });
    }

    fn on_output(&mut self, out: Output) {
        match out {
            Output::Send {
                to,
                channel,
                frame,
                not_before,
            } => {
                *self.sent.entry(frame.frame_type()).or_insert(0) += 1;
                let delay = not_before
                    .map(|t| t.0.saturating_sub(self.now().0))
                    .unwrap_or(0);
                match channel {
                    Channel::Udp => self.send_plain(to, frame),
                    Channel::Datagram => {
                        let Some(c) = self.conns.get(&to) else { return };
                        let conn = c.conn.clone();
                        let bytes = match frame.to_bytes() {
                            Ok(b) => b,
                            Err(_) => return,
                        };
                        if delay == 0 {
                            if conn.send_datagram(bytes).is_err() {
                                self.datagram_send_errors += 1;
                            }
                        } else {
                            let tx = self.ev_tx.clone();
                            tokio::spawn(async move {
                                tokio::time::sleep(std::time::Duration::from_micros(delay)).await;
                                if conn.send_datagram(bytes).is_err() {
                                    let _ = tx.send(Ev::DatagramSendError);
                                }
                            });
                        }
                    }
                    Channel::Control | Channel::Tree(_) | Channel::Pull => {
                        let bytes = match frame.to_bytes() {
                            Ok(b) => b,
                            Err(_) => return,
                        };
                        let Some(w) = self.writer(to, channel) else {
                            return;
                        };
                        if delay == 0 {
                            let _ = w.send(bytes);
                        } else {
                            tokio::spawn(async move {
                                tokio::time::sleep(std::time::Duration::from_micros(delay)).await;
                                let _ = w.send(bytes);
                            });
                        }
                    }
                }
            }
            Output::OpenSession(peer) => {
                if self.conns.contains_key(&peer) {
                    return;
                }
                let ep = self.endpoint.clone();
                let tx = self.ev_tx.clone();
                let c1 = self.cfg.params.c1;
                tokio::spawn(async move {
                    let r: anyhow::Result<quinn::Connection> = async {
                        let conn = ep.connect(peer, tls::SERVER_NAME)?.await?;
                        Ok(conn)
                    }
                    .await;
                    match r {
                        Ok(conn) => match tls::connection_peer(&conn, c1) {
                            Ok(pid) => {
                                let _ = tx.send(Ev::Connected {
                                    peer,
                                    node_id: pid.node_id,
                                    conn,
                                });
                            }
                            Err(e) => {
                                let _ = tx.send(Ev::ConnectFailed {
                                    peer,
                                    err: e.to_string(),
                                });
                            }
                        },
                        Err(e) => {
                            let _ = tx.send(Ev::ConnectFailed {
                                peer,
                                err: e.to_string(),
                            });
                        }
                    }
                });
            }
            Output::CloseSession(peer) => {
                if let Some(c) = self.conns.remove(&peer) {
                    c.conn.close(0u32.into(), b"");
                }
            }
            Output::Discover { .. } | Output::Register(_) | Output::StoreRecord(_) => {
                // Oracle-mode outputs; the node runs the DHT in the core.
            }
            Output::Deliver {
                chunk,
                layers_complete,
                layers_subscribed,
            } => self.deliver(chunk, layers_complete, layers_subscribed),
            Output::Event(e) => self.on_event(e),
            Output::Descriptor(d) => {
                self.descriptor_version = d.version;
                self.publish_descriptor(&d);
                info!(
                    "stream descriptor v{} ({:?}, {:?}, {} layers)",
                    d.version,
                    d.content_type,
                    d.layer_mode,
                    d.layers.len()
                );
            }
        }
    }

    fn writer(&mut self, to: SocketAddr, channel: Channel) -> Option<mpsc::UnboundedSender<Bytes>> {
        let tag = channel_tag(channel);
        let c = self.conns.get_mut(&to)?;
        if let Some(w) = c.writers.get(&tag) {
            return Some(w.clone());
        }
        let (tx, mut rx) = mpsc::unbounded_channel::<Bytes>();
        let conn = c.conn.clone();
        tokio::spawn(async move {
            let Ok((mut send, _recv)) = conn.open_bi().await else {
                return;
            };
            if send.write_all(&[tag]).await.is_err() {
                return;
            }
            while let Some(b) = rx.recv().await {
                if send.write_all(&b).await.is_err() {
                    return;
                }
            }
            let _ = send.finish();
        });
        c.writers.insert(tag, tx.clone());
        Some(tx)
    }

    fn send_plain(&mut self, to: SocketAddr, frame: Frame) {
        match frame.to_bytes() {
            Ok(b) => {
                if let Err(e) = self.sock.send_plain(to, &b) {
                    debug!("udp send to {to} failed: {e}");
                }
            }
            Err(e) => warn!("encode failed: {e}"),
        }
    }

    fn deliver(&mut self, chunk: LayeredChunk, layers_complete: u8, layers_subscribed: u8) {
        self.layers_complete_last = layers_complete;
        if layers_complete >= 1 {
            let complete: Vec<Bytes> = chunk
                .layers
                .iter()
                .take(layers_complete as usize)
                .cloned()
                .collect();
            self.checkpoint(chunk.segment.0, chunk.chunk_index, &complete);
        }
        if let Some((tx, _, _)) = self.player.as_ref() {
            if layers_complete >= 1 {
                let _ = tx.send(bs_player_api::Delivered {
                    segment: chunk.segment.0,
                    chunk_index: chunk.chunk_index,
                    layers: chunk
                        .layers
                        .iter()
                        .take(layers_complete as usize)
                        .cloned()
                        .collect(),
                });
            }
            self.publish_status();
        }
        let full = layers_complete >= layers_subscribed.max(1);
        if !self.sink.started {
            if !full {
                return; // pre-anchor partials
            }
            self.sink.started = true;
            self.sink.first_segment = Some(chunk.segment.0);
        }
        if full {
            self.full += 1;
        }
        for l in chunk.layers.iter().take(layers_complete as usize) {
            self.sink.hasher.update(l);
            self.sink.bytes += l.len() as u64;
            if let Some(f) = self.sink.file.as_mut() {
                // Writes are small (≤ 190 KB per chunk); a brief blocking write is
                // cheaper than a spawn at this rate.
                use std::io::Write;
                if let Err(e) = f.write_all(l) {
                    warn!("output write failed: {e}");
                }
            }
        }
    }

    fn on_event(&mut self, e: Event) {
        if self.player.is_some()
            && matches!(
                e,
                Event::State { .. }
                    | Event::ParentAttached { .. }
                    | Event::ParentLost { .. }
                    | Event::ChunkPlayed { .. }
            )
        {
            self.publish_status_after(&e);
        }
        match &e {
            Event::State { from, to } => info!("state {from:?} -> {to:?}"),
            Event::ParentAttached {
                tree,
                depth,
                repair,
                ..
            } => info!("tree {tree}: parent attached at depth {depth} (repair={repair})"),
            Event::ParentLost { tree, reason, .. } => {
                self.parent_lost += 1;
                warn!("tree {tree}: parent lost ({reason})");
            }
            Event::RepairDone { tree, duration_us } => {
                self.repairs += 1;
                info!("tree {tree}: repaired in {:.3}s", *duration_us as f64 / 1e6);
            }
            Event::ChunkPlayed {
                segment,
                chunk,
                ok,
                layers_complete,
                layers_subscribed,
                ..
            } => {
                self.played += 1;
                if *ok {
                    self.ok += 1;
                }
                if self.first_chunk.is_some() {
                    self.steady_played += 1;
                    if !ok {
                        self.steady_starved += 1;
                        warn!("starved: segment {segment} chunk {chunk} ({layers_complete}/{layers_subscribed} layers)");
                    }
                }
            }
            Event::FirstChunk { segment, chunk } => {
                self.first_chunk = Some(self.epoch.elapsed().as_secs_f64());
                info!(
                    "first chunk: segment {segment} chunk {chunk} at {:.2}s",
                    self.first_chunk.unwrap()
                );
            }
            Event::ChildAdmitted {
                tree,
                children,
                k_v,
                ..
            } => info!("tree {tree}: child admitted ({children}/{k_v})"),
            Event::LayerShed { top_layer } => warn!("shed to layer {top_layer}"),
            Event::ReceiptIssued { .. } => self.incentives.receipts_issued += 1,
            Event::ReceiptAccepted { .. } => self.incentives.receipts_accepted += 1,
            Event::ReceiptRejected { reason } => {
                *self
                    .incentives
                    .receipts_rejected
                    .entry(reason.to_string())
                    .or_default() += 1;
            }
            Event::ReceiptMissed {
                tree, consecutive, ..
            } => {
                self.incentives.receipts_missed += 1;
                warn!("tree {tree}: child missed a receipt deadline ({consecutive} in a row)");
            }
            Event::ChokeChanged { state, .. } => {
                *self
                    .incentives
                    .choke_changes
                    .entry(state.to_string())
                    .or_default() += 1;
            }
            Event::PullChoked => self.incentives.pulls_choked += 1,
            Event::ChildRemoved { tree, reason, .. } => {
                info!("tree {tree}: child removed ({reason})");
            }
            Event::BlockRejected {
                tree,
                segment,
                block,
                ..
            } => warn!("rejected block {block} of segment {segment} in tree {tree}"),
            Event::ManifestRejected {
                segment,
                chunk,
                reason,
            } => debug!("manifest {segment}/{chunk} rejected: {reason}"),
            _ => {}
        }
    }

    async fn report(&mut self) -> RunReport {
        if let Some(f) = self.sink.file.as_mut() {
            use std::io::Write;
            let _ = f.flush();
        }
        let output_hash = if self.sink.started {
            Some(hex::encode(self.sink.hasher.finalize().as_bytes()))
        } else {
            None
        };
        let expected = match &self.cfg.media {
            Media::Output {
                expect_hash: Some(h),
                ..
            }
            | Media::Play {
                expect_hash: Some(h),
                ..
            } if !h.is_empty() => Some(h.to_lowercase()),
            _ => None,
        };
        let hash_match = match (&output_hash, &expected) {
            (Some(o), Some(e)) => Some(o == e),
            _ => None,
        };
        RunReport {
            role: if matches!(self.cfg.role, Role::Publisher { .. }) {
                "publisher".into()
            } else {
                "viewer".into()
            },
            node_id: self.node.node_id().to_string(),
            stream_id: self.node.stream_id().to_string(),
            listen: self.local_addr.to_string(),
            state: format!("{:?}", self.node.state()),
            seconds: self.epoch.elapsed().as_secs_f64(),
            chunks_published: self.chunks_published,
            chunks_played: self.played,
            chunks_ok: self.ok,
            chunks_full: self.full,
            steady_starved: self.steady_starved,
            steady_played: self.steady_played,
            first_chunk_s: self.first_chunk,
            first_segment: self.sink.first_segment,
            bytes_out: self.sink.bytes,
            output_hash,
            expected_hash: expected,
            hash_match,
            source_hash: self.source_hash.map(|h| h.to_string()),
            sent: self
                .sent
                .iter()
                .map(|(k, v)| (k.name().to_string(), *v))
                .collect(),
            guardian_peers: Some(self.node.dht_registrations()),
            parent_lost: self.parent_lost,
            repairs: self.repairs,
            sessions: self.sessions,
            connections: self
                .conns
                .iter()
                .map(|(peer, c)| {
                    let st = c.conn.stats();
                    ConnStats {
                        peer: peer.to_string(),
                        rtt_ms: st.path.rtt.as_secs_f64() * 1000.0,
                        cwnd: st.path.cwnd,
                        congestion_events: st.path.congestion_events,
                        sent_packets: st.path.sent_packets,
                        lost_packets: st.path.lost_packets,
                        mtu: st.path.current_mtu,
                        datagrams_tx: st.frame_tx.datagram,
                        datagrams_rx: st.frame_rx.datagram,
                    }
                })
                .collect(),
            datagram_send_errors: self.datagram_send_errors,
            incentives: {
                let mut i = self.incentives.clone();
                i.deadline_evictions = self.node.deadline_evictions();
                i.receipts_held = self.node.receipt_count();
                i.rank = self.node.rank();
                i
            },
        }
    }
}

/// Blake3 of a file, hex — the value `watch --expect-hash` wants.
pub fn file_hash(path: &std::path::Path, max_bytes: Option<usize>) -> anyhow::Result<String> {
    let mut data = std::fs::read(path)?;
    if let Some(n) = max_bytes {
        data.truncate(n);
    }
    Ok(hex::encode(blake3::hash(&data).as_bytes()))
}

impl Sink {
    #[allow(dead_code)]
    fn bytes(&self) -> u64 {
        self.bytes
    }
}

#[allow(dead_code)]
fn _unused(_: anyhow::Error) -> anyhow::Error {
    anyhow!("")
}
