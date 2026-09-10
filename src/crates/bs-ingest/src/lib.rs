//! Real-media ingest for the publisher.
//!
//! * [`container`] — the application-level TLV framing inside each layer payload
//!   (INIT / MEDIA / META records). The protocol below it carries opaque bytes.
//! * [`mp4`] — a minimal ISO BMFF box splitter for ffmpeg's fragmented output.
//! * [`ffmpeg`] — [`FfmpegSource`]: one ffmpeg process per simulcast rendition,
//!   fragment-aligned, yielding one [`bs_media::LayeredChunk`] per 250 ms.
//!
//! Interim rule until ISSUE-060 lands a stream descriptor: every rendition's INIT
//! record is repeated in chunk 0 of every segment so a late joiner can start
//! within one second; players cache INITs per layer.

#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms)]

pub mod container;
pub mod ffmpeg;
pub mod mp4;

pub use container::{Record, RecordType};
pub use ffmpeg::{FfmpegSource, RenditionSpec};
