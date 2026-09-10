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
    assert len(vectors) == 19
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
    _, b = decode(vectors["disconnect_quit"])
    assert b["reason"] == "QUIT" and "tree_id" not in b
    _, b = decode(vectors["disconnect_rejected_not_assigned"])
    assert b["reason"] == "REJECTED_NOT_ASSIGNED" and b["tree_id"] == 3


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
