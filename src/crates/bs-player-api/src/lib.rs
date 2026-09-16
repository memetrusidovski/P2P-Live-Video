//! Local player API: serves the web client and hands it the reassembled media
//! over a WebSocket. Wire format to the page (see `clients/web/README.md`):
//! text `{"type":"hello","layers":N}` on connect, then binary frames
//! `[kind u8][layer u8][segment u32 BE][chunk u8][payload]` (kind 1 = init,
//! 2 = media) and a JSON status text frame every second.

#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms)]

use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::{Html, IntoResponse};
use axum::routing::get;
use axum::{Json, Router};
use bs_ingest::container::{decode, RecordType};
use bytes::{BufMut, Bytes, BytesMut};
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, watch};
use tracing::{debug, info};

/// A chunk delivered by the node: complete layers only, lowest first.
#[derive(Debug, Clone)]
pub struct Delivered {
    /// Segment.
    pub segment: u32,
    /// Chunk within the segment.
    pub chunk_index: u8,
    /// Layer payloads (TLV records inside).
    pub layers: Vec<Bytes>,
}

/// Per-chunk data-integrity checkpoint: Blake3 of each complete layer's bytes,
/// hex. Identical on the publisher and on every viewer that received the chunk,
/// so a dashboard can prove viewers hold the source's bytes chunk by chunk.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Checkpoint {
    /// Segment.
    pub segment: u32,
    /// Chunk.
    pub chunk: u8,
    /// Layer hashes (leading complete layers only).
    pub layer_hashes: Vec<String>,
}

/// Node status shown by the page.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Status {
    /// "publisher" or "viewer".
    #[serde(default)]
    pub role: String,
    /// NodeID (hex).
    #[serde(default)]
    pub node_id: String,
    /// Lifecycle state.
    pub state: String,
    /// Layers in the stream.
    pub layers: u8,
    /// Complete layers in the last delivered chunk.
    pub layers_complete_last: u8,
    /// Depth per tree (255 = no parent).
    pub depth: Vec<u8>,
    /// Chunks played.
    pub played: u64,
    /// Chunks starved.
    pub starved: u64,
    /// Chunks with every subscribed layer.
    pub chunks_full: u64,
    /// Seconds to the first chunk.
    pub first_chunk_s: Option<f64>,
    /// Chunks with the base layer complete.
    #[serde(default)]
    pub chunks_ok: u64,
    /// Blocks that failed Merkle verification.
    #[serde(default)]
    pub blocks_rejected: u64,
    /// Parent losses.
    #[serde(default)]
    pub parent_lost: u64,
    /// Bytes delivered to the application (viewer) or published (publisher).
    #[serde(default)]
    pub bytes: u64,
    /// Running Blake3 over every delivered/published byte in order, hex.
    #[serde(default)]
    pub hash_so_far: Option<String>,
    /// Blake3 of the whole source file, when known (publisher, file mode).
    #[serde(default)]
    pub source_hash: Option<String>,
    /// Version of the STREAM_DESCRIPTOR held (0 = none).
    #[serde(default)]
    pub descriptor_version: u32,
    /// Recent per-chunk checkpoints (newest last, bounded).
    #[serde(default)]
    pub checkpoints: Vec<Checkpoint>,
}

/// One layer of the STREAM_DESCRIPTOR as the page sees it.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DescriptorLayer {
    /// FourCC, e.g. "avc1".
    pub codec: String,
    /// Width.
    pub width: u16,
    /// Height.
    pub height: u16,
    /// Frames per second.
    pub fps: f64,
    /// Nominal bitrate.
    pub kbps: u16,
    /// Initialisation segment (sent to the page as a kind-1 frame, not in JSON).
    #[serde(skip)]
    pub init: Bytes,
    /// Whether initialisation data is present.
    pub has_init: bool,
}

/// The STREAM_DESCRIPTOR in force (App D §D.4.20), decoupled from wire types.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DescriptorInfo {
    /// Version.
    pub version: u32,
    /// "VideoCmaf", "Opaque", ...
    pub content_type: String,
    /// "Simulcast", "SvcTemporal", ...
    pub layer_mode: String,
    /// Layers, lowest first.
    pub layers: Vec<DescriptorLayer>,
}

/// Shared state between the node driver and the HTTP server.
pub struct PlayerState {
    /// Deliveries fan out to every connected socket.
    pub deliveries: broadcast::Sender<Delivered>,
    /// Latest status.
    pub status: watch::Receiver<Status>,
    /// The descriptor in force, if any.
    pub descriptor: watch::Receiver<Option<DescriptorInfo>>,
}

impl PlayerState {
    /// Create the channels. Returns the state plus the node-side senders.
    pub fn new() -> (
        Arc<Self>,
        broadcast::Sender<Delivered>,
        watch::Sender<Status>,
        watch::Sender<Option<DescriptorInfo>>,
    ) {
        let (dtx, _) = broadcast::channel(64);
        let (stx, srx) = watch::channel(Status::default());
        let (xtx, xrx) = watch::channel(None);
        (
            Arc::new(Self {
                deliveries: dtx.clone(),
                status: srx,
                descriptor: xrx,
            }),
            dtx,
            stx,
            xtx,
        )
    }
}

/// Bind and serve. Returns the bound address and the server task.
pub async fn serve(
    addr: SocketAddr,
    state: Arc<PlayerState>,
) -> anyhow::Result<(SocketAddr, tokio::task::JoinHandle<()>)> {
    let app = Router::new()
        .route("/", get(index))
        .route("/api/status", get(status))
        .route("/api/descriptor", get(descriptor))
        .route("/ws", get(ws_upgrade))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let bound = listener.local_addr()?;
    info!("player api on http://{bound}/");
    let task = tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            tracing::warn!("player api stopped: {e}");
        }
    });
    Ok((bound, task))
}

async fn index() -> Html<&'static str> {
    Html(include_str!("../../../clients/web/index.html"))
}

async fn status(State(s): State<Arc<PlayerState>>) -> Json<Status> {
    Json(s.status.borrow().clone())
}

async fn descriptor(State(s): State<Arc<PlayerState>>) -> Json<Option<DescriptorInfo>> {
    Json(s.descriptor.borrow().clone())
}

/// Send the descriptor as a text frame plus one kind-1 (init) frame per layer.
async fn send_descriptor(sock: &mut WebSocket, d: &DescriptorInfo) -> bool {
    let txt = serde_json::json!({"type": "descriptor", "descriptor": d}).to_string();
    if sock.send(Message::Text(txt.into())).await.is_err() {
        return false;
    }
    for (l, layer) in d.layers.iter().enumerate() {
        if !layer.init.is_empty() {
            let f = frame(1, l as u8, 0, 0, &layer.init);
            if sock.send(Message::Binary(f)).await.is_err() {
                return false;
            }
        }
    }
    true
}

async fn ws_upgrade(ws: WebSocketUpgrade, State(s): State<Arc<PlayerState>>) -> impl IntoResponse {
    ws.on_upgrade(move |sock| ws_session(sock, s))
}

/// Encode one binary frame for the page.
pub fn frame(kind: u8, layer: u8, segment: u32, chunk: u8, payload: &[u8]) -> Bytes {
    let mut b = BytesMut::with_capacity(7 + payload.len());
    b.put_u8(kind);
    b.put_u8(layer);
    b.put_u32(segment);
    b.put_u8(chunk);
    b.put_slice(payload);
    b.freeze()
}

async fn ws_session(mut sock: WebSocket, s: Arc<PlayerState>) {
    let mut rx = s.deliveries.subscribe();
    let mut status_rx = s.status.clone();
    let layers = status_rx.borrow().layers;
    let hello = serde_json::json!({"type": "hello", "layers": layers}).to_string();
    if sock.send(Message::Text(hello.into())).await.is_err() {
        return;
    }
    let mut desc_rx = s.descriptor.clone();
    let current = desc_rx.borrow_and_update().clone();
    if let Some(d) = current {
        if !send_descriptor(&mut sock, &d).await {
            return;
        }
    }
    let mut tick = tokio::time::interval(std::time::Duration::from_secs(1));
    loop {
        tokio::select! {
            r = desc_rx.changed() => {
                if r.is_err() { return; }
                let d = desc_rx.borrow_and_update().clone();
                if let Some(d) = d {
                    if !send_descriptor(&mut sock, &d).await { return; }
                }
            }
            d = rx.recv() => {
                match d {
                    Ok(d) => {
                        for (l, payload) in d.layers.iter().enumerate() {
                            for rec in decode(payload) {
                                let kind = match rec.kind {
                                    RecordType::Init => 1,
                                    RecordType::Media => 2,
                                    RecordType::Meta => continue,
                                };
                                let f = frame(kind, l as u8, d.segment, d.chunk_index, &rec.data);
                                if sock.send(Message::Binary(f)).await.is_err() {
                                    return;
                                }
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => debug!("player socket lagged {n} chunks"),
                    Err(broadcast::error::RecvError::Closed) => return,
                }
            }
            _ = tick.tick() => {
                let st = status_rx.borrow_and_update().clone();
                let txt = serde_json::to_string(&st).unwrap_or_default();
                if sock.send(Message::Text(txt.into())).await.is_err() {
                    return;
                }
            }
            m = sock.recv() => {
                match m {
                    Some(Ok(Message::Close(_))) | None | Some(Err(_)) => return,
                    _ => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_layout() {
        let f = frame(2, 1, 0x01020304, 3, b"xyz");
        assert_eq!(&f[..], &[2, 1, 1, 2, 3, 4, 3, b'x', b'y', b'z']);
    }

    #[tokio::test]
    async fn status_endpoint_serves_json_and_page() {
        let (state, _dtx, stx, _xtx) = PlayerState::new();
        stx.send_modify(|s| {
            s.state = "Active".into();
            s.layers = 2;
        });
        let (addr, _task) = serve("127.0.0.1:0".parse().unwrap(), state).await.unwrap();
        let body = tokio::task::spawn_blocking(move || {
            use std::io::{Read, Write};
            let mut s = std::net::TcpStream::connect(addr).unwrap();
            write!(
                s,
                "GET /api/status HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n"
            )
            .unwrap();
            let mut out = String::new();
            s.read_to_string(&mut out).unwrap();
            out
        })
        .await
        .unwrap();
        assert!(body.contains("\"state\":\"Active\""), "{body}");
        assert!(body.contains("\"layers\":2"));
    }
}
