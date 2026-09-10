//! Peer Record, Stream Record and slicing-matrix entry (Ch2 §2.3.3).

use bytes::{Buf, BufMut};

use crate::codec::{get_u16, get_u32, get_u64, get_u8, Decode, Encode};
use crate::error::{Result, WireError};
use crate::types::{
    Hash, NodeClass, NodeId, PeerFlags, PublicKeyBytes, SegmentSeq, SignatureBytes, TreeId,
    TreeSet, WireAddr,
};

/// `[NodeID 32][NodeClass 1][AssignedTrees 1][Flags 1][AddrFamily 1][IP 4/16][Port 2]`
/// — 42 bytes for IPv4, 54 for IPv6.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PeerRecord {
    /// Peer identity.
    pub node_id: NodeId,
    /// Self-declared class.
    pub node_class: NodeClass,
    /// Trees the peer currently relays.
    pub assigned_trees: TreeSet,
    /// Reachability and role flags.
    pub flags: PeerFlags,
    /// Address as *observed* by the record's author.
    pub addr: WireAddr,
}

impl PeerRecord {
    /// Whether this peer can serve as a parent in `tree` according to its record.
    pub fn relays(&self, tree: TreeId) -> bool {
        self.node_class == NodeClass::Relay && self.assigned_trees.contains(tree)
    }
}

impl Encode for PeerRecord {
    fn encoded_len(&self) -> usize {
        32 + 3 + self.addr.wire_len()
    }
    fn encode<B: BufMut>(&self, buf: &mut B) {
        self.node_id.encode(buf);
        buf.put_u8(self.node_class as u8);
        buf.put_u8(self.assigned_trees.0);
        buf.put_u8(self.flags.to_byte());
        self.addr.encode(buf);
    }
}

impl Decode for PeerRecord {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        Ok(Self {
            node_id: NodeId::decode(buf)?,
            node_class: NodeClass::from_code(get_u8(buf, "PeerRecord.class")?)?,
            assigned_trees: TreeSet(get_u8(buf, "PeerRecord.assigned_trees")?),
            flags: PeerFlags::from_byte(get_u8(buf, "PeerRecord.flags")?)?,
            addr: WireAddr::decode(buf)?,
        })
    }
}

/// One row of the slicing matrix:
/// `[TreeID 1][Layer 1][StripeIndex 1][StripeCount 1][Priority 1][BitrateKbps 2]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TreeMappingEntry {
    /// Tree this row describes.
    pub tree_id: TreeId,
    /// Layer index the tree carries (lowest layer = 0).
    pub layer: u8,
    /// Stripe of the layer this tree carries.
    pub stripe_index: u8,
    /// Total stripes of the layer.
    pub stripe_count: u8,
    /// Shed priority: lower is shed first; equal for all trees of one layer.
    pub priority: u8,
    /// Declared per-tree bitrate.
    pub bitrate_kbps: u16,
}

impl TreeMappingEntry {
    /// Encoded length.
    pub const LEN: usize = 7;
}

impl Encode for TreeMappingEntry {
    fn encoded_len(&self) -> usize {
        Self::LEN
    }
    fn encode<B: BufMut>(&self, buf: &mut B) {
        buf.put_u8(self.tree_id.0);
        buf.put_u8(self.layer);
        buf.put_u8(self.stripe_index);
        buf.put_u8(self.stripe_count);
        buf.put_u8(self.priority);
        buf.put_u16(self.bitrate_kbps);
    }
}

impl Decode for TreeMappingEntry {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        Ok(Self {
            tree_id: TreeId(get_u8(buf, "TreeMappingEntry.tree_id")?),
            layer: get_u8(buf, "TreeMappingEntry.layer")?,
            stripe_index: get_u8(buf, "TreeMappingEntry.stripe_index")?,
            stripe_count: get_u8(buf, "TreeMappingEntry.stripe_count")?,
            priority: get_u8(buf, "TreeMappingEntry.priority")?,
            bitrate_kbps: get_u16(buf, "TreeMappingEntry.bitrate_kbps")?,
        })
    }
}

/// Slicing mode (`StreamRecord.SlicingMode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum SlicingMode {
    /// Scalable video coding, spatial layers.
    SvcSpatial = 0x01,
    /// Multiple description coding.
    Mdc = 0x02,
}

impl SlicingMode {
    fn from_code(c: u8) -> Result<Self> {
        match c {
            0x01 => Ok(Self::SvcSpatial),
            0x02 => Ok(Self::Mdc),
            v => Err(WireError::InvalidField {
                field: "SlicingMode",
                value: v as u64,
            }),
        }
    }
}

/// The publisher's signed Stream Record (canonical layout, Ch2 §2.3.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamRecord {
    /// Publisher key; `StreamID = Blake3(PublisherPubKey)`.
    pub publisher_pubkey: PublicKeyBytes,
    /// Monotonic record version.
    pub manifest_version: u32,
    /// SVC or MDC.
    pub slicing_mode: SlicingMode,
    /// Peers register with probability 2^-s.
    pub register_sample_log2: u8,
    /// Publisher-scaled swarm size.
    pub swarm_size: u32,
    /// Publisher-scaled relay count.
    pub relay_count: u32,
    /// Live-edge anchor (bootstrap hint only).
    pub live_edge_segment: SegmentSeq,
    /// Manifest hash at the live edge.
    pub live_edge_manifest_hash: Hash,
    /// Live-edge timestamp, µs.
    pub live_edge_timestamp_us: u64,
    /// Switch segment of a pending `MANIFEST_UPDATE`, or 0.
    pub effective_segment: SegmentSeq,
    /// Version of the STREAM_DESCRIPTOR in force (0 = none).
    pub descriptor_version: u32,
    /// Blake3 of the STREAM_DESCRIPTOR in force.
    pub descriptor_hash: Hash,
    /// Matrix in force.
    pub trees: Vec<TreeMappingEntry>,
    /// Pending matrix (empty when nothing is pending).
    pub trees_next: Vec<TreeMappingEntry>,
    /// Signature over every preceding byte.
    pub signature: SignatureBytes,
}

impl StreamRecord {
    /// Fixed part length before the matrices and signature.
    pub const FIXED_LEN: usize = 32 + 4 + 1 + 1 + 1 + 1 + 4 + 4 + 4 + 32 + 8 + 4 + 4 + 32;

    /// Bytes the publisher signature covers.
    pub fn signable_bytes(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(self.encoded_len() - 64);
        self.encode_unsigned(&mut v);
        v
    }

    fn encode_unsigned<B: BufMut>(&self, buf: &mut B) {
        self.publisher_pubkey.encode(buf);
        buf.put_u32(self.manifest_version);
        buf.put_u8(self.slicing_mode as u8);
        buf.put_u8(self.trees.len() as u8);
        buf.put_u8(self.register_sample_log2);
        buf.put_u8(self.trees_next.len() as u8);
        buf.put_u32(self.swarm_size);
        buf.put_u32(self.relay_count);
        buf.put_u32(self.live_edge_segment.0);
        self.live_edge_manifest_hash.encode(buf);
        buf.put_u64(self.live_edge_timestamp_us);
        buf.put_u32(self.effective_segment.0);
        buf.put_u32(self.descriptor_version);
        self.descriptor_hash.encode(buf);
        for t in &self.trees {
            t.encode(buf);
        }
        for t in &self.trees_next {
            t.encode(buf);
        }
    }
}

impl Encode for StreamRecord {
    fn encoded_len(&self) -> usize {
        Self::FIXED_LEN + (self.trees.len() + self.trees_next.len()) * TreeMappingEntry::LEN + 64
    }
    fn encode<B: BufMut>(&self, buf: &mut B) {
        self.encode_unsigned(buf);
        self.signature.encode(buf);
    }
}

impl Decode for StreamRecord {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        let publisher_pubkey = PublicKeyBytes::decode(buf)?;
        let manifest_version = get_u32(buf, "StreamRecord.manifest_version")?;
        let slicing_mode = SlicingMode::from_code(get_u8(buf, "StreamRecord.slicing_mode")?)?;
        let num_trees = get_u8(buf, "StreamRecord.num_trees")?;
        let register_sample_log2 = get_u8(buf, "StreamRecord.sample_log2")?;
        let num_trees_next = get_u8(buf, "StreamRecord.num_trees_next")?;
        let swarm_size = get_u32(buf, "StreamRecord.swarm_size")?;
        let relay_count = get_u32(buf, "StreamRecord.relay_count")?;
        let live_edge_segment = SegmentSeq(get_u32(buf, "StreamRecord.live_edge")?);
        let live_edge_manifest_hash = Hash::decode(buf)?;
        let live_edge_timestamp_us = get_u64(buf, "StreamRecord.live_edge_ts")?;
        let effective_segment = SegmentSeq(get_u32(buf, "StreamRecord.effective_seq")?);
        let descriptor_version = get_u32(buf, "StreamRecord.descriptor_version")?;
        let descriptor_hash = Hash::decode(buf)?;
        let mut trees = Vec::with_capacity(num_trees as usize);
        for _ in 0..num_trees {
            trees.push(TreeMappingEntry::decode(buf)?);
        }
        let mut trees_next = Vec::with_capacity(num_trees_next as usize);
        for _ in 0..num_trees_next {
            trees_next.push(TreeMappingEntry::decode(buf)?);
        }
        let signature = SignatureBytes::decode(buf)?;
        Ok(Self {
            publisher_pubkey,
            manifest_version,
            slicing_mode,
            register_sample_log2,
            swarm_size,
            relay_count,
            live_edge_segment,
            live_edge_manifest_hash,
            live_edge_timestamp_us,
            effective_segment,
            descriptor_version,
            descriptor_hash,
            trees,
            trees_next,
            signature,
        })
    }
}
