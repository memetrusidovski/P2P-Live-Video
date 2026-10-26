//! The one running node: the Tauri commands and the managed state behind them.

use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bs_media::Ladder;
use bs_node::{Media, RunConfig, RunReport};
use serde::Serialize;
use tauri::State;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::link::ShareLink;

/// Renditions a video publisher encodes when the window leaves the field empty.
/// Two layers keep a laptop encoder within budget; the viewer's ladder follows.
const DEFAULT_RENDITIONS: &str = "360p:800,720p:2500";
const STOP_TIMEOUT: Duration = Duration::from_secs(10);

/// How a publisher turns the file into a stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Ingest {
    /// Transcode with ffmpeg into simulcast renditions the player can show.
    Video,
    /// Send the bytes as they are. Proves transport; the player shows no picture.
    Raw,
}

/// What the window shows about the node.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Status {
    /// Whether the node task is still alive.
    pub running: bool,
    /// "publisher", "viewer", or empty when no node was started.
    pub role: String,
    /// NodeID, hex.
    pub node_id: String,
    /// Stream id, hex.
    pub stream_id: String,
    /// UDP address the node bound.
    pub local_addr: String,
    /// Where the node serves the web player.
    pub player_url: String,
    /// Link to hand to viewers.
    pub share_link: Option<String>,
    /// Node lifecycle state, or "failed" or "stopped" once the task ended.
    pub state: String,
    /// Chunks played (viewer) or published (publisher).
    pub played: u64,
    /// Chunks starved.
    pub starved: u64,
    /// Layers in the stream.
    pub layers: u8,
    /// Seconds to the first chunk.
    pub first_chunk_s: Option<f64>,
    /// Last problem, from the node or from the app.
    pub error: Option<String>,
}

struct Running {
    role: &'static str,
    node_id: String,
    stream_id: String,
    local_addr: SocketAddr,
    player: SocketAddr,
    share_link: String,
    status: tokio::sync::watch::Receiver<bs_node::Status>,
    stop: tokio::sync::watch::Sender<bool>,
    /// `None` once the task finished and `outcome` holds its result.
    done: Option<JoinHandle<anyhow::Result<RunReport>>>,
    outcome: Option<Result<RunReport, String>>,
}

/// Tauri managed state: the current node, if one was started.
#[derive(Default)]
pub struct AppState(Mutex<Option<Running>>);

/// Join the stream named by a share link and serve the player for it.
#[tauri::command]
pub async fn start_viewer(
    state: State<'_, AppState>,
    link: String,
    upload_kbps: u32,
) -> Result<Status, String> {
    viewer(&state, &link, upload_kbps).await
}

/// Stream a file and serve the preview page for it.
#[tauri::command]
pub async fn start_publisher(
    state: State<'_, AppState>,
    path: String,
    upload_kbps: u32,
    ingest: Ingest,
    renditions: String,
) -> Result<Status, String> {
    publisher(&state, &path, upload_kbps, ingest, &renditions).await
}

/// Ask the node to stop and wait for its final report.
#[tauri::command]
pub async fn stop(state: State<'_, AppState>) -> Result<Status, String> {
    stop_node(&state).await
}

/// Current state of the node with its live counters.
#[tauri::command]
pub async fn status(state: State<'_, AppState>) -> Result<Status, String> {
    Ok(current(&state).await)
}

async fn viewer(state: &AppState, link: &str, upload_kbps: u32) -> Result<Status, String> {
    let parsed = ShareLink::parse(link)?;
    let bootstrap = tokio::net::lookup_host(&parsed.bootstrap)
        .await
        .map_err(|e| format!("resolve {}: {e}", parsed.bootstrap))?
        .next()
        .ok_or_else(|| format!("{} resolves to no address", parsed.bootstrap))?;
    let mut cfg = RunConfig::viewer(bootstrap, parsed.stream_id);
    cfg.upload_kbps = upload_kbps;
    cfg.seed = seed_now();
    cfg.player = Some(SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 0));
    launch(state, cfg, "viewer", Some(link.trim().to_string())).await
}

async fn publisher(
    state: &AppState,
    path: &str,
    upload_kbps: u32,
    ingest: Ingest,
    renditions: &str,
) -> Result<Status, String> {
    let path = PathBuf::from(path.trim());
    if !path.is_file() {
        return Err(format!("{} is not a file", path.display()));
    }
    let (media, ladder) = match ingest {
        Ingest::Video => {
            if which_ffmpeg().is_none() {
                return Err("ffmpeg is not on PATH; install it or publish raw bytes".into());
            }
            let renditions = if renditions.trim().is_empty() {
                DEFAULT_RENDITIONS
            } else {
                renditions.trim()
            };
            let specs = bs_ingest::RenditionSpec::parse_list(renditions)
                .map_err(|e| format!("renditions: {e:#}"))?;
            if specs.is_empty() {
                return Err("at least one rendition is needed, like 720p:2500".into());
            }
            (
                Media::Ffmpeg {
                    input: path,
                    renditions: specs
                        .iter()
                        .map(|r| format!("{}:{}", r.label, r.kbps))
                        .collect(),
                    fps: None,
                },
                bs_ingest::RenditionSpec::ladder(&specs),
            )
        }
        Ingest::Raw => (
            Media::File {
                path,
                repeat: false,
                max_bytes: None,
            },
            Ladder::reference(),
        ),
    };
    let mut cfg = RunConfig::publisher(media, ladder);
    cfg.upload_kbps = upload_kbps;
    cfg.seed = seed_now();
    cfg.player = Some(SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 0));
    launch(state, cfg, "publisher", None).await
}

async fn stop_node(state: &AppState) -> Result<Status, String> {
    let mut slot = state.0.lock().await;
    let Some(running) = slot.as_mut() else {
        return Err("no node is running".into());
    };
    let _ = running.stop.send(true);
    if let Some(mut done) = running.done.take() {
        let outcome = match tokio::time::timeout(STOP_TIMEOUT, &mut done).await {
            Ok(joined) => report_of(joined),
            Err(_) => {
                done.abort();
                Err(format!("the node did not stop within {STOP_TIMEOUT:?}"))
            }
        };
        running.outcome = Some(outcome);
    }
    Ok(snapshot(running).await)
}

async fn current(state: &AppState) -> Status {
    let mut slot = state.0.lock().await;
    match slot.as_mut() {
        None => Status::default(),
        Some(running) => snapshot(running).await,
    }
}

async fn launch(
    state: &AppState,
    cfg: RunConfig,
    role: &'static str,
    share_link: Option<String>,
) -> Result<Status, String> {
    let mut slot = state.0.lock().await;
    let busy = slot
        .as_ref()
        .is_some_and(|r| r.done.as_ref().is_some_and(|d| !d.is_finished()));
    if busy {
        return Err("a node is already running; stop it first".into());
    }
    let handle = bs_node::start(cfg)
        .await
        .map_err(|e| format!("start {role}: {e:#}"))?;
    let player = handle
        .player_addr
        .ok_or("the node did not bind a player API")?;
    // Behind a NAT the advertised address is the LAN one. The user swaps in the
    // public address before the link leaves the network.
    let share_link =
        share_link.unwrap_or_else(|| ShareLink::format(handle.advertise_addr, handle.stream_id));
    let running = slot.insert(Running {
        role,
        node_id: handle.node_id.to_string(),
        stream_id: handle.stream_id.to_string(),
        local_addr: handle.local_addr,
        player,
        share_link,
        status: handle.status,
        stop: handle.stop,
        done: Some(handle.done),
        outcome: None,
    });
    Ok(snapshot(running).await)
}

async fn snapshot(running: &mut Running) -> Status {
    if running.done.as_ref().is_some_and(|d| d.is_finished()) {
        let done = running.done.take().expect("checked above");
        running.outcome = Some(report_of(done.await));
    }
    let mut status = Status {
        running: running.done.is_some(),
        role: running.role.to_string(),
        node_id: running.node_id.clone(),
        stream_id: running.stream_id.clone(),
        local_addr: running.local_addr.to_string(),
        player_url: format!("http://{}/", running.player),
        share_link: Some(running.share_link.clone()),
        ..Status::default()
    };
    match &running.outcome {
        Some(Ok(report)) => {
            status.state = report.state.clone();
            status.played = if running.role == "publisher" {
                report.chunks_published
            } else {
                report.chunks_played
            };
            status.starved = report.steady_starved;
            status.first_chunk_s = report.first_chunk_s;
            status.layers = running.status.borrow().layers;
        }
        Some(Err(e)) => {
            status.state = "failed".into();
            status.error = Some(e.clone());
        }
        None => {
            let live = running.status.borrow().clone();
            status.state = live.state;
            status.played = live.played;
            status.starved = live.starved;
            status.layers = live.layers;
            status.first_chunk_s = live.first_chunk_s;
            status.error = live.last_error;
        }
    }
    status
}

fn report_of(
    joined: Result<anyhow::Result<RunReport>, tokio::task::JoinError>,
) -> Result<RunReport, String> {
    match joined {
        Ok(Ok(report)) => Ok(report),
        Ok(Err(e)) => Err(format!("{e:#}")),
        Err(e) => Err(format!("the node task ended abnormally: {e}")),
    }
}

/// A fresh identity seed per run so two app instances never share a NodeID.
fn seed_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or_default()
}

/// The ffmpeg binary on PATH, if any.
fn which_ffmpeg() -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join("ffmpeg"))
        .find(|p| p.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every publisher binds `bs_node::PUBLISH_PORT`, so the end-to-end tests run
    /// one at a time.
    static PUBLISHER_PORT: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("bs-desktop-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Poll the viewer until it played a chunk, or fail with its last status.
    async fn wait_for_playback(state: &AppState) -> Status {
        for _ in 0..40 {
            tokio::time::sleep(Duration::from_secs(1)).await;
            let s = current(state).await;
            assert!(s.running, "viewer ended early: {s:?}");
            if s.played > 0 {
                return s;
            }
        }
        panic!("viewer played nothing: {:?}", current(state).await);
    }

    /// The whole command path over localhost with raw bytes. Publish a file, hand
    /// the share link to a viewer, read live counters, stop both nodes.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn publisher_link_feeds_a_viewer() {
        let _port = PUBLISHER_PORT.lock().await;
        let file = temp_dir().join("source.bin");
        // 1.5 MB at the 6 Mbps reference ladder is 2 s of stream.
        let bytes: Vec<u8> = (0..1_500_000u32)
            .map(|i| (i.wrapping_mul(2654435761) >> 13) as u8)
            .collect();
        std::fs::write(&file, bytes).unwrap();
        let path = file.to_str().unwrap();

        let pub_state = AppState::default();
        let s = publisher(&pub_state, path, 100_000, Ingest::Raw, "")
            .await
            .unwrap();
        assert_eq!(s.role, "publisher");
        assert!(s.running);
        assert!(s.player_url.starts_with("http://127.0.0.1:"), "{s:?}");
        let link = s.share_link.clone().unwrap();
        assert!(link.starts_with(crate::link::SCHEME), "{link}");
        assert!(publisher(&pub_state, path, 100_000, Ingest::Raw, "")
            .await
            .is_err());

        let view_state = AppState::default();
        let s = viewer(&view_state, &link, 20_000).await.unwrap();
        assert_eq!(s.role, "viewer");
        assert_eq!(s.share_link.as_deref(), Some(link.as_str()));

        let view_live = wait_for_playback(&view_state).await;
        assert_eq!(view_live.layers, 3, "{view_live:?}");
        assert!(view_live.error.is_none(), "{view_live:?}");
        // The file is 2 s of stream, so the publisher ended before this point.
        // The counters come from its final report.
        let pub_done = current(&pub_state).await;
        assert!(pub_done.error.is_none(), "{pub_done:?}");
        assert_eq!(pub_done.state, "Terminated", "{pub_done:?}");
        assert_eq!(pub_done.played, 8, "{pub_done:?}");

        let s = stop_node(&view_state).await.unwrap();
        assert!(!s.running);
        assert!(s.error.is_none(), "{s:?}");
        assert!(stop_node(&pub_state).await.unwrap().error.is_none());
        assert!(stop_node(&view_state).await.unwrap().error.is_none());
        assert!(!current(&view_state).await.running);
    }

    /// A video file goes through ffmpeg and reaches the viewer as two renditions
    /// with a descriptor. Skipped where ffmpeg is not installed.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn video_publisher_feeds_a_viewer() {
        if which_ffmpeg().is_none() {
            eprintln!("ffmpeg not on PATH; skipping");
            return;
        }
        let _port = PUBLISHER_PORT.lock().await;
        let clip = temp_dir().join("clip.mp4");
        let made = std::process::Command::new("ffmpeg")
            .args(["-hide_banner", "-loglevel", "error", "-y"])
            .args([
                "-f",
                "lavfi",
                "-i",
                "testsrc2=size=640x360:rate=30",
                "-t",
                "6",
            ])
            .args([
                "-c:v", "libx264", "-preset", "veryfast", "-g", "30", "-pix_fmt", "yuv420p",
            ])
            .arg(&clip)
            .status()
            .unwrap();
        assert!(made.success(), "ffmpeg could not write the test clip");

        let pub_state = AppState::default();
        let s = publisher(
            &pub_state,
            clip.to_str().unwrap(),
            100_000,
            Ingest::Video,
            "",
        )
        .await
        .unwrap();
        let link = s.share_link.clone().unwrap();
        let view_state = AppState::default();
        viewer(&view_state, &link, 20_000).await.unwrap();

        let view_live = wait_for_playback(&view_state).await;
        assert_eq!(view_live.layers, 2, "{view_live:?}");
        assert!(view_live.error.is_none(), "{view_live:?}");
        let slot = view_state.0.lock().await;
        let descriptor = slot.as_ref().unwrap().status.borrow().descriptor_version;
        drop(slot);
        assert!(descriptor > 0, "the viewer holds no stream descriptor");

        assert!(stop_node(&view_state).await.unwrap().error.is_none());
        assert!(stop_node(&pub_state).await.unwrap().error.is_none());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn bad_inputs_are_named() {
        let state = AppState::default();
        let e = publisher(&state, "/nonexistent/file", 1000, Ingest::Raw, "")
            .await
            .unwrap_err();
        assert!(e.contains("is not a file"), "{e}");
        let e = viewer(&state, "http://x", 1000).await.unwrap_err();
        assert!(e.contains("bs://"), "{e}");
        assert!(stop_node(&state).await.is_err());
        assert!(!current(&state).await.running);
    }
}
