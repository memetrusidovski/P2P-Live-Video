"""Independent decoder for BitStream wire frames.

Written from the byte diagrams in protocol/appendix_d_frame_registry.md and the
chapter layouts it references, NOT from the Rust code, so that the two
implementations cross-check each other against the golden vectors.

All integers are big-endian. Every frame is [Version 1][Type 1][PayloadLen 2]
followed by PayloadLen bytes. Pre-session UDP frames carry the 152-byte
validation block immediately after the header.
"""
from __future__ import annotations

import struct
from dataclasses import dataclass, field
from typing import Any

PROTOCOL_VERSION = 0x01

FRAME_NAMES = {
    0x01: "PING", 0x02: "PONG", 0x03: "JOIN", 0x04: "STREAM_END", 0x05: "NEIGHBOR",
    0x06: "SHUFFLE", 0x07: "DISCONNECT", 0x08: "ACCEPTED", 0x09: "FORWARD_JOIN",
    0x0A: "REGISTER_PEER", 0x0B: "GET_PEERS", 0x0C: "FIND_NODE", 0x0D: "FIND_VALUE",
    0x0E: "PROBE", 0x0F: "PROBE_RESPONSE", 0x10: "BLOCK_TRANSMISSION", 0x11: "MANIFEST",
    0x12: "RAPTORQ_SYMBOL", 0x13: "BLOCK_PROOF", 0x14: "GOSSIP_EXCHANGE",
    0x15: "BUFFER_STATE_BITFIELD", 0x16: "PULL_REQUEST", 0x17: "MANIFEST_UPDATE",
    0x18: "ROSTER", 0x19: "RANK_PROOF", 0x1A: "STORE_RECORD", 0x1B: "STORE_RECORD_ACK",
    0x1C: "PUNCH_REQUEST", 0x1D: "MANIFEST_REQUEST", 0x1E: "DRAIN_NOTICE",
    0x20: "PROOF_OF_UPLOAD", 0x21: "CHOKE_STATE", 0x22: "REPUTATION_AUDIT_GOSSIP",
    0x30: "RELAY_PROPOSAL", 0x31: "RELAY_BIND",
}

NODE_CLASS = {0x00: "RELAY", 0x01: "LEAF", 0x02: "LEAF_PRIVATE"}
REACHABILITY = {0b00: "PUBLIC", 0b01: "CONE", 0b10: "SYMMETRIC"}
TREE_STATE = {0b00: "SERVING", 0b01: "WARMING", 0b10: "UNPARENTED"}
DISCONNECT_REASON = {
    0x01: "CHOKE", 0x02: "QUIT", 0x03: "EVICTION", 0x04: "PREEMPTED", 0x05: "DISPLACED",
    0x06: "REASSIGNED", 0x07: "DEMOTED", 0x08: "CAPACITY", 0x09: "DEPTH",
    # ISSUE-059 implementation extension: join rejections
    0x0A: "REJECTED_SATURATED", 0x0B: "REJECTED_NOT_ASSIGNED", 0x0C: "REJECTED_DEPTH",
}
REJECTION_REASONS = {0x0A, 0x0B, 0x0C}

VALIDATION_BLOCK_LEN = 152
SYMBOL_SIZE = 1024


class WireError(ValueError):
    pass


class Cursor:
    def __init__(self, data: bytes, pos: int = 0):
        self.data = data
        self.pos = pos

    def remaining(self) -> int:
        return len(self.data) - self.pos

    def take(self, n: int, what: str = "bytes") -> bytes:
        if self.remaining() < n:
            raise WireError(f"truncated while reading {what}: need {n}, have {self.remaining()}")
        out = self.data[self.pos:self.pos + n]
        self.pos += n
        return out

    def u8(self, what="u8") -> int:
        return self.take(1, what)[0]

    def u16(self, what="u16") -> int:
        return struct.unpack(">H", self.take(2, what))[0]

    def u32(self, what="u32") -> int:
        return struct.unpack(">I", self.take(4, what))[0]

    def u64(self, what="u64") -> int:
        return struct.unpack(">Q", self.take(8, what))[0]

    def skip(self, n: int) -> None:
        self.take(n, "reserved")

    def done(self) -> None:
        if self.remaining():
            raise WireError(f"{self.remaining()} trailing bytes")


@dataclass
class Header:
    version: int
    frame_type: int
    payload_len: int

    @property
    def name(self) -> str:
        return FRAME_NAMES.get(self.frame_type, f"UNKNOWN_{self.frame_type:#04x}")


def decode_header(c: Cursor) -> Header:
    version = c.u8("version")
    if version != PROTOCOL_VERSION:
        raise WireError(f"unsupported version {version}")
    ft = c.u8("type")
    if ft not in FRAME_NAMES:
        raise WireError(f"unknown frame type {ft:#04x}")
    return Header(version, ft, c.u16("payload_len"))


@dataclass
class ValidationBlock:
    node_id: bytes
    public_key: bytes
    static_nonce: int
    dynamic_nonce: int
    timestamp_us: int
    signature: bytes


def decode_validation_block(c: Cursor) -> ValidationBlock:
    # Ch2 sec. 2.2.3: [NodeID 32][PK 32][StaticNonce 8][DynamicNonce 8][Timestamp 8][Sig 64]
    return ValidationBlock(
        node_id=c.take(32, "node_id"),
        public_key=c.take(32, "public_key"),
        static_nonce=c.u64("static_nonce"),
        dynamic_nonce=c.u64("dynamic_nonce"),
        timestamp_us=c.u64("timestamp"),
        signature=c.take(64, "signature"),
    )


def decode_addr(c: Cursor) -> tuple[str, int]:
    # [AddrFamily 1][IP 4/16][Port 2]
    fam = c.u8("addr_family")
    if fam == 0x04:
        ip = ".".join(str(b) for b in c.take(4, "ipv4"))
    elif fam == 0x06:
        raw = c.take(16, "ipv6")
        ip = ":".join(f"{struct.unpack('>H', raw[i:i+2])[0]:x}" for i in range(0, 16, 2))
    else:
        raise WireError(f"bad address family {fam}")
    return ip, c.u16("port")


def decode_peer_flags(b: int) -> dict[str, Any]:
    reach = b & 0b11
    if reach not in REACHABILITY:
        raise WireError("reserved reachability value")
    return {
        "reachability": REACHABILITY[reach],
        "relay_capable": bool(b & (1 << 2)),
        "source": bool(b & (1 << 3)),
        "source_ingress": bool(b & (1 << 4)),
    }


def tree_set(b: int) -> list[int]:
    return [m for m in range(1, 9) if b & (1 << (m - 1))]


@dataclass
class PeerRecord:
    node_id: bytes
    node_class: str
    assigned_trees: list[int]
    flags: dict[str, Any]
    ip: str
    port: int


def decode_peer_record(c: Cursor) -> PeerRecord:
    # Ch2 sec. 2.3.3: [NodeID 32][NodeClass 1][AssignedTrees 1][Flags 1][AddrFamily 1][IP][Port 2]
    node_id = c.take(32, "node_id")
    cls = NODE_CLASS[c.u8("node_class")]
    trees = tree_set(c.u8("assigned_trees"))
    flags = decode_peer_flags(c.u8("flags"))
    ip, port = decode_addr(c)
    return PeerRecord(node_id, cls, trees, flags, ip, port)


def _manifest_body(c: Cursor) -> dict[str, Any]:
    # App D sec. D.4.8
    body = {
        "stream_id": c.take(32, "stream_id"),
        "segment": c.u32("segment"),
        "chunk_index": c.u8("chunk_index"),
        "chunk_count": c.u8("chunk_count"),
    }
    c.skip(2)
    body["timestamp_us"] = c.u64("timestamp")
    body["merkle_root"] = c.take(32, "merkle_root")
    block_count = c.u16("block_count")
    body["chunk_byte_length"] = c.u32("chunk_byte_length")
    body["slicing_matrix_version"] = c.u8("matrix_version")
    layer_count = c.u8("layer_count")
    c.skip(2)
    body["layer_block_counts"] = [c.u16("layer_block_count") for _ in range(layer_count)]
    if sum(body["layer_block_counts"]) != block_count:
        raise WireError("BlockCount != sum(LayerBlockCount)")
    body["block_count"] = block_count
    return body


def _tree_mapping_entry(c: Cursor) -> dict[str, int]:
    return {
        "tree_id": c.u8(), "layer": c.u8(), "stripe_index": c.u8(),
        "stripe_count": c.u8(), "priority": c.u8(), "bitrate_kbps": c.u16(),
    }


def decode_payload(ft: int, payload: bytes) -> dict[str, Any]:
    c = Cursor(payload)
    name = FRAME_NAMES[ft]
    out: dict[str, Any] = {"frame": name}

    if name == "PING":
        out["validation"] = decode_validation_block(c)
        out["stream_id"] = c.take(32)
    elif name == "PONG":
        out["validation"] = decode_validation_block(c)
        out["reflected_ip"], out["reflected_port"] = decode_addr(c)
    elif name == "JOIN":
        out["validation"] = decode_validation_block(c)
        out["stream_id"] = c.take(32)
        out["node_class"] = NODE_CLASS[c.u8()]
        c.skip(3)
    elif name == "PROBE":
        out["validation"] = decode_validation_block(c)
        out["stream_id"] = c.take(32)
        out["tree_id"] = c.u8()
    elif name == "PROBE_RESPONSE":
        out["sender"] = c.take(32)
        out["k_avail"] = c.u16()
        out["reliability"] = c.u16() / 65535.0
        out["hop_count"] = c.u8()
        out["node_class"] = NODE_CLASS[c.u8()]
        fb = c.u8()
        out["flags"] = decode_peer_flags(fb)
        ts = (fb >> 5) & 0b11
        if ts not in TREE_STATE:
            raise WireError("reserved TreeState")
        out["tree_state"] = TREE_STATE[ts]
        out["assigned_trees"] = tree_set(c.u8())
        out["live_edge_segment"] = c.u32()
    elif name == "NEIGHBOR":
        out["sender"] = c.take(32)
        out["priority"] = {0x01: "HIGH", 0x02: "LOW"}[c.u8()]
        out["tree_id"] = c.u8()
        out["node_class"] = NODE_CLASS[c.u8()]
        out["assigned_trees"] = tree_set(c.u8())
    elif name == "ACCEPTED":
        out["sender"] = c.take(32)
        out["accepted_type"] = {0x03: "JOIN", 0x05: "NEIGHBOR"}[c.u8()]
        out["tree_id"] = c.u8()
        out["hop_depth"] = c.u8()
        out["pending"] = bool(c.u8() & 1)
    elif name == "DISCONNECT":
        out["sender"] = c.take(32)
        code = c.u8()
        out["reason"] = DISCONNECT_REASON[code]
        if code in REJECTION_REASONS:
            out["tree_id"] = c.u8()
    elif name == "DRAIN_NOTICE":
        out["tree_id"] = c.u8()
        out["reason"] = DISCONNECT_REASON[c.u8()]
        out["scope"] = {0x00: "THIS_CHILD", 0x01: "ALL_CHILDREN"}[c.u8()]
        c.skip(1)
        out["deadline_segment"] = c.u32()
    elif name == "STREAM_END":
        out["stream_id"] = c.take(32)
        out["final_segment"] = c.u32()
        out["timestamp_us"] = c.u64()
        out["signature"] = c.take(64)
    elif name == "CHOKE_STATE":
        out["state"] = {0: "CHOKE", 1: "UNCHOKE", 2: "UNCHOKE_OPTIMISTIC"}[c.u8()]
        c.skip(3)
    elif name == "MANIFEST_REQUEST":
        out["segment"] = c.u32()
        ci = c.u8()
        out["selector"] = {0xFF: "ALL_CHUNKS", 0xFE: "PENDING_UPDATE"}.get(ci, ci)
        c.skip(3)
    elif name == "PULL_REQUEST":
        out["segment"] = c.u32()
        bi = c.u16()
        out["block_index"] = bi
        out["chunk"], out["j"] = bi >> 12, bi & 0xFFF
        out["missing_symbols"] = c.u8()
        out["urgency_ms"] = c.u16()
        out["broadcast_want"] = bool(c.u8() & 1)
        c.skip(2)
    elif name == "MANIFEST":
        out["body"] = _manifest_body(c)
        out["signature"] = c.take(64)
    elif name == "MANIFEST_UPDATE":
        out["body"] = _manifest_body(c)
        out["effective_segment"] = c.u32()
        n = c.u8()
        out["trees"] = [_tree_mapping_entry(c) for _ in range(n)]
        out["signature"] = c.take(64)
    elif name == "BLOCK_PROOF":
        out["segment"] = c.u32()
        bi = c.u16()
        out["block_index"] = bi
        out["chunk"], out["j"] = bi >> 12, bi & 0xFFF
        depth = c.u8()
        out["sender_hop_depth"] = c.u8()
        out["siblings"] = [c.take(32) for _ in range(depth)]
    elif name == "RAPTORQ_SYMBOL":
        out["segment"] = c.u32()
        out["sbn"] = c.u16()
        out["esi"] = c.u16()
        out["payload"] = c.take(SYMBOL_SIZE)
        out["is_source"] = out["esi"] < 16
    elif name == "BLOCK_TRANSMISSION":
        out["segment"] = c.u32()
        out["block_index"] = c.u16()
        h = c.u16()
        out["siblings"] = [c.take(32) for _ in range(h)]
        out["data"] = c.take(c.remaining())
    else:
        out["raw"] = payload
        return out
    c.done()
    return out


def decode_frame(data: bytes) -> tuple[Header, dict[str, Any]]:
    """Decode exactly one frame from `data`; trailing bytes are an error."""
    c = Cursor(data)
    h = decode_header(c)
    payload = c.take(h.payload_len, "payload")
    c.done()
    return h, decode_payload(h.frame_type, payload)
