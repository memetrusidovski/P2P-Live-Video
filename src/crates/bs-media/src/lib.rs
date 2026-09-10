//! BitStream media pipeline — pure, payload-agnostic.
//!
//! A **chunk** is 250 ms of stream: an ordered list of *layer* byte-strings
//! ([`LayeredChunk`]). This crate turns chunks into 16 KB Merkle blocks with a
//! signed [`Manifest`](bs_wire::frames::Manifest) ([`chunk`]), decides which tree
//! carries which block ([`mapping`]), encodes blocks into RaptorQ symbols and
//! back ([`fec`]), produces chunks from synthetic data or a file ([`source`]),
//! and reassembles them on the consumer side with a running hash that lets a
//! viewer prove it received exactly what the publisher sent ([`sink`]).
//!
//! Nothing here knows what a codec is. Video-specific ingest lives in `bs-ingest`.

#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms)]

pub mod chunk;
pub mod fec;
pub mod ladder;
pub mod mapping;
pub mod sink;
pub mod source;

pub use chunk::{BuiltChunk, ChunkBuilder, LayeredChunk};
pub use fec::{FecDecoder, FecEncoder, SymbolCollector};
pub use ladder::{Ladder, Layer};
pub use mapping::{SlicingMatrix, TreeInfo};
pub use sink::ChunkAssembler;
pub use source::{FileSource, Source, SyntheticSource};

/// Errors from the media pipeline.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MediaError {
    /// A layer exceeded the per-chunk block limit.
    #[error("chunk has {blocks} blocks; the BlockIndex layout allows 4096")]
    TooManyBlocks {
        /// Blocks requested.
        blocks: usize,
    },
    /// Chunk described layers the ladder does not have.
    #[error("chunk has {got} layers but the ladder has {expected}")]
    LayerCountMismatch {
        /// Layers in the chunk.
        got: usize,
        /// Layers in the ladder.
        expected: usize,
    },
    /// A block failed Merkle verification.
    #[error("block {block} of segment {segment} failed verification")]
    BadBlock {
        /// Segment.
        segment: u32,
        /// Block index.
        block: u16,
    },
    /// A manifest signature did not verify.
    #[error("manifest signature invalid")]
    BadManifestSignature,
    /// RaptorQ could not reconstruct the block from the symbols given.
    #[error("insufficient symbols to decode block")]
    Undecodable,
    /// A symbol had the wrong size.
    #[error("symbol of {0} bytes; expected 1024")]
    BadSymbolSize(usize),
    /// Mapping could not be built.
    #[error("invalid mapping: {0}")]
    Mapping(&'static str),
}
