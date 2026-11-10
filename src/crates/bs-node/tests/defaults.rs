//! The `RunConfig` constructors carry the specification proof-of-work difficulty, so a
//! default publisher and a default viewer agree on the puzzle and an identity costs
//! real work on the Internet.

use std::net::SocketAddr;

use bs_core::Params;
use bs_media::Ladder;
use bs_node::{Media, RunConfig};
use bs_wire::StreamId;

#[test]
fn publisher_defaults_to_spec_pow() {
    let cfg = RunConfig::publisher(Media::Synthetic { seed: 1 }, Ladder::reference());
    let spec = Params::default();
    assert_eq!((cfg.params.c1, cfg.params.c2_min), (spec.c1, spec.c2_min));
}

#[test]
fn viewer_defaults_to_spec_pow() {
    let bootstrap: SocketAddr = "127.0.0.1:4000".parse().unwrap();
    let cfg = RunConfig::viewer(bootstrap, StreamId([0; 32]));
    let spec = Params::default();
    assert_eq!((cfg.params.c1, cfg.params.c2_min), (spec.c1, spec.c2_min));
}
