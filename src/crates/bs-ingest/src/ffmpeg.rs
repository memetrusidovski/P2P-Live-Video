//! One ffmpeg process that encodes every simulcast rendition, fragment-aligned,
//! combined into `LayeredChunk`s (layer `l` = rendition `l`).

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use anyhow::{anyhow, Context};
use bs_media::{Ladder, Layer, LayeredChunk};
use bs_wire::consts::CHUNKS_PER_SEGMENT;
use bs_wire::SegmentSeq;
use bytes::Bytes;
use tokio::io::AsyncReadExt;
use tokio::process::{Child, Command};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use crate::container::{encode, Record, RecordType};
use crate::mp4::{FragmentAssembler, Unit};

/// A rendition: `"480p:1500"` = 480 lines high at 1500 kbps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenditionSpec {
    /// Label (e.g. "480p").
    pub label: String,
    /// Frame height; width follows the aspect ratio.
    pub height: u32,
    /// Video bitrate, kbps.
    pub kbps: u32,
}

impl std::str::FromStr for RenditionSpec {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (h, k) = s
            .split_once(':')
            .ok_or_else(|| anyhow!("rendition `{s}`: expected <height>p:<kbps>"))?;
        let height: u32 = h
            .trim_end_matches('p')
            .parse()
            .with_context(|| format!("rendition `{s}`: height"))?;
        let kbps: u32 = k
            .parse()
            .with_context(|| format!("rendition `{s}`: kbps"))?;
        Ok(Self {
            label: format!("{height}p"),
            height,
            kbps,
        })
    }
}

impl RenditionSpec {
    /// Parse a comma-separated list.
    pub fn parse_list(s: &str) -> anyhow::Result<Vec<Self>> {
        s.split(',')
            .filter(|x| !x.trim().is_empty())
            .map(|x| x.trim().parse())
            .collect()
    }
    /// Ladder for a list of renditions (lowest first as given).
    pub fn ladder(list: &[Self]) -> Ladder {
        Ladder {
            layers: list
                .iter()
                .map(|r| Layer {
                    label: r.label.clone(),
                    bitrate_kbps: r.kbps,
                })
                .collect(),
        }
    }
}

/// Chunk period.
const CHUNK_US: u64 = 250_000;
/// A rendition this many fragments behind the leader is skipped for the chunk.
const MAX_LAG: usize = 8;

enum Ev {
    Init(usize, Bytes),
    Media(usize, Bytes),
    Ended(usize),
}

struct Rendition {
    init: Option<Bytes>,
    init_sent: bool,
    queue: VecDeque<Bytes>,
    ended: bool,
    skipped: u64,
}

/// The live simulcast source.
pub struct FfmpegSource {
    ladder: Ladder,
    rx: mpsc::Receiver<Ev>,
    rend: Vec<Rendition>,
    k: u64,
    children: Vec<Child>,
    /// Fragments emitted per rendition.
    pub emitted: Vec<u64>,
    /// Frame rate used for every rendition.
    fps: u32,
    /// The rendition specs, in layer order.
    specs: Vec<RenditionSpec>,
}

/// Detect the frame rate of `input` with ffprobe.
pub async fn probe_fps(input: &Path) -> anyhow::Result<u32> {
    let out = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=r_frame_rate",
            "-of",
            "csv=p=0",
        ])
        .arg(input)
        .output()
        .await
        .context("running ffprobe")?;
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let (n, d) = s.split_once('/').unwrap_or((&s, "1"));
    let n: f64 = n.parse().with_context(|| format!("ffprobe fps `{s}`"))?;
    let d: f64 = d.parse().unwrap_or(1.0);
    Ok((n / d.max(1.0)).round().max(1.0) as u32)
}

/// How ffmpeg reads the input.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InputOptions {
    /// ffmpeg arguments placed before `-i`, such as `-f avfoundation` for a camera or
    /// `-listen 1` for an RTMP listener.
    pub args: Vec<String>,
    /// The input produces frames in real time (a camera, a screen, a network feed).
    /// ffmpeg then reads as fast as frames arrive. A file is read at its frame rate.
    pub live: bool,
}

/// Frame rate assumed for a live input when none is given: a device cannot be probed.
const LIVE_FPS: u32 = 30;

impl FfmpegSource {
    /// Spawn ffmpeg on a file. `fps` = None probes the input.
    pub async fn spawn(
        input: PathBuf,
        renditions: Vec<RenditionSpec>,
        fps: Option<u32>,
    ) -> anyhow::Result<Self> {
        Self::spawn_with(input, renditions, fps, InputOptions::default()).await
    }

    /// Spawn one ffmpeg process that decodes the input once and encodes every
    /// rendition.
    ///
    /// WHY: One process per rendition opens the input once per rendition. A camera or
    ///      a listening socket can be opened once only, and separate processes drift
    ///      apart in time.
    ///
    /// Args:
    ///     input: File, device or URL, as ffmpeg's `-i` takes it.
    ///     renditions: Renditions in layer order, lowest first.
    ///     fps: Frame rate. `None` probes a file and assumes `LIVE_FPS` for a live input.
    ///     options: How ffmpeg reads the input.
    pub async fn spawn_with(
        input: PathBuf,
        renditions: Vec<RenditionSpec>,
        fps: Option<u32>,
        options: InputOptions,
    ) -> anyhow::Result<Self> {
        if renditions.is_empty() {
            anyhow::bail!("at least one rendition");
        }
        let fps = match fps {
            Some(f) => f,
            None if options.live => LIVE_FPS,
            None => probe_fps(&input).await?,
        };
        info!(
            "ffmpeg ingest: {} rendition(s) at {fps} fps from {}",
            renditions.len(),
            input.display()
        );
        // Each rendition leaves ffmpeg on its own loopback TCP connection.
        let mut listeners = Vec::with_capacity(renditions.len());
        for _ in &renditions {
            listeners.push(
                tokio::net::TcpListener::bind("127.0.0.1:0")
                    .await
                    .context("binding a loopback port for ffmpeg output")?,
            );
        }
        let mut cmd = Command::new("ffmpeg");
        cmd.args(["-hide_banner", "-loglevel", "error"]);
        if !options.live {
            cmd.arg("-re");
        }
        cmd.args(&options.args).arg("-i").arg(&input);
        for (r, l) in renditions.iter().zip(&listeners) {
            // Every rendition carries its own AAC audio track, so switching
            // renditions never changes the set of tracks (MSE type
            // `video/mp4; codecs="avc1.…,mp4a.40.2"`).
            cmd.args(["-map", "0:v:0", "-map", "0:a:0?"])
                .args(["-c:a", "aac", "-b:a", "96k", "-ac", "2"])
                .args([
                    "-c:v",
                    "libx264",
                    "-preset",
                    "veryfast",
                    "-tune",
                    "zerolatency",
                ])
                .args([
                    "-b:v",
                    &format!("{}k", r.kbps),
                    "-maxrate",
                    &format!("{}k", r.kbps),
                    "-bufsize",
                    &format!("{}k", r.kbps * 2),
                ])
                .args(["-vf", &format!("scale=-2:{}", r.height)])
                .args([
                    "-g",
                    &fps.to_string(),
                    "-keyint_min",
                    &fps.to_string(),
                    "-sc_threshold",
                    "0",
                ])
                .args(["-force_key_frames", "expr:gte(t,n_forced*1)"])
                .args([
                    "-f",
                    "mp4",
                    "-movflags",
                    "frag_keyframe+empty_moov+default_base_moof+omit_tfhd_offset",
                ])
                // RULE: No `-min_frag_duration`. With it ffmpeg does not cut at a
                //       keyframe, and the keyframe lands in the middle of a fragment.
                //       A player can then never switch renditions at a fragment start.
                .args(["-frag_duration", "250000"])
                .arg(format!("tcp://{}", l.local_addr()?));
        }
        cmd.stdout(Stdio::null())
            .stdin(Stdio::null())
            .kill_on_drop(true);
        let mut child = cmd.spawn().context("spawning ffmpeg")?;
        let (tx, rx) = mpsc::channel::<Ev>(256);
        for (i, listener) in listeners.into_iter().enumerate() {
            // ffmpeg opens its outputs in order. A process that dies first never connects.
            let stream = tokio::select! {
                r = listener.accept() => r.context("ffmpeg output connection")?.0,
                status = child.wait() => {
                    anyhow::bail!("ffmpeg ended before it opened its outputs: {}", status?)
                }
            };
            let tx = tx.clone();
            tokio::spawn(async move {
                let mut stream = stream;
                let mut asm = FragmentAssembler::new();
                let mut buf = vec![0u8; 64 * 1024];
                loop {
                    match stream.read(&mut buf).await {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            for u in asm.feed(&buf[..n]) {
                                let ev = match u {
                                    Unit::Init(b) => Ev::Init(i, b),
                                    Unit::Media(b) => Ev::Media(i, b),
                                };
                                if tx.send(ev).await.is_err() {
                                    return;
                                }
                            }
                        }
                    }
                }
                let _ = tx.send(Ev::Ended(i)).await;
            });
        }
        let children = vec![child];
        let n = renditions.len();
        Ok(Self {
            ladder: RenditionSpec::ladder(&renditions),
            fps,
            specs: renditions.clone(),
            rx,
            rend: (0..n)
                .map(|_| Rendition {
                    init: None,
                    init_sent: false,
                    queue: VecDeque::new(),
                    ended: false,
                    skipped: 0,
                })
                .collect(),
            k: 0,
            children,
            emitted: vec![0; n],
        })
    }

    /// Why the ingest ended, if an ffmpeg process failed. Call it after
    /// `next_chunk` returned `None`.
    ///
    /// Returns `None` if every ffmpeg process exited with success: the input ended.
    pub async fn failure(&mut self) -> Option<String> {
        let mut failed = Vec::new();
        for child in self.children.iter_mut() {
            match child.wait().await {
                Ok(status) if status.success() => {}
                Ok(status) => failed.push(format!("ffmpeg ended with {status}")),
                Err(e) => failed.push(format!("ffmpeg: {e}")),
            }
        }
        if failed.is_empty() {
            None
        } else {
            Some(failed.join("; "))
        }
    }

    /// The ladder.
    pub fn ladder(&self) -> &Ladder {
        &self.ladder
    }

    fn ready(&self) -> bool {
        let max_q = self.rend.iter().map(|r| r.queue.len()).max().unwrap_or(0);
        self.rend
            .iter()
            .all(|r| !r.queue.is_empty() || r.ended || max_q > MAX_LAG)
            && max_q > 0
    }

    fn all_done(&self) -> bool {
        self.rend.iter().all(|r| r.ended && r.queue.is_empty())
    }

    /// Next chunk, or `None` when every rendition has ended.
    pub async fn next_chunk(&mut self) -> Option<LayeredChunk> {
        loop {
            if self.ready() {
                return Some(self.build());
            }
            if self.all_done() {
                return None;
            }
            match self.rx.recv().await {
                Some(Ev::Init(i, b)) => {
                    debug!("rendition {i}: init segment ({} bytes)", b.len());
                    self.rend[i].init = Some(b);
                }
                Some(Ev::Media(i, b)) => self.rend[i].queue.push_back(b),
                Some(Ev::Ended(i)) => {
                    info!("rendition {i}: ffmpeg ended");
                    self.rend[i].ended = true;
                }
                None => {
                    for r in &mut self.rend {
                        r.ended = true;
                    }
                }
            }
        }
    }

    fn build(&mut self) -> LayeredChunk {
        let k = self.k;
        self.k += 1;
        let segment = SegmentSeq((k / CHUNKS_PER_SEGMENT as u64) as u32 + 1);
        let chunk_index = (k % CHUNKS_PER_SEGMENT as u64) as u8;
        let mut layers = Vec::with_capacity(self.rend.len());
        for (i, r) in self.rend.iter_mut().enumerate() {
            let mut recs = Vec::new();
            match r.queue.pop_front() {
                Some(media) => {
                    // The initialisation segment rides the first chunk only; from then
                    // on the STREAM_DESCRIPTOR (App D §D.4.20) carries it to joiners.
                    if !r.init_sent {
                        if let Some(init) = &r.init {
                            recs.push(Record::new(RecordType::Init, init.clone()));
                            let meta = format!(
                                r#"{{"fps":{},"height":{},"kbps":{},"label":"{}"}}"#,
                                self.fps,
                                self.specs[i].height,
                                self.specs[i].kbps,
                                self.specs[i].label
                            );
                            recs.push(Record::new(RecordType::Meta, meta.into_bytes()));
                            r.init_sent = true;
                        }
                    }
                    recs.push(Record::new(RecordType::Media, media));
                    self.emitted[i] += 1;
                }
                None => {
                    if !r.ended {
                        r.skipped += 1;
                        warn!("rendition {i} lagging; empty layer in chunk {k}");
                    }
                }
            }
            layers.push(encode(&recs));
        }
        LayeredChunk {
            segment,
            chunk_index,
            timestamp_us: k * CHUNK_US,
            layers,
        }
    }
}
