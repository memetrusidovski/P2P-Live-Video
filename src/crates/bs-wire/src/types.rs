//! Primitive wire types shared by many frames.

use core::fmt;
use core::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use bytes::{Buf, BufMut};

use crate::codec::{get_array, get_u16, get_u8, Decode, Encode};
use crate::consts::{HASH_LEN, PUBKEY_LEN, SIGNATURE_LEN};
use crate::error::{Result, WireError};

macro_rules! byte_array_newtype {
    ($(#[$doc:meta])* $name:ident, $len:expr) => {
        $(#[$doc])*
        #[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(pub [u8; $len]);

        impl $name {
            /// Length in bytes.
            pub const LEN: usize = $len;
            /// The all-zero value ("none" / "generic" where the spec says so).
            pub const ZERO: Self = Self([0u8; $len]);
            /// Borrow the bytes.
            pub fn as_bytes(&self) -> &[u8; $len] { &self.0 }
            /// Whether every byte is zero.
            pub fn is_zero(&self) -> bool { self.0.iter().all(|b| *b == 0) }
            /// Short hex prefix for logs.
            pub fn short(&self) -> String {
                self.0[..4].iter().map(|b| format!("{:02x}", b)).collect()
            }
        }
        impl From<[u8; $len]> for $name { fn from(b: [u8; $len]) -> Self { Self(b) } }
        impl AsRef<[u8]> for $name { fn as_ref(&self) -> &[u8] { &self.0 } }
        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({}…)", stringify!($name), self.short())
            }
        }
        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                for b in &self.0 { write!(f, "{:02x}", b)?; }
                Ok(())
            }
        }
        impl Encode for $name {
            fn encoded_len(&self) -> usize { $len }
            fn encode<B: BufMut>(&self, buf: &mut B) { buf.put_slice(&self.0); }
        }
        impl Decode for $name {
            fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
                Ok(Self(get_array::<$len, _>(buf, stringify!($name))?))
            }
        }
        #[cfg(feature = "serde")]
        impl serde::Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, s: S) -> core::result::Result<S::Ok, S::Error> {
                s.serialize_str(&hex::encode(self.0))
            }
        }
        #[cfg(feature = "serde")]
        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> core::result::Result<Self, D::Error> {
                let st = <alloc_string::String as serde::Deserialize>::deserialize(d)?;
                let v = hex::decode(&st).map_err(serde::de::Error::custom)?;
                let arr: [u8; $len] = v.try_into().map_err(|_| serde::de::Error::custom("wrong length"))?;
                Ok(Self(arr))
            }
        }
    };
}

#[cfg(feature = "serde")]
mod alloc_string {
    pub use std::string::String;
}

byte_array_newtype!(
    /// 256-bit S/Kademlia node identifier: `Blake3(PK_node ‖ N_static)`.
    NodeId, HASH_LEN
);
byte_array_newtype!(
    /// Stream identifier `K_s = Blake3(PublisherPubKey)`.
    StreamId, HASH_LEN
);
byte_array_newtype!(
    /// A Blake3 digest (Merkle node, manifest root, …).
    Hash, HASH_LEN
);
byte_array_newtype!(
    /// Ed25519 public key bytes.
    PublicKeyBytes, PUBKEY_LEN
);
byte_array_newtype!(
    /// Ed25519 signature bytes.
    SignatureBytes, SIGNATURE_LEN
);

/// Node class (Ch1 §1.2.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum NodeClass {
    /// Interior in assigned tree(s), leaf elsewhere.
    Relay = 0x00,
    /// Leaf in every tree by self-declaration.
    Leaf = 0x01,
    /// Leaf in every tree, onion-routed.
    LeafPrivate = 0x02,
}

impl NodeClass {
    /// Parse a code.
    pub fn from_code(c: u8) -> Result<Self> {
        match c {
            0x00 => Ok(Self::Relay),
            0x01 => Ok(Self::Leaf),
            0x02 => Ok(Self::LeafPrivate),
            v => Err(WireError::InvalidField {
                field: "NodeClass",
                value: v as u64,
            }),
        }
    }
    /// Whether this is one of the leaf classes.
    pub fn is_leaf(self) -> bool {
        !matches!(self, Self::Relay)
    }
}

/// NAT reachability (Peer Record `Flags` bits 0–1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum Reachability {
    /// No NAT or full cone.
    #[default]
    Public = 0b00,
    /// Restricted / port-restricted: reachable after a hole punch.
    Cone = 0b01,
    /// Symmetric / CGNAT: not reachable inbound.
    Symmetric = 0b10,
}

impl Reachability {
    fn from_bits(b: u8) -> Result<Self> {
        match b & 0b11 {
            0b00 => Ok(Self::Public),
            0b01 => Ok(Self::Cone),
            0b10 => Ok(Self::Symmetric),
            v => Err(WireError::InvalidField {
                field: "Reachability",
                value: v as u64,
            }),
        }
    }
}

/// Peer Record `Flags` byte (Ch2 §2.3.3), bits 0–4. `PROBE_RESPONSE` reuses bits
/// 0–4 and adds `TreeState` in bits 5–6.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PeerFlags {
    /// Bits 0–1.
    pub reachability: Reachability,
    /// Bit 2: meets emergent-relay criteria and accepts `RELAY_BIND`.
    pub relay_capable: bool,
    /// Bit 3: this peer is the broadcaster.
    pub source: bool,
    /// Bit 4: ingress relay bound by a NAT-blocked broadcaster.
    pub source_ingress: bool,
}

impl PeerFlags {
    /// Flags for a plain public peer.
    pub const PUBLIC: PeerFlags = PeerFlags {
        reachability: Reachability::Public,
        relay_capable: false,
        source: false,
        source_ingress: false,
    };
    /// Pack bits 0–4; bits 5–7 zero.
    pub fn to_byte(&self) -> u8 {
        (self.reachability as u8)
            | ((self.relay_capable as u8) << 2)
            | ((self.source as u8) << 3)
            | ((self.source_ingress as u8) << 4)
    }
    /// Unpack bits 0–4, ignoring 5–7.
    pub fn from_byte(b: u8) -> Result<Self> {
        Ok(Self {
            reachability: Reachability::from_bits(b)?,
            relay_capable: b & (1 << 2) != 0,
            source: b & (1 << 3) != 0,
            source_ingress: b & (1 << 4) != 0,
        })
    }
}

/// One bit per tree: bit `m-1` set means tree `T_m`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TreeSet(pub u8);

impl TreeSet {
    /// No trees.
    pub const EMPTY: TreeSet = TreeSet(0);
    /// Set containing exactly `tree`.
    pub fn single(tree: TreeId) -> Self {
        let mut s = Self::EMPTY;
        s.insert(tree);
        s
    }
    /// All trees `1..=m`.
    pub fn first_n(m: u8) -> Self {
        let m = m.min(8);
        if m == 0 {
            Self::EMPTY
        } else {
            TreeSet(((1u16 << m) - 1) as u8)
        }
    }
    /// Whether `tree` is present.
    pub fn contains(&self, tree: TreeId) -> bool {
        (1..=8).contains(&tree.0) && self.0 & (1 << (tree.0 - 1)) != 0
    }
    /// Insert `tree`; trees outside 1..=8 are ignored.
    pub fn insert(&mut self, tree: TreeId) {
        if (1..=8).contains(&tree.0) {
            self.0 |= 1 << (tree.0 - 1);
        }
    }
    /// Remove `tree`.
    pub fn remove(&mut self, tree: TreeId) {
        if (1..=8).contains(&tree.0) {
            self.0 &= !(1 << (tree.0 - 1));
        }
    }
    /// Number of trees.
    pub fn len(&self) -> usize {
        self.0.count_ones() as usize
    }
    /// Whether empty.
    pub fn is_empty(&self) -> bool {
        self.0 == 0
    }
    /// Iterate tree ids in ascending order.
    pub fn iter(&self) -> impl Iterator<Item = TreeId> + '_ {
        (1u8..=8).filter(|m| self.contains(TreeId(*m))).map(TreeId)
    }
    /// Set intersection.
    pub fn intersection(&self, other: TreeSet) -> TreeSet {
        TreeSet(self.0 & other.0)
    }
    /// Set union.
    pub fn union(&self, other: TreeSet) -> TreeSet {
        TreeSet(self.0 | other.0)
    }
}

impl fmt::Debug for TreeSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Trees{{")?;
        for (i, t) in self.iter().enumerate() {
            if i > 0 {
                write!(f, ",")?;
            }
            write!(f, "{}", t.0)?;
        }
        write!(f, "}}")
    }
}

/// Tree identifier. `0` means "membership only / no tree"; real trees start at `1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TreeId(pub u8);

impl TreeId {
    /// "No tree / membership only".
    pub const NONE: TreeId = TreeId(0);
    /// Tree 1 always carries the base layer.
    pub const BASE: TreeId = TreeId(1);
    /// Whether this names a real tree.
    pub fn is_tree(self) -> bool {
        self.0 >= 1
    }
    /// Zero-based index for array storage.
    pub fn index(self) -> usize {
        self.0.saturating_sub(1) as usize
    }
}

impl fmt::Display for TreeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "T{}", self.0)
    }
}

/// Segment sequence number. Numbering starts at `1`; `0` means "none".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SegmentSeq(pub u32);

impl SegmentSeq {
    /// "No segment".
    pub const NONE: SegmentSeq = SegmentSeq(0);
    /// First real segment.
    pub const FIRST: SegmentSeq = SegmentSeq(1);
    /// Next segment.
    pub fn next(self) -> SegmentSeq {
        SegmentSeq(self.0.wrapping_add(1))
    }
    /// `self + n`.
    pub fn plus(self, n: u32) -> SegmentSeq {
        SegmentSeq(self.0.wrapping_add(n))
    }
    /// Whether this is a real segment.
    pub fn is_some(self) -> bool {
        self.0 != 0
    }
}

impl fmt::Display for SegmentSeq {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "S{}", self.0)
    }
}

/// Global block index within a segment: `ChunkIndex << 12 | j` (Ch4 §4.1.1).
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BlockIndex(pub u16);

impl BlockIndex {
    /// Build from chunk index and in-chunk index `j`.
    pub fn new(chunk: u8, j: u16) -> Result<Self> {
        if j as usize >= crate::consts::MAX_BLOCKS_PER_CHUNK {
            return Err(WireError::InvalidField {
                field: "BlockIndex.j",
                value: j as u64,
            });
        }
        if chunk as u32 >= (1 << (16 - crate::consts::BLOCK_INDEX_CHUNK_SHIFT)) {
            return Err(WireError::InvalidField {
                field: "BlockIndex.chunk",
                value: chunk as u64,
            });
        }
        Ok(Self(
            ((chunk as u16) << crate::consts::BLOCK_INDEX_CHUNK_SHIFT) | j,
        ))
    }
    /// Chunk index (upper 4 bits).
    pub fn chunk(self) -> u8 {
        (self.0 >> crate::consts::BLOCK_INDEX_CHUNK_SHIFT) as u8
    }
    /// In-chunk block number `j` (lower 12 bits).
    pub fn j(self) -> u16 {
        self.0 & crate::consts::BLOCK_INDEX_J_MASK
    }
}

impl fmt::Debug for BlockIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "B(c{} j{})", self.chunk(), self.j())
    }
}
impl fmt::Display for BlockIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "c{}j{}", self.chunk(), self.j())
    }
}

/// `[AddrFamily 1B][IP 4/16B][Port 2B]` as used in PONG and Peer Records.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WireAddr(pub SocketAddr);

impl WireAddr {
    /// Family code for IPv4.
    pub const FAMILY_V4: u8 = 0x04;
    /// Family code for IPv6.
    pub const FAMILY_V6: u8 = 0x06;
    /// Encoded length for this family.
    pub fn wire_len(&self) -> usize {
        match self.0 {
            SocketAddr::V4(_) => 7,
            SocketAddr::V6(_) => 19,
        }
    }
}

impl Encode for WireAddr {
    fn encoded_len(&self) -> usize {
        self.wire_len()
    }
    fn encode<B: BufMut>(&self, buf: &mut B) {
        match self.0 {
            SocketAddr::V4(a) => {
                buf.put_u8(Self::FAMILY_V4);
                buf.put_slice(&a.ip().octets());
                buf.put_u16(a.port());
            }
            SocketAddr::V6(a) => {
                buf.put_u8(Self::FAMILY_V6);
                buf.put_slice(&a.ip().octets());
                buf.put_u16(a.port());
            }
        }
    }
}

impl Decode for WireAddr {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        let fam = get_u8(buf, "WireAddr.family")?;
        let ip = match fam {
            Self::FAMILY_V4 => IpAddr::V4(Ipv4Addr::from(get_array::<4, _>(buf, "WireAddr.v4")?)),
            Self::FAMILY_V6 => IpAddr::V6(Ipv6Addr::from(get_array::<16, _>(buf, "WireAddr.v6")?)),
            v => {
                return Err(WireError::InvalidField {
                    field: "AddrFamily",
                    value: v as u64,
                })
            }
        };
        let port = get_u16(buf, "WireAddr.port")?;
        Ok(Self(SocketAddr::new(ip, port)))
    }
}

/// `DISCONNECT` / `DRAIN_NOTICE` reason codes (Appendix D §D.3).
///
/// Codes `0x0A..=0x0C` implement the proposed fix of **ISSUE-059**: the spec's join
/// algorithm returns `REJECTED` / `REJECTED_NOT_ASSIGNED` but defines no frame that
/// carries a rejection. A parent answers a refused `NEIGHBOR(TreeID>0)` with a
/// `DISCONNECT` carrying one of these codes on the same stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum DisconnectReason {
    /// Child leaves after a warm handover (`DISCONNECT_CHOKE`).
    Choke = 0x01,
    /// Graceful quit.
    Quit = 0x02,
    /// Evicted for misbehaviour or timeout.
    Eviction = 0x03,
    /// Rank admission preempted the child.
    Preempted = 0x04,
    /// Leaf share displaced the child.
    Displaced = 0x05,
    /// Parent moves at a forest resize.
    Reassigned = 0x06,
    /// Parent demoted RELAY → LEAF.
    Demoted = 0x07,
    /// Parent's slot count fell.
    Capacity = 0x08,
    /// Parent's depth reached `D_max`.
    Depth = 0x09,
    /// ISSUE-059: join refused, no free slot and no admission rule applied.
    RejectedSaturated = 0x0A,
    /// ISSUE-059: join refused, responder does not relay the requested tree.
    RejectedNotAssigned = 0x0B,
    /// ISSUE-059: join refused, the child would sit at `>= D_max`.
    RejectedDepth = 0x0C,
}

impl DisconnectReason {
    /// Parse a code.
    pub fn from_code(c: u8) -> Result<Self> {
        use DisconnectReason::*;
        Ok(match c {
            0x01 => Choke,
            0x02 => Quit,
            0x03 => Eviction,
            0x04 => Preempted,
            0x05 => Displaced,
            0x06 => Reassigned,
            0x07 => Demoted,
            0x08 => Capacity,
            0x09 => Depth,
            0x0A => RejectedSaturated,
            0x0B => RejectedNotAssigned,
            0x0C => RejectedDepth,
            v => {
                return Err(WireError::InvalidField {
                    field: "DisconnectReason",
                    value: v as u64,
                })
            }
        })
    }
    /// Whether this code is a join rejection.
    pub fn is_rejection(self) -> bool {
        matches!(
            self,
            Self::RejectedSaturated | Self::RejectedNotAssigned | Self::RejectedDepth
        )
    }
}

/// Per-tree state in `PROBE_RESPONSE.Flags` bits 5–6.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum TreeState {
    /// Has a parent and ≥ 1 verified segment in the tree.
    Serving = 0b00,
    /// Has a parent, nothing verified yet.
    Warming = 0b01,
    /// Assigned but currently without a parent.
    Unparented = 0b10,
}

impl TreeState {
    /// Parse from bits 5–6 of a flags byte.
    pub fn from_flags_byte(b: u8) -> Result<Self> {
        match (b >> 5) & 0b11 {
            0b00 => Ok(Self::Serving),
            0b01 => Ok(Self::Warming),
            0b10 => Ok(Self::Unparented),
            v => Err(WireError::InvalidField {
                field: "TreeState",
                value: v as u64,
            }),
        }
    }
    /// Bits 5–6 value.
    pub fn to_flags_bits(self) -> u8 {
        (self as u8) << 5
    }
}
