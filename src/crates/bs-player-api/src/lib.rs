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
use axum::response::{Html, IntoResponse, Response};
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
    /// Last failure that needs the user, such as a bootstrap that does not answer or
    /// an ingest process that did not start. Cleared on recovery.
    #[serde(default)]
    pub last_error: Option<String>,
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
        let (stx, srx) = watch::channel(Status::default());
        let (state, dtx, xtx) = Self::with_status(srx);
        (state, dtx, stx, xtx)
    }

    /// Create the delivery and descriptor channels around a status channel the
    /// node already owns. Returns the state plus the node-side senders.
    pub fn with_status(
        status: watch::Receiver<Status>,
    ) -> (
        Arc<Self>,
        broadcast::Sender<Delivered>,
        watch::Sender<Option<DescriptorInfo>>,
    ) {
        let (dtx, _) = broadcast::channel(64);
        let (xtx, xrx) = watch::channel(None);
        (
            Arc::new(Self {
                deliveries: dtx.clone(),
                status,
                descriptor: xrx,
            }),
            dtx,
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
        .layer(axum::middleware::from_fn(same_origin_only))
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

/// Refuse a request that another web site made, or that came through a foreign name.
///
/// SECURITY: The API streams the media and the node's status to whoever asks. Any
///           page open in the viewer's browser can reach a loopback port. A browser
///           names the asking page in `Origin`, so a request whose `Origin` differs
///           from its `Host` came from another site. A `Host` that is a public DNS
///           name is refused too. A hostile site can point its own name at this
///           address (DNS rebinding), and its requests then pass the first check.
async fn same_origin_only(req: axum::extract::Request, next: axum::middleware::Next) -> Response {
    let host = header_lower(&req, axum::http::header::HOST);
    let origin = header_lower(&req, axum::http::header::ORIGIN);
    if request_allowed(host.as_deref(), origin.as_deref()) {
        next.run(req).await
    } else {
        tracing::warn!("player api: refused a request from origin {origin:?} for host {host:?}");
        axum::http::StatusCode::FORBIDDEN.into_response()
    }
}

fn header_lower(req: &axum::extract::Request, name: axum::http::HeaderName) -> Option<String> {
    req.headers()
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(str::to_ascii_lowercase)
}

/// Whether a request with these lower-case `Host` and `Origin` headers is served.
///
/// A request with no `Origin` comes from a tool or a same-origin page load. With an
/// `Origin`, the origin must name the same host and port as `Host`.
fn request_allowed(host: Option<&str>, origin: Option<&str>) -> bool {
    let Some(host) = host else {
        // HTTP/1.1 requires `Host`; a request without one is not from a browser.
        return origin.is_none();
    };
    if !local_host_name(host) {
        return false;
    }
    match origin {
        None => true,
        Some(o) => o
            .strip_prefix("http://")
            .or_else(|| o.strip_prefix("https://"))
            .is_some_and(|authority| authority == host),
    }
}

/// Whether a `Host` header value is an address or a name that only the local
/// network resolves. Such a name is `localhost`, a single label, or ends in `.local`.
fn local_host_name(host: &str) -> bool {
    if host.parse::<SocketAddr>().is_ok() || host.parse::<std::net::IpAddr>().is_ok() {
        return true;
    }
    if host.starts_with('[') {
        // A bracketed IPv6 literal with no port.
        return host
            .trim_start_matches('[')
            .trim_end_matches(']')
            .parse::<std::net::Ipv6Addr>()
            .is_ok();
    }
    let name = host.rsplit_once(':').map_or(host, |(n, port)| {
        if port.chars().all(|c| c.is_ascii_digit()) {
            n
        } else {
            host
        }
    });
    name == "localhost" || !name.contains('.') || name.ends_with(".local")
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

    #[test]
    fn only_the_page_itself_and_tools_are_served() {
        // Tools and same-origin page loads send no Origin.
        assert!(request_allowed(Some("127.0.0.1:8080"), None));
        assert!(request_allowed(Some("10.0.1.212:8079"), None));
        // The player page, over loopback, a LAN address, a LAN name or IPv6.
        assert!(request_allowed(
            Some("127.0.0.1:8080"),
            Some("http://127.0.0.1:8080")
        ));
        assert!(request_allowed(
            Some("localhost:8080"),
            Some("http://localhost:8080")
        ));
        assert!(request_allowed(
            Some("desk.local:8080"),
            Some("http://desk.local:8080")
        ));
        assert!(request_allowed(
            Some("[::1]:8080"),
            Some("http://[::1]:8080")
        ));
        // Another site open in the viewer's browser.
        assert!(!request_allowed(
            Some("127.0.0.1:8080"),
            Some("https://evil.example")
        ));
        assert!(!request_allowed(
            Some("127.0.0.1:8080"),
            Some("http://127.0.0.1:9999")
        ));
        assert!(!request_allowed(Some("127.0.0.1:8080"), Some("null")));
        // DNS rebinding: the hostile site's own name points at this address.
        assert!(!request_allowed(
            Some("evil.example:8080"),
            Some("http://evil.example:8080")
        ));
        assert!(!request_allowed(Some("evil.example:8080"), None));
    }

    #[tokio::test]
    async fn a_foreign_origin_gets_403() {
        let (state, _dtx, _stx, _xtx) = PlayerState::new();
        let (addr, _task) = serve("127.0.0.1:0".parse().unwrap(), state).await.unwrap();
        let get = move |origin: Option<String>| {
            tokio::task::spawn_blocking(move || {
                use std::io::{Read, Write};
                let mut s = std::net::TcpStream::connect(addr).unwrap();
                let origin = origin.map_or(String::new(), |o| format!("Origin: {o}\r\n"));
                write!(
                    s,
                    "GET /api/status HTTP/1.1\r\nHost: {addr}\r\n{origin}Connection: close\r\n\r\n"
                )
                .unwrap();
                let mut out = String::new();
                s.read_to_string(&mut out).unwrap();
                out
            })
        };
        let own = get(Some(format!("http://{addr}"))).await.unwrap();
        assert!(own.starts_with("HTTP/1.1 200"), "{own}");
        let foreign = get(Some("https://evil.example".into())).await.unwrap();
        assert!(foreign.starts_with("HTTP/1.1 403"), "{foreign}");
    }
}
