//! HyParView membership frames: FORWARD_JOIN (0x09), SHUFFLE (0x06),
//! GOSSIP_EXCHANGE (0x14) (Ch3 §3.1–3.2, App D §D.4.5, §D.4.10).

use bytes::{Buf, BufMut};

use crate::codec::{get_u32, get_u8, put_reserved, skip_reserved, Decode, Encode};
use crate::error::Result;
use crate::records::PeerRecord;
use crate::types::{Hash, NodeId, TreeSet};

/// FORWARD_JOIN (0x09): `[Origin PeerRecord 42/54][TTL 1][PR_gossip 1]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForwardJoin {
    /// The joiner, as observed by the contact that admitted it.
    pub origin: PeerRecord,
    /// Remaining hops of the random walk.
    pub ttl: u8,
    /// Hop count at which receivers file the origin into their Passive Set.
    pub pr_gossip: u8,
}
impl Encode for ForwardJoin {
    fn encoded_len(&self) -> usize {
        self.origin.encoded_len() + 2
    }
    fn encode<B: BufMut>(&self, buf: &mut B) {
        self.origin.encode(buf);
        buf.put_u8(self.ttl);
        buf.put_u8(self.pr_gossip);
    }
}
impl Decode for ForwardJoin {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        Ok(Self {
            origin: PeerRecord::decode(buf)?,
            ttl: get_u8(buf, "ForwardJoin.ttl")?,
            pr_gossip: get_u8(buf, "ForwardJoin.pr_gossip")?,
        })
    }
}

/// SHUFFLE (0x06): `[Origin NodeID 32][TTL 1][Count 1][WantedTrees 1][Reserved 1][PeerRecord × Count]`.
/// `ttl > 0` is a random walk with no reply; `ttl == 0` over a session is a direct
/// request answered with GOSSIP_EXCHANGE.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shuffle {
    /// Original sender.
    pub origin: NodeId,
    /// Remaining hops.
    pub ttl: u8,
    /// Trees the origin is short of relays for.
    pub wanted_trees: TreeSet,
    /// Records offered.
    pub records: Vec<PeerRecord>,
}
impl Encode for Shuffle {
    fn encoded_len(&self) -> usize {
        36 + self.records.iter().map(|r| r.encoded_len()).sum::<usize>()
    }
    fn encode<B: BufMut>(&self, buf: &mut B) {
        self.origin.encode(buf);
        buf.put_u8(self.ttl);
        buf.put_u8(self.records.len() as u8);
        buf.put_u8(self.wanted_trees.0);
        put_reserved(buf, 1);
        for r in &self.records {
            r.encode(buf);
        }
    }
}
impl Decode for Shuffle {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        let origin = NodeId::decode(buf)?;
        let ttl = get_u8(buf, "Shuffle.ttl")?;
        let n = get_u8(buf, "Shuffle.count")?;
        let wanted_trees = TreeSet(get_u8(buf, "Shuffle.wanted")?);
        skip_reserved(buf, 1, "Shuffle.reserved")?;
        let mut records = Vec::with_capacity(n as usize);
        for _ in 0..n {
            records.push(PeerRecord::decode(buf)?);
        }
        Ok(Self {
            origin,
            ttl,
            wanted_trees,
            records,
        })
    }
}

/// GOSSIP_EXCHANGE (0x14):
/// `[Sender NodeID 32][SequenceNumber 4][NeighborCount 1][WantedTrees 1][Reserved 2][PeerRecord × N][HMAC-Blake3 32]`.
/// The HMAC covers every preceding payload byte under the session key (Ch3 §3.2.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GossipExchange {
    /// Sender.
    pub sender: NodeId,
    /// Monotonic per-session sequence.
    pub sequence: u32,
    /// Trees the sender is short of relays for.
    pub wanted_trees: TreeSet,
    /// Records.
    pub records: Vec<PeerRecord>,
    /// Keyed Blake3 over the preceding bytes.
    pub hmac: Hash,
}
impl GossipExchange {
    /// Bytes the HMAC covers.
    pub fn authenticated_bytes(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(self.encoded_len() - 32);
        self.encode_unauthenticated(&mut v);
        v
    }
    fn encode_unauthenticated<B: BufMut>(&self, buf: &mut B) {
        self.sender.encode(buf);
        buf.put_u32(self.sequence);
        buf.put_u8(self.records.len() as u8);
        buf.put_u8(self.wanted_trees.0);
        put_reserved(buf, 2);
        for r in &self.records {
            r.encode(buf);
        }
    }
}
impl Encode for GossipExchange {
    fn encoded_len(&self) -> usize {
        40 + self.records.iter().map(|r| r.encoded_len()).sum::<usize>() + 32
    }
    fn encode<B: BufMut>(&self, buf: &mut B) {
        self.encode_unauthenticated(buf);
        self.hmac.encode(buf);
    }
}
impl Decode for GossipExchange {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        let sender = NodeId::decode(buf)?;
        let sequence = get_u32(buf, "GossipExchange.sequence")?;
        let n = get_u8(buf, "GossipExchange.count")?;
        let wanted_trees = TreeSet(get_u8(buf, "GossipExchange.wanted")?);
        skip_reserved(buf, 2, "GossipExchange.reserved")?;
        let mut records = Vec::with_capacity(n as usize);
        for _ in 0..n {
            records.push(PeerRecord::decode(buf)?);
        }
        let hmac = Hash::decode(buf)?;
        Ok(Self {
            sender,
            sequence,
            wanted_trees,
            records,
            hmac,
        })
    }
}
