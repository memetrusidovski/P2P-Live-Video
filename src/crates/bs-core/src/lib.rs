//! BitStream peer state machine — sans-I/O.
//!
//! A [`Node`] owns everything a peer knows and decides everything a peer does,
//! but touches no socket, clock or thread. Drivers (the simulator in `bs-sim`,
//! the real runtime in `bs-node`) feed it [`Input`]s with the current
//! [`Instant`] and drain [`Output`]s. Time and randomness are injected.
//!
//! Module map (see `ARCHITECTURE.md` §2.4):
//! * [`params`] — every protocol constant, by Appendix B name.
//! * [`io`] — the Input/Output contract, commands and events.
//! * [`forest`] — per-tree parent/children state, depth, slots.
//! * [`join`] — the per-tree parent-selection state machine.
//! * [`swarm`] — manifests, block reassembly, verify-then-forward, playout.
//! * [`publisher`] — the source's chunk emission.
//! * [`node`] — the orchestrator.

#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms)]

pub mod forest;
pub mod io;
pub mod join;
pub mod node;
pub mod params;
pub mod publisher;
pub mod swarm;
pub mod time;

pub use io::{Channel, Command, Event, Input, Output, PeerAddr};
pub use node::{Node, NodeConfig, Role};
pub use params::Params;
pub use time::{Duration, Instant};

/// Node-level lifecycle state (Ch1 §1.3.1). `CHURN_REPAIR` is per tree and lives
/// in [`forest::TreeState::repairing`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[repr(u8)]
pub enum Lifecycle {
    /// Instantiating keys, validating the NodeID.
    Bootstrap = 0x00,
    /// Querying discovery for the stream.
    Discovery = 0x01,
    /// HyParView handshakes (M3; M1 passes straight through).
    Joining = 0x02,
    /// Acquiring a parent in every subscribed tree.
    Connecting = 0x03,
    /// Streaming.
    Active = 0x04,
    /// Shut down.
    Terminated = 0x06,
}
