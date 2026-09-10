"""Cross-check the Python decoder against the Rust golden vectors."""
import json
import pathlib

import pytest

from bitstream_tools import wire

VECTORS = pathlib.Path(__file__).resolve().parents[2] / "crates" / "bs-wire" / "tests" / "vectors" / "frames.json"


@pytest.fixture(scope="module")
def vectors():
    data = json.loads(VECTORS.read_text())
    return {v["name"]: v for v in data}


def decode(v):
    raw = bytes.fromhex(v["hex"])
    h, body = wire.decode_frame(raw)
    assert h.version == wire.PROTOCOL_VERSION
    assert h.frame_type == v["frame_type"]
    assert h.payload_len == len(raw) - 4
    return h, body


def test_every_vector_decodes(vectors):
    assert len(vectors) == 23
    for v in vectors.values():
        decode(v)


def vb_check(vb, seed):
    assert vb.node_id == bytes([seed]) * 32
    assert vb.public_key == bytes([seed + 1]) * 32
    assert vb.static_nonce == 0x0102030405060708
    assert vb.dynamic_nonce == 0x1112131415161718
    assert vb.timestamp_us == 1_700_000_000_000_000
    assert vb.signature == bytes([seed + 2]) * 64


def test_ping(vectors):
    _, b = decode(vectors["ping"])
    vb_check(b["validation"], 0x10)
    assert b["stream_id"] == b"\xaa" * 32


def test_pong(vectors):
    _, b = decode(vectors["pong_v4"])
    vb_check(b["validation"], 0x20)
    assert (b["reflected_ip"], b["reflected_port"]) == ("203.0.113.7", 40000)


def test_join(vectors):
    _, b = decode(vectors["join"])
    vb_check(b["validation"], 0x30)
    assert b["node_class"] == "LEAF"


def test_probe(vectors):
    _, b = decode(vectors["probe"])
    vb_check(b["validation"], 0x40)
    assert b["tree_id"] == 2


def test_probe_response(vectors):
    _, b = decode(vectors["probe_response"])
    assert b["sender"] == b"\x11" * 32
    assert b["k_avail"] == 6
    assert abs(b["reliability"] - 0.98) < 1e-4
    assert b["hop_count"] == 3
    assert b["node_class"] == "RELAY"
    assert b["flags"]["reachability"] == "CONE" and b["flags"]["relay_capable"]
    assert b["tree_state"] == "SERVING"
    assert b["assigned_trees"] == [2, 3]
    assert b["live_edge_segment"] == 41


def test_neighbor(vectors):
    _, b = decode(vectors["neighbor_tree_join"])
    assert b["priority"] == "HIGH" and b["tree_id"] == 2
    assert b["node_class"] == "RELAY" and b["assigned_trees"] == [3]


def test_accepted(vectors):
    _, b = decode(vectors["accepted"])
    assert b["accepted_type"] == "NEIGHBOR" and b["tree_id"] == 2
    assert b["hop_depth"] == 4 and b["pending"] is True


def test_disconnect(vectors):
    # App D sec. D.4.3b: always [Sender 32][Reason 1][TreeID 1][Reserved 2] = 36 bytes.
    h, b = decode(vectors["disconnect_quit"])
    assert h.payload_len == 36
    assert b["reason"] == "QUIT" and b["tree_id"] == 0 and b["is_rejection"] is False
    h, b = decode(vectors["disconnect_rejected_not_assigned"])
    assert h.payload_len == 36
    assert b["reason"] == "REJECTED_NOT_ASSIGNED" and b["tree_id"] == 3 and b["is_rejection"] is True


def test_drain_notice(vectors):
    _, b = decode(vectors["drain_notice"])
    assert (b["tree_id"], b["reason"], b["scope"], b["deadline_segment"]) == (1, "PREEMPTED", "THIS_CHILD", 47)


def test_stream_end(vectors):
    _, b = decode(vectors["stream_end"])
    assert b["final_segment"] == 1000 and b["timestamp_us"] == 1_700_000_100_000_000
    assert b["signature"] == b"\xee" * 64


def test_choke_state(vectors):
    _, b = decode(vectors["choke_state"])
    assert b["state"] == "UNCHOKE_OPTIMISTIC"


def test_manifest_request(vectors):
    _, b = decode(vectors["manifest_request_pending"])
    assert b["segment"] == 0 and b["selector"] == "PENDING_UPDATE"


def test_pull_request(vectors):
    _, b = decode(vectors["pull_request"])
    assert b["segment"] == 42 and (b["chunk"], b["j"]) == (2, 5)
    assert b["missing_symbols"] == 0 and b["urgency_ms"] == 800 and b["broadcast_want"]


def manifest_body_check(body):
    assert body["stream_id"] == b"\xaa" * 32
    assert body["segment"] == 42 and body["chunk_index"] == 2 and body["chunk_count"] == 4
    assert body["timestamp_us"] == 1_700_000_000_250_000
    assert body["merkle_root"] == b"\xcc" * 32
    assert body["chunk_byte_length"] == 187_500
    assert body["slicing_matrix_version"] == 3
    assert body["layer_block_counts"] == [3, 3, 6] and body["block_count"] == 12
    assert body["layer_byte_lengths"] == [46_875, 46_875, 93_750]


def test_manifest(vectors):
    _, b = decode(vectors["manifest"])
    manifest_body_check(b["body"])
    assert b["signature"] == b"\xdd" * 64


def test_manifest_update(vectors):
    _, b = decode(vectors["manifest_update"])
    manifest_body_check(b["body"])
    assert b["effective_segment"] == 47
    assert [t["bitrate_kbps"] for t in b["trees"]] == [1500, 1500, 3000]
    assert [t["layer"] for t in b["trees"]] == [0, 1, 2]
    assert [t["priority"] for t in b["trees"]] == [2, 1, 0]


def test_block_proof(vectors):
    _, b = decode(vectors["block_proof"])
    assert b["segment"] == 42 and (b["chunk"], b["j"]) == (2, 5) and b["sender_hop_depth"] == 3
    assert b["siblings"] == [bytes([i]) * 32 for i in (1, 2, 3, 4)]


def test_raptorq_symbol(vectors):
    _, b = decode(vectors["raptorq_symbol"])
    assert b["segment"] == 42 and b["sbn"] == (2 << 12) | 5 and b["esi"] == 17
    assert b["payload"] == b"\x5a" * 1024 and not b["is_source"]


def test_block_transmission(vectors):
    _, b = decode(vectors["block_transmission"])
    assert b["segment"] == 42 and b["block_index"] == (2 << 12) | 5
    assert b["siblings"] == [b"\x01" * 32, b"\x02" * 32]
    assert b["data"] == b"\x7b" * 64


def test_truncation_detected(vectors):
    raw = bytes.fromhex(vectors["manifest"]["hex"])
    with pytest.raises(wire.WireError):
        wire.decode_frame(raw[:-1])
    with pytest.raises(wire.WireError):
        wire.decode_frame(raw + b"\x00")


def test_register_peer(vectors):
    _, b = decode(vectors["register_peer"])
    assert b["stream_id"] == b"\xaa" * 32
    vb_check(b["validation"], 0x50)
    assert b["port"] == 4001
    assert b["protocol"] == "UDP"
    assert b["node_class"] == "RELAY"
    assert b["assigned_trees"] == [2]
    assert b["flags"]["reachability"] == "PUBLIC"
    assert b["flags"]["relay_capable"] is True
    assert b["flags"]["source"] is False
    assert b["signature"] == b"\x66" * 64


def test_get_peers_request(vectors):
    _, b = decode(vectors["get_peers_request"])
    assert b["direction"] == "request"
    vb_check(b["validation"], 0x60)
    assert b["stream_id"] == b"\xaa" * 32
    assert b["starved_trees"] == [3]
    assert b["wanted_trees"] == [2, 3]
    assert b["want_relay_capable"] is True


def test_get_peers_response(vectors):
    _, b = decode(vectors["get_peers_response"])
    assert b["direction"] == "response"
    assert b["guardian"] == b"\x77" * 32
    assert b["sampled"] is True
    recs = b["records"]
    assert len(recs) == 2
    assert recs[0].node_id == b"\x21" * 32
    assert recs[0].node_class == "RELAY"
    assert recs[0].assigned_trees == [1]
    assert recs[0].flags["source"] is True
    assert (recs[0].ip, recs[0].port) == ("10.0.0.1", 4000)
    assert recs[1].node_class == "LEAF"
    assert recs[1].assigned_trees == []
    assert recs[1].flags["reachability"] == "CONE"
    assert (recs[1].ip, recs[1].port) == ("10.0.0.2", 4002)
    sr = b["stream_record"]
    assert sr["publisher_pubkey"] == b"\x88" * 32
    assert sr["manifest_version"] == 9
    assert sr["slicing_mode"] == "SVC_SPATIAL"
    assert sr["register_sample_log2"] == 0
    assert sr["swarm_size"] == 120
    assert sr["relay_count"] == 80
    assert sr["live_edge_segment"] == 41
    assert sr["live_edge_manifest_hash"] == b"\xcc" * 32
    assert sr["live_edge_timestamp_us"] == 1_700_000_000_250_000
    assert sr["effective_segment"] == 0
    assert sr["descriptor_version"] == 1
    assert sr["descriptor_hash"] == b"\x5d" * 32
    assert [t["tree_id"] for t in sr["trees"]] == [1, 2, 3]
    assert [t["layer"] for t in sr["trees"]] == [0, 1, 2]
    assert [t["bitrate_kbps"] for t in sr["trees"]] == [1500, 1500, 3000]
    assert [t["priority"] for t in sr["trees"]] == [2, 1, 0]
    assert sr["trees_next"] == []
    assert sr["signature"] == b"\x99" * 64
    # Fixed part 132 B + 3 x 7 B + 64 B signature (Ch2 sec. 2.3.3, SOLUTION-060)
    raw = bytes.fromhex(vectors["get_peers_response"]["hex"])
    sr_len = int.from_bytes(raw[4 + 32 + 2:4 + 32 + 4], "big")
    assert sr_len == 132 + 3 * 7 + 64


def test_stream_descriptor(vectors):
    # App D sec. D.4.20
    h, b = decode(vectors["stream_descriptor"])
    assert h.frame_type == 0x1F and h.name == "STREAM_DESCRIPTOR"
    assert b["stream_id"] == b"\xaa" * 32
    assert b["version"] == 1 and b["effective_segment"] == 1
    assert b["content_type"] == "VIDEO_CMAF" and b["layer_mode"] == "SIMULCAST"
    assert len(b["layers"]) == 2
    l0, l1 = b["layers"]
    assert l0["codec_tag"] == "avc1" and (l0["width"], l0["height"]) == (640, 360)
    assert l0["frame_rate_milli"] == 24_000 and l0["bitrate_kbps"] == 800
    assert l0["init_data"] == b"\x1a" * 16
    assert l1["codec_tag"] == "avc1" and (l1["width"], l1["height"]) == (1280, 720)
    assert l1["bitrate_kbps"] == 2500 and l1["init_data"] == b"\x2b" * 16
    assert b["signature"] == b"\xd5" * 64
    # 32 + 4 + 4 + 4 + 2 x (4+2+2+4+2+2+16) + 64
    assert h.payload_len == 44 + 2 * 32 + 64


def test_manifest_request_descriptor_selector():
    from bitstream_tools.wire import decode_frame
    raw = bytes([0x01, 0x1D, 0x00, 0x08]) + (0).to_bytes(4, "big") + bytes([0xFD, 0, 0, 0])
    _, b = decode_frame(raw)
    assert b["selector"] == "DESCRIPTOR"
