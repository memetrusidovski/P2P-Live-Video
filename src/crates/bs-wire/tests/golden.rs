//! Golden byte vectors. Each frame below is constructed from fixed field values
//! and its hex encoding is compared with `tests/vectors/frames.json`, which is
//! also consumed by the Python decoder in `py/`. Run with
//! `BS_UPDATE_VECTORS=1` to regenerate after an intentional layout change.

use std::net::{Ipv4Addr, SocketAddr};

use bs_wire::frames::*;
use bs_wire::*;
use bytes::Bytes;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct Vector {
    name: String,
    frame_type: u8,
    spec: String,
    hex: String,
}

fn vb(seed: u8) -> ValidationBlock {
    ValidationBlock {
        node_id: NodeId([seed; 32]),
        public_key: PublicKeyBytes([seed.wrapping_add(1); 32]),
        static_nonce: 0x0102030405060708,
        dynamic_nonce: 0x1112131415161718,
        timestamp_us: 1_700_000_000_000_000,
        signature: SignatureBytes([seed.wrapping_add(2); 64]),
    }
}

fn fixtures() -> Vec<(&'static str, &'static str, Frame)> {
    let sid = StreamId([0xAA; 32]);
    let nid = NodeId([0x11; 32]);
    let body = ManifestBody {
        stream_id: sid,
        segment: SegmentSeq(42),
        chunk_index: 2,
        chunk_count: 4,
        timestamp_us: 1_700_000_000_250_000,
        merkle_root: Hash([0xCC; 32]),
        slicing_matrix_version: 3,
        layer_block_counts: vec![3, 3, 6],
        layer_byte_lengths: vec![46_875, 46_875, 93_750],
    };
    vec![
        (
            "ping",
            "App D §D.4.1",
            Frame::Ping(Ping {
                validation: vb(0x10),
                stream_id: sid,
            }),
        ),
        (
            "pong_v4",
            "App D §D.4.1",
            Frame::Pong(Pong {
                validation: vb(0x20),
                reflected: WireAddr(SocketAddr::new(Ipv4Addr::new(203, 0, 113, 7).into(), 40000)),
            }),
        ),
        (
            "join",
            "App D §D.4.2",
            Frame::Join(Join {
                validation: vb(0x30),
                stream_id: sid,
                node_class: NodeClass::Leaf,
            }),
        ),
        (
            "probe",
            "App D §D.4.7",
            Frame::Probe(Probe {
                validation: vb(0x40),
                stream_id: sid,
                tree_id: TreeId(2),
            }),
        ),
        (
            "probe_response",
            "App D §D.4.7",
            Frame::ProbeResponse(ProbeResponse {
                sender: nid,
                k_avail: 6,
                reliability: ProbeResponse::reliability_from_f32(0.98),
                hop_count: 3,
                node_class: NodeClass::Relay,
                flags: PeerFlags {
                    reachability: Reachability::Cone,
                    relay_capable: true,
                    source: false,
                    source_ingress: false,
                },
                tree_state: TreeState::Serving,
                assigned_trees: TreeSet(0b0000_0110),
                live_edge: SegmentSeq(41),
            }),
        ),
        (
            "neighbor_tree_join",
            "App D §D.4.3",
            Frame::Neighbor(Neighbor {
                sender: nid,
                priority: Priority::High,
                tree_id: TreeId(2),
                node_class: NodeClass::Relay,
                assigned_trees: TreeSet(0b100),
            }),
        ),
        (
            "accepted",
            "App D §D.4.4",
            Frame::Accepted(Accepted {
                sender: nid,
                accepted_type: AcceptedType::Neighbor,
                tree_id: TreeId(2),
                hop_depth: 4,
                pending: true,
            }),
        ),
        (
            "disconnect_quit",
            "Ch3 §3.2.1",
            Frame::Disconnect(Disconnect {
                sender: nid,
                reason: DisconnectReason::Quit,
                tree_id: TreeId::NONE,
            }),
        ),
        (
            "disconnect_rejected_not_assigned",
            "App D §D.4.3b",
            Frame::Disconnect(Disconnect {
                sender: nid,
                reason: DisconnectReason::RejectedNotAssigned,
                tree_id: TreeId(3),
            }),
        ),
        (
            "drain_notice",
            "App D §D.4.19",
            Frame::DrainNotice(DrainNotice {
                tree_id: TreeId(1),
                reason: DisconnectReason::Preempted,
                scope: DrainScope::ThisChild,
                deadline: SegmentSeq(47),
            }),
        ),
        (
            "stream_end",
            "Ch1 §1.3.1",
            Frame::StreamEnd(StreamEnd {
                stream_id: sid,
                final_segment: SegmentSeq(1000),
                timestamp_us: 1_700_000_100_000_000,
                signature: SignatureBytes([0xEE; 64]),
            }),
        ),
        (
            "choke_state",
            "App D §D.4.13",
            Frame::ChokeState(ChokeState::UnchokeOptimistic),
        ),
        (
            "manifest_request_pending",
            "App D §D.4.16",
            Frame::ManifestRequest(ManifestRequest {
                segment: SegmentSeq(0),
                selector: ManifestSelector::PendingUpdate,
            }),
        ),
        (
            "pull_request",
            "App D §D.4.11",
            Frame::PullRequest(PullRequest {
                segment: SegmentSeq(42),
                block: BlockIndex::new(2, 5).unwrap(),
                missing_symbols: 0,
                urgency_ms: 800,
                broadcast_want: true,
            }),
        ),
        (
            "manifest",
            "App D §D.4.8",
            Frame::Manifest(Manifest {
                body: body.clone(),
                signature: SignatureBytes([0xDD; 64]),
            }),
        ),
        (
            "manifest_update",
            "App D §D.4.8",
            Frame::ManifestUpdate(ManifestUpdate {
                body,
                effective_segment: SegmentSeq(47),
                trees: vec![
                    TreeMappingEntry {
                        tree_id: TreeId(1),
                        layer: 0,
                        stripe_index: 0,
                        stripe_count: 1,
                        priority: 2,
                        bitrate_kbps: 1500,
                    },
                    TreeMappingEntry {
                        tree_id: TreeId(2),
                        layer: 1,
                        stripe_index: 0,
                        stripe_count: 1,
                        priority: 1,
                        bitrate_kbps: 1500,
                    },
                    TreeMappingEntry {
                        tree_id: TreeId(3),
                        layer: 2,
                        stripe_index: 0,
                        stripe_count: 1,
                        priority: 0,
                        bitrate_kbps: 3000,
                    },
                ],
                signature: SignatureBytes([0xDD; 64]),
            }),
        ),
        (
            "block_proof",
            "App D §D.4.9",
            Frame::BlockProof(BlockProof {
                segment: SegmentSeq(42),
                block: BlockIndex::new(2, 5).unwrap(),
                sender_hop_depth: 3,
                siblings: vec![Hash([1; 32]), Hash([2; 32]), Hash([3; 32]), Hash([4; 32])],
            }),
        ),
        (
            "raptorq_symbol",
            "Ch4 §4.2.3",
            Frame::RaptorQSymbol(RaptorQSymbol {
                segment: SegmentSeq(42),
                block: BlockIndex::new(2, 5).unwrap(),
                esi: 17,
                payload: Bytes::from(vec![0x5A; 1024]),
            }),
        ),
        (
            "block_transmission",
            "Ch4 §4.1.3",
            Frame::BlockTransmission(BlockTransmission {
                segment: SegmentSeq(42),
                block: BlockIndex::new(2, 5).unwrap(),
                siblings: vec![Hash([1; 32]), Hash([2; 32])],
                data: Bytes::from(vec![0x7B; 64]),
            }),
        ),
        (
            "register_peer",
            "Ch2 §2.3.3",
            Frame::RegisterPeer(RegisterPeer {
                stream_id: sid,
                validation: vb(0x50),
                port: 4001,
                node_class: NodeClass::Relay,
                assigned_trees: TreeSet(0b0000_0010),
                flags: PeerFlags {
                    reachability: Reachability::Public,
                    relay_capable: true,
                    source: false,
                    source_ingress: false,
                },
                signature: SignatureBytes([0x66; 64]),
            }),
        ),
        (
            "get_peers_request",
            "App D §D.4.6b",
            Frame::GetPeers(GetPeersFrame::Request(GetPeers {
                validation: vb(0x60),
                stream_id: sid,
                starved_trees: TreeSet(0b0000_0100),
                wanted_trees: TreeSet(0b0000_0110),
                want_relay_capable: true,
            })),
        ),
        (
            "get_peers_response",
            "App D §D.4.6b",
            Frame::GetPeers(GetPeersFrame::Response(GetPeersResponse {
                guardian: NodeId([0x77; 32]),
                sampled: true,
                records: vec![
                    PeerRecord {
                        node_id: NodeId([0x21; 32]),
                        node_class: NodeClass::Relay,
                        assigned_trees: TreeSet(0b0000_0001),
                        flags: PeerFlags {
                            reachability: Reachability::Public,
                            relay_capable: false,
                            source: true,
                            source_ingress: false,
                        },
                        addr: WireAddr(SocketAddr::new(Ipv4Addr::new(10, 0, 0, 1).into(), 4000)),
                    },
                    PeerRecord {
                        node_id: NodeId([0x22; 32]),
                        node_class: NodeClass::Leaf,
                        assigned_trees: TreeSet(0),
                        flags: PeerFlags {
                            reachability: Reachability::Cone,
                            relay_capable: false,
                            source: false,
                            source_ingress: false,
                        },
                        addr: WireAddr(SocketAddr::new(Ipv4Addr::new(10, 0, 0, 2).into(), 4002)),
                    },
                ],
                stream_record: Some(StreamRecord {
                    publisher_pubkey: PublicKeyBytes([0x88; 32]),
                    manifest_version: 9,
                    slicing_mode: SlicingMode::SvcSpatial,
                    register_sample_log2: 0,
                    swarm_size: 120,
                    relay_count: 80,
                    live_edge_segment: SegmentSeq(41),
                    live_edge_manifest_hash: Hash([0xCC; 32]),
                    live_edge_timestamp_us: 1_700_000_000_250_000,
                    effective_segment: SegmentSeq(0),
                    descriptor_version: 1,
                    descriptor_hash: Hash([0x5D; 32]),
                    trees: vec![
                        TreeMappingEntry {
                            tree_id: TreeId(1),
                            layer: 0,
                            stripe_index: 0,
                            stripe_count: 1,
                            priority: 2,
                            bitrate_kbps: 1500,
                        },
                        TreeMappingEntry {
                            tree_id: TreeId(2),
                            layer: 1,
                            stripe_index: 0,
                            stripe_count: 1,
                            priority: 1,
                            bitrate_kbps: 1500,
                        },
                        TreeMappingEntry {
                            tree_id: TreeId(3),
                            layer: 2,
                            stripe_index: 0,
                            stripe_count: 1,
                            priority: 0,
                            bitrate_kbps: 3000,
                        },
                    ],
                    trees_next: vec![],
                    signature: SignatureBytes([0x99; 64]),
                }),
            })),
        ),
        (
            "stream_descriptor",
            "App D §D.4.20",
            Frame::StreamDescriptor(StreamDescriptor {
                stream_id: sid,
                version: 1,
                effective_segment: SegmentSeq(1),
                content_type: ContentType::VideoCmaf,
                layer_mode: LayerMode::Simulcast,
                layers: vec![
                    LayerDescriptor {
                        codec_tag: *b"avc1",
                        width: 640,
                        height: 360,
                        frame_rate_milli: 24_000,
                        bitrate_kbps: 800,
                        init_data: Bytes::from(vec![0x1A; 16]),
                    },
                    LayerDescriptor {
                        codec_tag: *b"avc1",
                        width: 1280,
                        height: 720,
                        frame_rate_milli: 24_000,
                        bitrate_kbps: 2500,
                        init_data: Bytes::from(vec![0x2B; 16]),
                    },
                ],
                signature: SignatureBytes([0xD5; 64]),
            }),
        ),
    ]
}

const PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/vectors/frames.json");

#[test]
fn golden_vectors_match() {
    let produced: Vec<Vector> = fixtures()
        .into_iter()
        .map(|(name, spec, f)| Vector {
            name: name.into(),
            frame_type: f.frame_type() as u8,
            spec: spec.into(),
            hex: hex::encode(f.to_bytes().unwrap()),
        })
        .collect();

    if std::env::var("BS_UPDATE_VECTORS").is_ok() {
        std::fs::write(PATH, serde_json::to_string_pretty(&produced).unwrap()).unwrap();
        return;
    }
    let expected: Vec<Vector> = serde_json::from_str(
        &std::fs::read_to_string(PATH)
            .expect("run once with BS_UPDATE_VECTORS=1 to create vectors"),
    )
    .unwrap();
    for (p, e) in produced.iter().zip(expected.iter()) {
        assert_eq!(p.name, e.name, "vector order changed");
        assert_eq!(
            p.hex, e.hex,
            "encoding of `{}` ({}) changed",
            p.name, p.spec
        );
    }
    assert_eq!(produced.len(), expected.len(), "vector count changed");
    // And every vector decodes back to the fixture.
    for ((_, _, f), v) in fixtures().into_iter().zip(expected) {
        let back = Frame::from_slice(&hex::decode(v.hex).unwrap()).unwrap();
        assert_eq!(back, f);
    }
}

/// Spot checks of exact byte offsets against the diagrams in Appendix D, so a
/// field reorder cannot pass by roundtripping alone.
#[test]
fn header_and_offsets() {
    let f = Frame::Neighbor(Neighbor {
        sender: NodeId([9; 32]),
        priority: Priority::Low,
        tree_id: TreeId(5),
        node_class: NodeClass::Leaf,
        assigned_trees: TreeSet(0),
    });
    let b = f.to_bytes().unwrap();
    assert_eq!(&b[..4], &[0x01, 0x05, 0x00, 36]);
    assert_eq!(b[4 + 32], 0x02, "Priority");
    assert_eq!(b[4 + 33], 5, "TreeID");
    assert_eq!(b[4 + 34], 0x01, "NodeClass");

    let p = Frame::Ping(Ping {
        validation: vb(0),
        stream_id: StreamId::ZERO,
    })
    .to_bytes()
    .unwrap();
    assert_eq!(p.len(), 4 + 152 + 32);
    assert_eq!(
        &p[4 + 64..4 + 72],
        &0x0102030405060708u64.to_be_bytes(),
        "StaticNonce at VB+64"
    );
}
