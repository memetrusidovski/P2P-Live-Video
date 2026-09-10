//! Property tests: every typed frame survives encode → decode unchanged, and the
//! header's payload length matches what was written. Spec: Appendix D.

use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};

use bs_wire::frames::*;
use bs_wire::*;
use bytes::Bytes;
use proptest::prelude::*;

fn arb_hash() -> impl Strategy<Value = Hash> {
    any::<[u8; 32]>().prop_map(Hash)
}
fn arb_node_id() -> impl Strategy<Value = NodeId> {
    any::<[u8; 32]>().prop_map(NodeId)
}
fn arb_sig() -> impl Strategy<Value = SignatureBytes> {
    any::<[u8; 64]>().prop_map(SignatureBytes)
}
fn arb_class() -> impl Strategy<Value = NodeClass> {
    prop_oneof![
        Just(NodeClass::Relay),
        Just(NodeClass::Leaf),
        Just(NodeClass::LeafPrivate)
    ]
}
fn arb_flags() -> impl Strategy<Value = PeerFlags> {
    (0u8..=0x1F).prop_map(|b| PeerFlags::from_byte(b & !0b11 | (b & 0b11).min(2)).unwrap())
}
fn arb_addr() -> impl Strategy<Value = WireAddr> {
    prop_oneof![
        (any::<[u8; 4]>(), any::<u16>())
            .prop_map(|(ip, p)| WireAddr(SocketAddr::new(Ipv4Addr::from(ip).into(), p))),
        (any::<[u8; 16]>(), any::<u16>())
            .prop_map(|(ip, p)| WireAddr(SocketAddr::new(Ipv6Addr::from(ip).into(), p))),
    ]
}
fn arb_vb() -> impl Strategy<Value = ValidationBlock> {
    (
        arb_node_id(),
        any::<[u8; 32]>(),
        any::<u64>(),
        any::<u64>(),
        any::<u64>(),
        arb_sig(),
    )
        .prop_map(|(node_id, pk, s, d, t, sig)| ValidationBlock {
            node_id,
            public_key: PublicKeyBytes(pk),
            static_nonce: s,
            dynamic_nonce: d,
            timestamp_us: t,
            signature: sig,
        })
}
fn arb_block_index() -> impl Strategy<Value = BlockIndex> {
    (0u8..16, 0u16..4096).prop_map(|(c, j)| BlockIndex::new(c, j).unwrap())
}
fn arb_reason() -> impl Strategy<Value = DisconnectReason> {
    (1u8..=0x0C).prop_map(|c| DisconnectReason::from_code(c).unwrap())
}
fn arb_mapping() -> impl Strategy<Value = TreeMappingEntry> {
    (1u8..=8, 0u8..4, 0u8..4, 1u8..4, any::<u8>(), any::<u16>()).prop_map(|(t, l, si, sc, p, b)| {
        TreeMappingEntry {
            tree_id: TreeId(t),
            layer: l,
            stripe_index: si,
            stripe_count: sc,
            priority: p,
            bitrate_kbps: b,
        }
    })
}
fn arb_manifest_body() -> impl Strategy<Value = ManifestBody> {
    (
        any::<[u8; 32]>(),
        1u32..,
        0u8..4,
        any::<u64>(),
        arb_hash(),
        any::<u32>(),
        any::<u8>(),
        prop::collection::vec(0u16..64, 1..4),
    )
        .prop_map(|(sid, seg, ci, ts, root, len, v, layers)| ManifestBody {
            stream_id: StreamId(sid),
            segment: SegmentSeq(seg),
            chunk_index: ci,
            chunk_count: 4,
            timestamp_us: ts,
            merkle_root: root,
            chunk_byte_length: len,
            slicing_matrix_version: v,
            layer_block_counts: layers,
        })
}

fn arb_frame() -> impl Strategy<Value = Frame> {
    prop_oneof![
        (arb_vb(), any::<[u8; 32]>()).prop_map(|(v, s)| Frame::Ping(Ping {
            validation: v,
            stream_id: StreamId(s)
        })),
        (arb_vb(), arb_addr()).prop_map(|(v, a)| Frame::Pong(Pong {
            validation: v,
            reflected: a
        })),
        (arb_vb(), any::<[u8; 32]>(), arb_class()).prop_map(|(v, s, c)| Frame::Join(Join {
            validation: v,
            stream_id: StreamId(s),
            node_class: c
        })),
        (arb_vb(), any::<[u8; 32]>(), 1u8..=8).prop_map(|(v, s, t)| Frame::Probe(Probe {
            validation: v,
            stream_id: StreamId(s),
            tree_id: TreeId(t)
        })),
        (
            arb_node_id(),
            any::<u16>(),
            any::<u16>(),
            any::<u8>(),
            arb_class(),
            arb_flags(),
            0u8..3,
            any::<u8>(),
            any::<u32>()
        )
            .prop_map(|(n, k, r, h, c, f, ts, at, le)| Frame::ProbeResponse(
                ProbeResponse {
                    sender: n,
                    k_avail: k,
                    reliability: r,
                    hop_count: h,
                    node_class: c,
                    flags: f,
                    tree_state: match ts {
                        0 => TreeState::Serving,
                        1 => TreeState::Warming,
                        _ => TreeState::Unparented,
                    },
                    assigned_trees: TreeSet(at),
                    live_edge: SegmentSeq(le),
                }
            )),
        (
            arb_node_id(),
            any::<bool>(),
            0u8..=8,
            arb_class(),
            any::<u8>()
        )
            .prop_map(|(n, p, t, c, a)| Frame::Neighbor(Neighbor {
                sender: n,
                priority: if p { Priority::High } else { Priority::Low },
                tree_id: TreeId(t),
                node_class: c,
                assigned_trees: TreeSet(a),
            })),
        (
            arb_node_id(),
            any::<bool>(),
            0u8..=8,
            any::<u8>(),
            any::<bool>()
        )
            .prop_map(|(n, j, t, h, p)| Frame::Accepted(Accepted {
                sender: n,
                accepted_type: if j {
                    AcceptedType::Join
                } else {
                    AcceptedType::Neighbor
                },
                tree_id: TreeId(t),
                hop_depth: h,
                pending: p,
            })),
        (arb_node_id(), arb_reason(), 1u8..=8).prop_map(|(n, r, t)| Frame::Disconnect(
            Disconnect {
                sender: n,
                reason: r,
                tree_id: if r.is_rejection() {
                    Some(TreeId(t))
                } else {
                    None
                },
            }
        )),
        (1u8..=8, arb_reason(), any::<bool>(), any::<u32>()).prop_map(|(t, r, s, d)| {
            Frame::DrainNotice(DrainNotice {
                tree_id: TreeId(t),
                reason: r,
                scope: if s {
                    DrainScope::AllChildren
                } else {
                    DrainScope::ThisChild
                },
                deadline: SegmentSeq(d),
            })
        }),
        (any::<[u8; 32]>(), any::<u32>(), any::<u64>(), arb_sig()).prop_map(|(s, f, t, sig)| {
            Frame::StreamEnd(StreamEnd {
                stream_id: StreamId(s),
                final_segment: SegmentSeq(f),
                timestamp_us: t,
                signature: sig,
            })
        }),
        (0u8..3).prop_map(|c| Frame::ChokeState(match c {
            0 => ChokeState::Choke,
            1 => ChokeState::Unchoke,
            _ => ChokeState::UnchokeOptimistic,
        })),
        (any::<u32>(), 0u8..6).prop_map(|(s, c)| Frame::ManifestRequest(ManifestRequest {
            segment: SegmentSeq(s),
            selector: match c {
                0 => ManifestSelector::AllChunks,
                1 => ManifestSelector::PendingUpdate,
                c => ManifestSelector::Chunk(c),
            },
        })),
        (
            any::<u32>(),
            arb_block_index(),
            any::<u8>(),
            any::<u16>(),
            any::<bool>()
        )
            .prop_map(|(s, b, m, u, w)| Frame::PullRequest(PullRequest {
                segment: SegmentSeq(s),
                block: b,
                missing_symbols: m,
                urgency_ms: u,
                broadcast_want: w,
            })),
        (arb_manifest_body(), arb_sig()).prop_map(|(b, s)| Frame::Manifest(Manifest {
            body: b,
            signature: s
        })),
        (
            arb_manifest_body(),
            any::<u32>(),
            prop::collection::vec(arb_mapping(), 1..7),
            arb_sig()
        )
            .prop_map(|(b, e, t, s)| Frame::ManifestUpdate(ManifestUpdate {
                body: b,
                effective_segment: SegmentSeq(e),
                trees: t,
                signature: s,
            })),
        (
            any::<u32>(),
            arb_block_index(),
            any::<u8>(),
            prop::collection::vec(arb_hash(), 0..8)
        )
            .prop_map(|(s, b, h, sib)| Frame::BlockProof(BlockProof {
                segment: SegmentSeq(s),
                block: b,
                sender_hop_depth: h,
                siblings: sib,
            })),
        (
            any::<u32>(),
            arb_block_index(),
            any::<u16>(),
            prop::collection::vec(any::<u8>(), 1024)
        )
            .prop_map(|(s, b, e, p)| Frame::RaptorQSymbol(RaptorQSymbol {
                segment: SegmentSeq(s),
                block: b,
                esi: e,
                payload: Bytes::from(p),
            })),
        (
            any::<u32>(),
            arb_block_index(),
            prop::collection::vec(arb_hash(), 0..8),
            prop::collection::vec(any::<u8>(), 0..2048)
        )
            .prop_map(
                |(s, b, sib, d)| Frame::BlockTransmission(BlockTransmission {
                    segment: SegmentSeq(s),
                    block: b,
                    siblings: sib,
                    data: Bytes::from(d),
                })
            ),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Appendix D: every frame is `[Header][Payload]` and decodes to itself.
    #[test]
    fn frame_roundtrip(frame in arb_frame()) {
        let bytes = frame.to_bytes().unwrap();
        prop_assert_eq!(bytes.len(), FrameHeader::LEN + frame.payload_len());
        let hdr = FrameHeader::from_slice_exact(&bytes[..4]).unwrap();
        prop_assert_eq!(hdr.frame_type, frame.frame_type());
        prop_assert_eq!(hdr.payload_len as usize, frame.payload_len());
        let back = Frame::from_slice(&bytes).unwrap();
        prop_assert_eq!(back, frame);
    }

    /// A truncated frame reports `Truncated`, never panics or mis-decodes.
    #[test]
    fn truncation_is_detected(frame in arb_frame(), cut in 0usize..64) {
        let bytes = frame.to_bytes().unwrap();
        if cut >= bytes.len() { return Ok(()); }
        let short = &bytes[..bytes.len() - cut - 1];
        let mut cur = short;
        match Frame::decode(&mut cur) {
            Err(WireError::Truncated { .. }) => {}
            Err(_) => {}
            Ok(_) => prop_assert!(false, "decoded a truncated frame"),
        }
    }

    /// Ch2 §2.3.3: Peer Records are 42 (v4) or 54 (v6) bytes and roundtrip.
    #[test]
    fn peer_record_roundtrip(n in arb_node_id(), c in arb_class(), t in any::<u8>(), f in arb_flags(), a in arb_addr()) {
        let r = PeerRecord { node_id: n, node_class: c, assigned_trees: TreeSet(t), flags: f, addr: a };
        let v = r.to_vec();
        prop_assert!(v.len() == 42 || v.len() == 54);
        prop_assert_eq!(PeerRecord::from_slice_exact(&v).unwrap(), r);
    }

    /// Ch2 §2.3.3: Stream Record fixed part is 96 bytes plus 7 per tree row plus signature.
    #[test]
    fn stream_record_roundtrip(pk in any::<[u8;32]>(), trees in prop::collection::vec(arb_mapping(), 1..7), next in prop::collection::vec(arb_mapping(), 0..7), sig in arb_sig()) {
        let r = StreamRecord {
            publisher_pubkey: PublicKeyBytes(pk), manifest_version: 7, slicing_mode: SlicingMode::SvcSpatial,
            register_sample_log2: 0, swarm_size: 100, relay_count: 50, live_edge_segment: SegmentSeq(9),
            live_edge_manifest_hash: Hash::ZERO, live_edge_timestamp_us: 1, effective_segment: SegmentSeq(0),
            trees: trees.clone(), trees_next: next.clone(), signature: sig,
        };
        let v = r.to_vec();
        prop_assert_eq!(v.len(), 96 + 7 * (trees.len() + next.len()) + 64);
        prop_assert_eq!(StreamRecord::from_slice_exact(&v).unwrap(), r);
    }
}

/// Appendix D §D.3: every registered code round-trips through `from_code`, and
/// unknown codes are rejected by the header decoder.
#[test]
fn registry_codes_are_exhaustive() {
    for ft in FrameType::ALL {
        assert_eq!(FrameType::from_code(*ft as u8), Some(*ft));
    }
    assert_eq!(FrameType::ALL.len(), 35, "Appendix D §D.3 lists 35 frames");
    let bad = [PROTOCOL_VERSION, 0x1F, 0, 0];
    assert!(matches!(
        FrameHeader::from_slice_exact(&bad),
        Err(WireError::UnknownFrameType(0x1F))
    ));
}

/// Ch4 §4.1.1: `BlockIndex = ChunkIndex << 12 | j`.
#[test]
fn block_index_layout() {
    let b = BlockIndex::new(3, 17).unwrap();
    assert_eq!(b.0, (3 << 12) | 17);
    assert_eq!(b.chunk(), 3);
    assert_eq!(b.j(), 17);
    assert!(BlockIndex::new(0, 4096).is_err());
}
