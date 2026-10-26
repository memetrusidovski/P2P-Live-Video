//! BitStream real-network peer runtime: one `bs_core::Node` driven over UDP and
//! QUIC (quinn), with identity-bound TLS; discovery runs on the core's S/Kademlia DHT, seeded by
//! the DHT until M3.

#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms)]

pub mod driver;
pub mod tls;
pub mod transport;

pub use bs_player_api::Status;
pub use driver::{detect_ip, file_hash, start, Handle, Media, RunConfig, RunReport, PUBLISH_PORT};
