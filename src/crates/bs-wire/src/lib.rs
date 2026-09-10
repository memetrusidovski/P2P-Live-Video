//! BitStream canonical wire format.
//!
//! Byte-exact implementation of the protocol's Appendix D frame registry and the
//! record layouts it references. **No I/O, no cryptography, no protocol logic** —
//! only encoding and decoding of what travels on the wire, and the constants that
//! fix their sizes.
//!
//! Every multi-byte integer is big-endian. Every frame starts with the 4-byte
//! [`FrameHeader`]. Pre-session UDP frames carry a 152-byte [`ValidationBlock`]
//! immediately after the header.
//!
//! Signatures are opaque bytes here; `bs-crypto` produces and verifies them.
//! Signed structures expose a `signable_bytes()` helper returning exactly the
//! bytes a signature covers, so the two crates cannot disagree about coverage.
//!
//! Frames whose layout is implemented decode into typed [`Frame`] variants; the
//! rest decode into [`Frame::Raw`]. Coverage is tracked in `docs/wire_coverage.md`.

#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms)]

pub mod codec;
pub mod consts;
pub mod error;
pub mod frame;
pub mod frame_type;
pub mod frames;
pub mod header;
pub mod records;
pub mod types;
pub mod validation;

pub use codec::{Decode, Encode};
pub use error::WireError;
pub use frame::Frame;
pub use frame_type::{FrameType, Transport};
pub use header::{FrameHeader, PROTOCOL_VERSION};
pub use records::{PeerRecord, SlicingMode, StreamRecord, TreeMappingEntry};
pub use types::*;
pub use validation::ValidationBlock;
