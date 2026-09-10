//! Error type for wire encoding and decoding.

use crate::frame_type::FrameType;

/// Errors produced while encoding or decoding wire structures.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WireError {
    /// The buffer ended before the structure was complete.
    #[error("truncated: needed {needed} more byte(s) while decoding {context}")]
    Truncated {
        /// Bytes still required.
        needed: usize,
        /// What was being decoded.
        context: &'static str,
    },
    /// The header declared a protocol version this implementation does not speak.
    #[error("unsupported protocol version {0:#04x}")]
    UnsupportedVersion(u8),
    /// The frame type code is not in the registry.
    #[error("unknown frame type {0:#04x}")]
    UnknownFrameType(u8),
    /// A field held a value outside its legal range.
    #[error("invalid value {value} for field {field}")]
    InvalidField {
        /// Field name.
        field: &'static str,
        /// Offending value.
        value: u64,
    },
    /// The payload was longer than the layout allows.
    #[error("payload of {len} bytes exceeds the maximum {max} for {frame:?}")]
    PayloadTooLarge {
        /// Frame being encoded.
        frame: FrameType,
        /// Actual length.
        len: usize,
        /// Maximum allowed.
        max: usize,
    },
    /// Trailing bytes remained after a fixed-size layout was consumed.
    #[error("{0} trailing byte(s) after a fixed-size layout")]
    TrailingBytes(usize),
    /// A length-prefixed count disagreed with the bytes present.
    #[error("count mismatch in {context}: declared {declared}, present {present}")]
    CountMismatch {
        /// What was being decoded.
        context: &'static str,
        /// Declared count.
        declared: usize,
        /// Count actually present.
        present: usize,
    },
}

/// Convenience alias.
pub type Result<T> = core::result::Result<T, WireError>;
