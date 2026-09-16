//! One ffmpeg process per simulcast rendition, fragment-aligned, combined into
//! `LayeredChunk`s (layer `l` = rendition `l`).

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
    _children: Vec<Child>,
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

impl FfmpegSource {
    /// Spawn one ffmpeg per rendition. `fps` = None probes the input.
    pub async fn spawn(
        input: PathBuf,
        renditions: Vec<RenditionSpec>,
        fps: Option<u32>,
    ) -> anyhow::Result<Self> {
        if renditions.is_empty() {
            anyhow::bail!("at least one rendition");
        }
        let fps = match fps {
            Some(f) => f,
            None => probe_fps(&input).await?,
        };
        info!(
            "ffmpeg ingest: {} rendition(s) at {fps} fps from {}",
            renditions.len(),
            input.display()
        );
        let (tx, rx) = mpsc::channel::<Ev>(256);
        let mut children = Vec::new();
        for (i, r) in renditions.iter().enumerate() {
            let mut cmd = Command::new("ffmpeg");
            cmd.args(["-hide_banner", "-loglevel", "error", "-re", "-i"])
                .arg(&input)
                // Every rendition carries its own AAC audio track, so switching
                // renditions never changes the set of tracks (MSE type
                // `video/mp4; codecs="avc1.…,mp4a.40.2"`).
                .args([
                    "-c:a",
                    "aac",
                    "-b:a",
                    "96k",
                    "-ac",
                    "2",
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
                .args([
                    "-frag_duration",
                    "250000",
                    "-min_frag_duration",
                    "250000",
                    "pipe:1",
                ])
                .stdout(Stdio::piped())
                .stdin(Stdio::null())
                .kill_on_drop(true);
            let mut child = cmd
                .spawn()
                .with_context(|| format!("spawning ffmpeg for {}", r.label))?;
            let mut stdout = child.stdout.take().ok_or_else(|| anyhow!("no stdout"))?;
            let tx = tx.clone();
            tokio::spawn(async move {
                let mut asm = FragmentAssembler::new();
                let mut buf = vec![0u8; 64 * 1024];
                loop {
                    match stdout.read(&mut buf).await {
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
            children.push(child);
        }
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
            _children: children,
            emitted: vec![0; n],
        })
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
