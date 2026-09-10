//! Typed payload layouts, one module per frame family. Each type implements
//! [`Encode`](crate::Encode)/[`Decode`](crate::Decode) for its **payload only**
//! (no header); [`crate::Frame`] adds the header.

pub mod control;
pub mod media;
pub mod presession;

pub use control::*;
pub use media::*;
pub use presession::*;
