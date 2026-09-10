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
use bs_crypto::Signer;
use bs_media::source::Source;
use bs_media::{FileSource, Ladder, LayeredChunk, SyntheticSource};
use bs_wire::frames::{GetPeers, GetPeersFrame, RegisterPeer};
use bs_wire::{Frame, FrameType, NodeClass, NodeId, StreamId, TreeSet, WireAddr};
use bytes::{Buf, Bytes, BytesMut};
use serde::{Deserialize, Serialize};
use tokio::io::AsyncReadExt;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use crate::guardian::Guardian;
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
    /// Peers registered at the guardian (publisher).
    pub guardian_peers: Option<usize>,
    /// Parent losses.
    pub parent_lost: u64,
    /// Repairs.
    pub repairs: u64,
    /// Sessions accepted/opened.
    pub sessions: u64,
}

#[derive(Debug)]
enum Ev {
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
        _ => 0xFF,
    }
}
fn tag_channel(t: u8) -> Channel {
    if t == 0 {
        Channel::Control
    } else {
        Channel::Tree(bs_wire::TreeId(t))
    }
}

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
    identity: bs_crypto::Identity,
    sock: Arc<DemuxSocket>,
    endpoint: quinn::Endpoint,
    plain_rx: mpsc::UnboundedReceiver<PlainDatagram>,
    ev_tx: mpsc::UnboundedSender<Ev>,
    ev_rx: mpsc::UnboundedReceiver<Ev>,
    conns: HashMap<SocketAddr, Conn>,
    epoch: tokio::time::Instant,
    guardian: Option<Guardian>,
    source: Option<Box<dyn Source + Send>>,
    source_hash: Option<bs_wire::Hash>,
    sink: Sink,
    dyn_nonce: u64,
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
    ended: bool,
    end_at: Option<tokio::time::Instant>,
    /// Live chunks from ffmpeg (publisher, `Media::Ffmpeg`).
    live_rx: Option<mpsc::Receiver<LayeredChunk>>,
    /// Player API senders (viewer, `Media::Play`).
    player: Option<(
        tokio::sync::broadcast::Sender<bs_player_api::Delivered>,
        tokio::sync::watch::Sender<bs_player_api::Status>,
    )>,
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
        let guardian = if is_pub {
            Some(Guardian::new(
                node.stream_id(),
                node.node_id(),
                difficulty,
                cfg.params.tau_ttl.as_micros(),
                cfg.seed,
            ))
        } else {
            None
        };
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
        let c2 = difficulty.dynamic_required(2, cfg.node_class, false);
        let dyn_nonce = identity.solve_dynamic(node.addr().ip(), c2);
        Ok(Self {
            cfg,
            node,
            identity,
            sock,
            endpoint,
            plain_rx,
            ev_tx,
            ev_rx,
            conns: HashMap::new(),
            epoch: tokio::time::Instant::now(),
            guardian,
            source,
            source_hash,
            sink,
            dyn_nonce,
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
            repairs: 0,
            sessions: 0,
            ended: false,
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
        let mut register_tick = tokio::time::interval(std::time::Duration::from_micros(
            self.cfg.params.tau_ttl.as_micros() / 2,
        ));

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
                            self.feed(Input::Cmd(Command::PublishChunk(c)));
                        }
                        None => {
                            info!("ffmpeg ingest ended after {} chunks; sending STREAM_END", self.chunks_published);
                            self.ended = true;
                            self.feed(Input::Cmd(Command::EndStream));
                        }
                    }
                }
                _ = register_tick.tick(), if !is_pub => {
                    let rec = self.node.peer_record();
                    self.register(rec);
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
            Media::Play { addr, .. } => {
                let (state, dtx, stx) = bs_player_api::PlayerState::new();
                let layers = self
                    .node
                    .matrix()
                    .map(|m| m.layer_count)
                    .unwrap_or(self.cfg.ladder.len() as u8);
                stx.send_modify(|s| s.layers = layers);
                let (bound, _task) = bs_player_api::serve(addr, state).await?;
                info!("player: open http://{bound}/");
                self.player = Some((dtx, stx));
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

    fn publish_status(&mut self) {
        let Some((_, stx)) = self.player.as_ref() else {
            return;
        };
        let depth: Vec<u8> = self.node.trees().iter().map(|t| t.depth).collect();
        let st = bs_player_api::Status {
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

    fn publish_tick(&mut self) {
        let chunk = self.source.as_mut().and_then(|s| s.next_chunk());
        match chunk {
            Some(c) => {
                self.chunks_published += 1;
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
        // Guardian frames are handled here, never by the core.
        match &frame {
            Frame::RegisterPeer(r) => {
                let now = self.now().as_micros();
                if let Some(g) = self.guardian.as_mut() {
                    if !g.handle_register(p.from, r, now) {
                        debug!("rejected REGISTER_PEER from {}", p.from);
                    }
                }
                return;
            }
            Frame::GetPeers(GetPeersFrame::Request(g)) => {
                let now = self.now().as_micros();
                if let Some(gd) = self.guardian.as_mut() {
                    if let Some(resp) = gd.handle_get_peers(p.from, g, now) {
                        self.send_plain(p.from, Frame::GetPeers(GetPeersFrame::Response(resp)));
                    }
                }
                return;
            }
            Frame::GetPeers(GetPeersFrame::Response(r)) => {
                self.feed(Input::Cmd(Command::Discovered {
                    records: r.records.clone(),
                    stream_record: r.stream_record.clone(),
                }));
                return;
            }
            _ => {}
        }
        self.feed(Input::Frame {
            from: p.from,
            channel: Channel::Udp,
            frame,
        });
    }

    fn on_ev(&mut self, ev: Ev) {
        match ev {
            Ev::Connected {
                peer,
                node_id,
                conn,
            } => {
                self.sessions += 1;
                self.spawn_readers(peer, conn.clone());
                self.conns.insert(
                    peer,
                    Conn {
                        conn,
                        node_id,
                        writers: HashMap::new(),
                    },
                );
                self.feed(Input::SessionOpened { peer, node_id });
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
                            let _ = conn.send_datagram(bytes);
                        } else {
                            tokio::spawn(async move {
                                tokio::time::sleep(std::time::Duration::from_micros(delay)).await;
                                let _ = conn.send_datagram(bytes);
                            });
                        }
                    }
                    Channel::Control | Channel::Tree(_) => {
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
            Output::Discover {
                wanted_trees,
                starved_trees,
            } => self.discover(wanted_trees, starved_trees),
            Output::Register(rec) => self.register(rec),
            Output::StoreRecord(sr) => {
                if let Some(g) = self.guardian.as_mut() {
                    g.stream_record = Some(sr);
                }
            }
            Output::Deliver {
                chunk,
                layers_complete,
                layers_subscribed,
            } => self.deliver(chunk, layers_complete, layers_subscribed),
            Output::Event(e) => self.on_event(e),
            Output::Descriptor(d) => info!(
                "stream descriptor v{} ({:?}, {:?}, {} layers)",
                d.version,
                d.content_type,
                d.layer_mode,
                d.layers.len()
            ),
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

    fn validation_block(&self, ty: FrameType, body: &[u8]) -> bs_wire::ValidationBlock {
        self.identity
            .validation_block(ty, self.dyn_nonce, self.now().as_micros(), body)
    }

    fn discover(&mut self, wanted: TreeSet, starved: TreeSet) {
        let Some(b) = self.cfg.bootstrap else { return };
        let sid = self.node.stream_id();
        let mut body = Vec::with_capacity(36);
        body.extend_from_slice(sid.as_bytes());
        body.push(starved.0);
        body.push(wanted.0);
        body.push(0);
        body.push(0);
        let vb = self.validation_block(FrameType::GET_PEERS, &body);
        let f = GetPeers {
            validation: vb,
            stream_id: sid,
            starved_trees: starved,
            wanted_trees: wanted,
            want_relay_capable: false,
        };
        self.send_plain(b, Frame::GetPeers(GetPeersFrame::Request(f)));
    }

    fn register(&mut self, rec: bs_wire::PeerRecord) {
        let now = self.now().as_micros();
        if let Some(g) = self.guardian.as_mut() {
            g.register_local(rec, now);
            return;
        }
        let Some(b) = self.cfg.bootstrap else { return };
        let sid = self.node.stream_id();
        let port = self.local_addr.port();
        let ts = now;
        let msg = RegisterPeer::signable_bytes(
            &sid,
            &rec.node_id,
            port,
            rec.node_class,
            rec.assigned_trees,
            rec.flags,
            ts,
        );
        let signature = self.identity.sign(&msg);
        let mut body = Vec::with_capacity(72);
        body.extend_from_slice(&port.to_be_bytes());
        body.push(RegisterPeer::PROTOCOL_UDP);
        body.push(rec.node_class as u8);
        body.push(rec.assigned_trees.0);
        body.push(rec.flags.to_byte());
        body.extend_from_slice(&[0, 0]);
        body.extend_from_slice(signature.as_bytes());
        let vb =
            self.identity
                .validation_block(FrameType::REGISTER_PEER, self.dyn_nonce, ts, &body);
        let f = RegisterPeer {
            stream_id: sid,
            validation: vb,
            port,
            node_class: rec.node_class,
            assigned_trees: rec.assigned_trees,
            flags: rec.flags,
            signature,
        };
        let _ = WireAddr(self.local_addr);
        self.send_plain(b, Frame::RegisterPeer(f));
    }

    fn deliver(&mut self, chunk: LayeredChunk, layers_complete: u8, layers_subscribed: u8) {
        self.layers_complete_last = layers_complete;
        if let Some((tx, _)) = self.player.as_ref() {
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
            role: if self.guardian.is_some() {
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
            guardian_peers: self.guardian.as_ref().map(|g| g.len()),
            parent_lost: self.parent_lost,
            repairs: self.repairs,
            sessions: self.sessions,
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
