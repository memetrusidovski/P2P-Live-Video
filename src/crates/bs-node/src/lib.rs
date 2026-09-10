//! BitStream real-network peer runtime: one `bs_core::Node` driven over UDP and
//! QUIC (quinn), with identity-bound TLS and a bootstrap guardian standing in for
//! the DHT until M3.

#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms)]

pub mod driver;
pub mod guardian;
pub mod tls;
pub mod transport;

pub use driver::{file_hash, start, Handle, Media, RunConfig, RunReport};
