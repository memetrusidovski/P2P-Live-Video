//! Core state-machine tests. Each test names the specification section it
//! enforces. They drive `Node`s through the in-test harness in `common/`.

mod common;

use bs_core::{Channel, Command, Duration, Event, Input, Instant, Lifecycle};
use bs_crypto::blake3_hash;
use bs_wire::frames::*;
use bs_wire::{
    DisconnectReason, Frame, FrameType, NodeClass, NodeId, SegmentSeq, SignatureBytes, TreeId,
    TreeSet,
};
use common::*;

fn secs(s: f64) -> Duration {
    Duration::from_secs_f64(s)
}

/// Publisher + one relay viewer subscribing to all three layers, attached in
/// every tree. Publisher has ample capacity.
fn pub_and_viewer(h: &mut Harness) -> (std::net::SocketAddr, std::net::SocketAddr) {
    let p = addr(1);
    let v = addr(2);
    h.add_publisher(p, 100_000, 1);
    h.add_viewer(v, NodeClass::Relay, 2, 10_000, 7);
    h.start(p);
    h.start_publishing();
    h.run_for(secs(0.5));
    h.start(v);
    h.run_for(secs(2.0));
    (p, v)
}

fn sends_of(
    h: &Harness,
    a: std::net::SocketAddr,
    ty: FrameType,
) -> Vec<(Instant, std::net::SocketAddr, Frame, Option<Instant>)> {
    h.sends(a)
        .into_iter()
        .filter(|(_, _, _, f, _)| f.frame_type() == ty)
        .map(|(t, to, _, f, nb)| (t, to, f, nb))
        .collect()
}

/// Ch4 §4.1 / Ch3 §3.3.2 / Ch2 §2.3.3: the publisher registers as a relay of
/// every tree with the SOURCE flag, stores a Stream Record, goes ACTIVE, sends no
/// media without children, and with a child sends MANIFEST before any
/// BLOCK_PROOF of that chunk and paces symbols within half a chunk period.
#[test]
fn publisher_start_and_pacing() {
    let mut h = Harness::new();
    let p = addr(1);
    h.add_publisher(p, 100_000, 1);
    h.start(p);
    let reg = h
        .items(p)
        .iter()
        .find_map(|(_, i)| {
            if let TraceItem::Register(r) = i {
                Some(*r)
            } else {
                None
            }
        })
        .expect("registered");
    assert!(reg.flags.source);
    assert_eq!(reg.assigned_trees, TreeSet::first_n(3));
    assert!(h
        .items(p)
        .iter()
        .any(|(_, i)| matches!(i, TraceItem::StoreRecord)));
    assert_eq!(h.node(p).state(), Lifecycle::Active);
    h.start_publishing();
    h.run_for(secs(1.0));
    assert!(
        sends_of(&h, p, FrameType::RAPTORQ_SYMBOL).is_empty(),
        "no children, no media"
    );
    assert!(sends_of(&h, p, FrameType::BLOCK_PROOF).is_empty());

    let v = addr(2);
    h.add_viewer(v, NodeClass::Relay, 2, 10_000, 7);
    h.start(v);
    h.run_for(secs(2.0));
    let items = h.items(p);
    // MANIFEST for (seg, chunk) to v precedes the first BLOCK_PROOF of that chunk to v.
    let mut checked = 0;
    for (k, (_, it)) in items.iter().enumerate() {
        if let TraceItem::Send {
            to,
            frame: Frame::BlockProof(bp),
            ..
        } = it
        {
            if *to != v {
                continue;
            }
            let has_manifest = items[..k].iter().any(|(_, j)| matches!(j, TraceItem::Send { to: t2, frame: Frame::Manifest(m), .. } if *t2 == v && m.body.segment == bp.segment && m.body.chunk_index == bp.block.chunk()));
            assert!(
                has_manifest,
                "BLOCK_PROOF {:?} {:?} before its MANIFEST",
                bp.segment, bp.block
            );
            checked += 1;
        }
    }
    assert!(
        checked > 10,
        "expected many forwarded blocks, got {checked}"
    );
    // Pacing: not_before within ½ chunk period of the send time.
    let half = h.params.chunk_period.mul_f64(0.5);
    let mut symbols = 0;
    for (t, _, _, f, nb) in h.sends(p) {
        if let Frame::RaptorQSymbol(_) = f {
            let nb = nb.expect("source pacing sets not_before");
            assert!(
                nb >= t && nb.duration_since(t) <= half,
                "pacing offset {} > {}",
                nb.duration_since(t),
                half
            );
            symbols += 1;
        }
    }
    assert!(symbols > 100);
}

/// Ch1 §1.2.2 §2.2 and §1.3.1: a viewer discovers, probes over UDP, opens a
/// session, sends NEIGHBOR on the control stream, receives ACCEPTED, attaches at
/// depth 1 in every subscribed tree and becomes ACTIVE.
#[test]
fn viewer_join_happy_path() {
    let mut h = Harness::new();
    let (p, v) = pub_and_viewer(&mut h);
    let i_disc = h
        .find_from(v, 0, |i| matches!(i, TraceItem::Discover))
        .expect("discover");
    let i_conn = h
        .find_from(v, i_disc, |i| {
            matches!(
                i,
                TraceItem::Event(Event::State {
                    to: Lifecycle::Connecting,
                    ..
                })
            )
        })
        .expect("connecting");
    let i_probe = h.find_from(v, i_conn, |i| matches!(i, TraceItem::Send { channel: Channel::Udp, frame: Frame::Probe(_), to, .. } if *to == p)).expect("probe");
    let i_open = h
        .find_from(
            v,
            i_probe,
            |i| matches!(i, TraceItem::OpenSession(a) if *a == p),
        )
        .expect("open session");
    let i_nb = h.find_from(v, i_open, |i| matches!(i, TraceItem::Send { channel: Channel::Control, frame: Frame::Neighbor(n), .. } if n.tree_id.is_tree())).expect("neighbor");
    let i_att = h
        .find_from(v, i_nb, |i| {
            matches!(i, TraceItem::Event(Event::ParentAttached { depth: 1, .. }))
        })
        .expect("attached");
    let i_act = h
        .find_from(v, i_att, |i| {
            matches!(
                i,
                TraceItem::Event(Event::State {
                    to: Lifecycle::Active,
                    ..
                })
            )
        })
        .expect("active");
    assert!(
        i_disc < i_conn
            && i_conn < i_probe
            && i_probe < i_open
            && i_open < i_nb
            && i_nb < i_att
            && i_att < i_act
    );
    assert_eq!(
        h.count_events(v, |e| matches!(e, Event::ParentAttached { .. })),
        3,
        "one parent per tree"
    );
    // The publisher answered with ACCEPTED, not a rejection.
    assert!(sends_of(&h, p, FrameType::ACCEPTED).len() >= 3);
    assert!(sends_of(&h, p, FrameType::DISCONNECT).is_empty());
    for t in h.node(v).trees() {
        assert_eq!(t.depth, 1);
        assert!(t.parent.is_some());
    }
}

/// Ch4 §4.1.2: a relay forwards a block only after verifying it, and the
/// BLOCK_PROOF it sends carries its own depth (Ch1 §1.2.2 Depth Propagation).
#[test]
fn verify_then_forward() {
    let mut h = Harness::new();
    let (_p, r, l) = chain_topology(&mut h);
    h.run_for(secs(2.0));
    let items = h.items(r);
    let mut forwards = 0;
    for (t, it) in items.iter() {
        if let TraceItem::Send {
            to,
            frame: Frame::BlockProof(bp),
            ..
        } = it
        {
            assert_eq!(*to, l);
            assert_eq!(bp.sender_hop_depth, 1, "relay sits at depth 1");
            // Verification and forwarding happen in the same `handle` call, so the
            // BlockVerified event carries the same instant as the send; it must never
            // be later.
            let verified = items.iter().any(|(tv, j)| *tv <= *t && matches!(j, TraceItem::Event(Event::BlockVerified { segment, block, .. }) if *segment == bp.segment.0 && *block == bp.block.0));
            assert!(
                verified,
                "forwarded {:?} {:?} without verifying it",
                bp.segment, bp.block
            );
            forwards += 1;
        }
    }
    // And nothing was forwarded that the relay never verified at all.
    assert!(h.count_events(r, |e| matches!(e, Event::BlockRejected { .. })) == 0);
    assert!(forwards > 8, "relay forwarded {forwards} blocks");
}

/// Ch4 §4.1.2: corrupted symbols fail Merkle verification, are never forwarded
/// or delivered, and the delivering parent is flagged and evicted.
#[test]
fn poisoned_symbols_are_rejected_and_parent_evicted() {
    let mut h = Harness::new();
    let (_p, r, l) = chain_topology(&mut h);
    h.poison_all_from = Some(r);
    h.run_for(secs(3.0));
    assert!(h.count_events(l, |e| matches!(e, Event::BlockRejected { .. })) >= 3);
    assert_eq!(
        h.count_events(
            l,
            |e| matches!(e, Event::ParentLost { reason, .. } if reason == "poisoned blocks")
        ),
        1
    );
    // Nothing corrupted reached the application.
    for (_, chunk, complete, _) in h.delivers(l) {
        let src = h
            .published
            .iter()
            .find(|c| c.segment == chunk.segment && c.chunk_index == chunk.chunk_index)
            .expect("source chunk");
        for li in 0..complete as usize {
            assert_eq!(blake3_hash(&chunk.layers[li]), blake3_hash(&src.layers[li]));
        }
    }
    // The leaf never forwarded anything (it has no children) and never sent a
    // symbol at all.
    assert!(sends_of(&h, l, FrameType::RAPTORQ_SYMBOL).is_empty());
}

/// Ch7 §7.1.2: manifests are admitted by sequence window — a forged signature is
/// rejected, a duplicate is ignored without penalty, and one far ahead of the
/// live edge is outside the window.
#[test]
fn manifest_acceptance_window() {
    let mut h = Harness::new();
    let (p, v) = pub_and_viewer(&mut h);
    let (acc_seg, acc_chunk) = h
        .events(v)
        .into_iter()
        .find_map(|(_, e)| {
            if let Event::ManifestAccepted { segment, chunk, .. } = e {
                Some((segment, chunk))
            } else {
                None
            }
        })
        .expect("viewer accepted a manifest");
    let real = sends_of(&h, p, FrameType::MANIFEST)
        .into_iter()
        .filter_map(|(_, to, f, _)| {
            if to == v {
                if let Frame::Manifest(m) = f {
                    Some(m)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .find(|m| m.body.segment.0 == acc_seg && m.body.chunk_index == acc_chunk)
        .expect("the accepted manifest was sent by the publisher");
    let accepted_before = h.count_events(v, |e| matches!(e, Event::ManifestAccepted { .. }));
    // Duplicate.
    h.inject(v, p, Channel::Control, Frame::Manifest(real.clone()));
    assert_eq!(
        h.count_events(v, |e| matches!(e, Event::ManifestAccepted { .. })),
        accepted_before
    );
    assert_eq!(
        h.count_events(v, |e| matches!(e, Event::ManifestRejected { .. })),
        0,
        "duplicates are never penalised"
    );
    // Forged signature on a new chunk.
    let live = h.node(v).swarm().live_edge;
    let mut forged = real.clone();
    forged.body.segment = live.next();
    forged.body.chunk_index = 0;
    forged.signature = SignatureBytes([7; 64]);
    h.inject(v, p, Channel::Control, Frame::Manifest(forged));
    assert_eq!(h.count_events(v, |e| matches!(e, Event::ManifestRejected { reason, .. } if reason.contains("BadSignature"))), 1);
    // Far ahead of the live edge.
    let mut ahead = real.clone();
    ahead.body.segment = SegmentSeq(live.0 + h.params.manifest_window_ahead + 5);
    h.inject(v, p, Channel::Control, Frame::Manifest(ahead));
    assert_eq!(h.count_events(v, |e| matches!(e, Event::ManifestRejected { reason, .. } if reason.contains("OutsideWindow"))), 1);
}

/// Ch3 §3.3: after τ_ping of silence the child PINGs its parent; after τ_evict it
/// evicts it, enters per-tree repair and probes for a new parent.
#[test]
fn heartbeat_then_eviction() {
    let mut h = Harness::new();
    let (p, v) = pub_and_viewer(&mut h);
    let t0 = h.now;
    h.drop_from.insert(p);
    // Frames the publisher already emitted stay in flight for up to the pacing
    // window (½ chunk period) plus latency; silence starts after the last arrives.
    h.run_for(secs(1.0));
    let last_rx = h
        .sends(p)
        .iter()
        .filter(|(t, to, ..)| *to == v && *t <= t0)
        .map(|(t, _, _, _, nb)| nb.unwrap_or(*t) + LATENCY)
        .max()
        .unwrap_or(t0)
        .max(t0);
    let ping = sends_of(&h, v, FrameType::PING)
        .into_iter()
        .find(|(t, to, _, _)| *t >= t0 && *to == p)
        .expect("child PINGs a silent parent");
    let silence_at_ping = ping.0.duration_since(last_rx);
    assert!(
        silence_at_ping >= h.params.tau_ping
            && silence_at_ping < h.params.tau_ping + Duration::from_millis(60),
        "ping after {silence_at_ping} of silence"
    );
    let lost: Vec<(Instant, Event)> = h
        .events(v)
        .into_iter()
        .filter(|(_, e)| matches!(e, Event::ParentLost { reason, .. } if reason == "timeout"))
        .collect();
    assert_eq!(lost.len(), 3, "every tree's parent was the publisher");
    assert!(lost[0].0 > ping.0, "ping precedes eviction");
    let evict_at = lost[0].0.duration_since(last_rx);
    assert!(
        evict_at >= h.params.tau_evict_floor && evict_at < Duration::from_millis(600),
        "evicted after {evict_at} of silence"
    );
    assert!(
        sends_of(&h, v, FrameType::PROBE)
            .iter()
            .any(|(t, _, _, _)| *t >= lost[0].0),
        "repair probes after eviction"
    );
    assert!(h.node(v).trees()[0].parent.is_none());
}

/// Ch4 §4.3.1: chunks reach the application Δ_buffer after the live edge, in
/// presentation order; FirstChunk fires once; steady-state chunks carry every
/// subscribed layer.
#[test]
fn playout_timeline() {
    let mut h = Harness::new();
    let (_p, v) = pub_and_viewer(&mut h);
    h.run_for(secs(6.0));
    let first_manifest = h
        .events(v)
        .into_iter()
        .find(|(_, e)| matches!(e, Event::ManifestAccepted { .. }))
        .expect("manifest")
        .0;
    let firsts: Vec<(Instant, Event)> = h
        .events(v)
        .into_iter()
        .filter(|(_, e)| matches!(e, Event::FirstChunk { .. }))
        .collect();
    assert_eq!(firsts.len(), 1);
    let gap = firsts[0].0.duration_since(first_manifest);
    assert!(
        gap >= Duration::from_millis(2_900) && gap <= Duration::from_millis(3_600),
        "first playable chunk {gap} after the first manifest"
    );
    let delivers = h.delivers(v);
    let mut keys: Vec<(u32, u8)> = delivers
        .iter()
        .map(|(_, c, _, _)| (c.segment.0, c.chunk_index))
        .collect();
    let sorted = {
        let mut s = keys.clone();
        s.sort();
        s.dedup();
        s
    };
    keys.dedup();
    assert_eq!(keys, sorted, "deliveries in presentation order");
    let after_first: Vec<_> = h
        .events(v)
        .into_iter()
        .filter(|(t, e)| *t > firsts[0].0 && matches!(e, Event::ChunkPlayed { .. }))
        .collect();
    assert!(after_first.len() >= 8);
    for (_, e) in after_first {
        if let Event::ChunkPlayed {
            ok,
            layers_complete,
            layers_subscribed,
            ..
        } = e
        {
            assert!(ok);
            assert_eq!(layers_complete, layers_subscribed);
            assert_eq!(layers_subscribed, 3);
        }
    }
}

/// End to end: publisher → relay → leaf. Every layer the leaf and the relay
/// deliver is byte-identical to what the source emitted.
#[test]
fn data_correctness_end_to_end() {
    let mut h = Harness::new();
    let (_p, r, l) = chain_topology(&mut h);
    h.run_for(secs(4.0));
    for a in [r, l] {
        let d = h.delivers(a);
        let full: Vec<_> = d
            .iter()
            .filter(|(_, _, complete, _)| *complete >= 1)
            .collect();
        assert!(
            full.len() >= 8,
            "{a} delivered only {} complete chunks",
            full.len()
        );
        for (_, chunk, complete, subscribed) in &d {
            assert_eq!(*subscribed, 1, "layer prefix L0 only");
            let src = h
                .published
                .iter()
                .find(|c| c.segment == chunk.segment && c.chunk_index == chunk.chunk_index)
                .expect("source chunk exists");
            for li in 0..*complete as usize {
                assert_eq!(
                    chunk.layers[li], src.layers[li],
                    "layer {li} of {} c{}",
                    chunk.segment, chunk.chunk_index
                );
            }
        }
    }
}

/// ISSUE-059: a refused tree join is answered with DISCONNECT carrying a
/// rejection reason and the tree id — NOT_ASSIGNED when the responder does not
/// relay the tree, SATURATED when it has no free slot for a non-relay requester.
#[test]
fn join_rejections_carry_reason_and_tree() {
    let mut h = Harness::new();
    let (p, r, _l) = chain_topology(&mut h);
    let stranger = addr(9);
    let nb = |tree: u8, class: NodeClass| {
        Frame::Neighbor(Neighbor {
            sender: NodeId([0x55; 32]),
            priority: Priority::Low,
            tree_id: TreeId(tree),
            node_class: class,
            assigned_trees: TreeSet::EMPTY,
        })
    };
    // The relay is assigned to tree 1 only.
    h.inject(r, stranger, Channel::Control, nb(2, NodeClass::Leaf));
    let d = sends_of(&h, r, FrameType::DISCONNECT)
        .into_iter()
        .find(|(_, to, _, _)| *to == stranger)
        .expect("rejection sent");
    let Frame::Disconnect(d) = d.2 else {
        unreachable!()
    };
    assert_eq!(d.reason, DisconnectReason::RejectedNotAssigned);
    assert_eq!(d.tree_id, Some(TreeId(2)));
    // The publisher's single tree-1 slot is held by the relay.
    h.inject(p, stranger, Channel::Control, nb(1, NodeClass::Leaf));
    let d = sends_of(&h, p, FrameType::DISCONNECT)
        .into_iter()
        .find(|(_, to, _, _)| *to == stranger)
        .expect("rejection sent");
    let Frame::Disconnect(d) = d.2 else {
        unreachable!()
    };
    assert_eq!(d.reason, DisconnectReason::RejectedSaturated);
    assert_eq!(d.tree_id, Some(TreeId(1)));
    assert!(sends_of(&h, p, FrameType::ACCEPTED)
        .iter()
        .all(|(_, to, _, _)| *to != stranger));
}

/// ISSUE-061: a relay assigned to tree m displaces a pure subscriber holding the
/// last slot in m; the subscriber gets DRAIN_NOTICE(DISPLACED) and keeps being
/// served while it re-selects.
#[test]
fn relay_of_tree_displaces_pure_subscriber() {
    let mut h = Harness::new();
    let p = addr(1);
    let sub = addr(2);
    let relay = addr(3);
    h.add_publisher(p, 8000, 1); // K_v = (1, 1, 0)
    h.add_viewer(sub, NodeClass::Leaf, 0, 1000, 5);
    let seed = seed_for_tree(relay, &h.params.clone(), 3, 1);
    h.add_viewer(relay, NodeClass::Relay, 0, 10_000, seed);
    h.start(p);
    h.start_publishing();
    h.run_for(secs(0.5));
    h.start(sub);
    h.run_for(secs(2.0));
    assert!(
        h.node(sub).trees()[0].parent.is_some(),
        "subscriber holds the only tree-1 slot"
    );
    let before = h.now;
    h.start(relay);
    h.run_for(secs(2.0));
    let drains: Vec<_> = sends_of(&h, p, FrameType::DRAIN_NOTICE)
        .into_iter()
        .filter(|(t, to, _, _)| *t >= before && *to == sub)
        .collect();
    assert_eq!(
        drains.len(),
        1,
        "exactly one drain notice to the subscriber"
    );
    let Frame::DrainNotice(d) = &drains[0].2 else {
        unreachable!()
    };
    assert_eq!(d.reason, DisconnectReason::Displaced);
    assert_eq!(d.tree_id, TreeId(1));
    let acc = sends_of(&h, p, FrameType::ACCEPTED)
        .into_iter()
        .find(|(t, to, _, _)| *t >= before && *to == relay)
        .expect("relay accepted");
    let Frame::Accepted(a) = acc.2 else {
        unreachable!()
    };
    assert_eq!(a.tree_id, TreeId(1));
    assert!(a.pending, "8 Mbps < 10·B_m·Ω → sequential handover");
    assert_eq!(
        h.count_events(relay, |e| matches!(
            e,
            Event::ParentAttached { tree: 1, .. }
        )),
        1
    );
    // The subscriber was never dropped mid-stream: it kept its parent until it
    // moved (or the drain deadline), and its base layer stayed complete.
    assert_eq!(
        h.count_events(
            sub,
            |e| matches!(e, Event::ParentLost { reason, .. } if reason == "timeout")
        ),
        0
    );
}

/// Ch1 §1.1.5 §5.3 and §1.2.4 §4.2.2: two consecutive failed join rounds for a
/// tree shed that layer and every layer above it; the base layer is never shed
/// and the node still reaches ACTIVE.
#[test]
fn shed_rule_drops_unreachable_layer() {
    let mut h = Harness::new();
    let p = addr(1);
    let v = addr(2);
    h.add_publisher(p, 8000, 1); // K_v = (1, 1, 0): tree 3 has no slot anywhere
    let seed = seed_for_tree(v, &h.params.clone(), 3, 1);
    h.add_viewer(v, NodeClass::Relay, 2, 10_000, seed);
    h.start(p);
    h.start_publishing();
    h.run_for(secs(0.5));
    h.start(v);
    h.run_for(secs(4.0));
    assert!(
        h.count_events(v, |e| matches!(e, Event::JoinRoundFailed { tree: 3, .. }))
            >= h.params.shed_after_failed_rounds as usize
    );
    assert_eq!(
        h.count_events(v, |e| matches!(e, Event::LayerShed { top_layer: 1 })),
        1
    );
    assert_eq!(h.node(v).top_layer(), 1);
    assert_eq!(h.node(v).state(), Lifecycle::Active);
    let trees = h.node(v).trees();
    assert!(trees[0].parent.is_some() && trees[1].parent.is_some());
    assert!(!trees[2].subscribed && trees[2].parent.is_none());
}

/// Ch1 §1.3.1 STREAM_END: the source signs a STREAM_END; children verify it,
/// forward it down their subtrees and terminate; a forged one is ignored.
#[test]
fn stream_end_propagates_and_forgeries_are_ignored() {
    let mut h = Harness::new();
    let (p, r, l) = chain_topology(&mut h);
    // Forgery first: zero signature from the parent.
    let sid = h.node(p).stream_id();
    let forged = StreamEnd {
        stream_id: sid,
        final_segment: SegmentSeq(5),
        timestamp_us: 1,
        signature: SignatureBytes::ZERO,
    };
    h.inject(r, p, Channel::Control, Frame::StreamEnd(forged));
    assert_eq!(
        h.node(r).state(),
        Lifecycle::Active,
        "forged STREAM_END ignored"
    );
    // The real thing.
    h.feed(p, Input::Cmd(Command::EndStream));
    h.run_for(secs(1.0));
    assert_eq!(h.node(p).state(), Lifecycle::Terminated);
    assert!(sends_of(&h, p, FrameType::STREAM_END)
        .iter()
        .any(|(_, to, _, _)| *to == r));
    assert!(
        sends_of(&h, r, FrameType::STREAM_END)
            .iter()
            .any(|(_, to, _, _)| *to == l),
        "relay forwards to its children"
    );
    assert_eq!(h.node(r).state(), Lifecycle::Terminated);
    assert_eq!(h.node(l).state(), Lifecycle::Terminated);
}
