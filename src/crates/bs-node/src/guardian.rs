//! Bootstrap guardian: the M2 stand-in for the S/Kademlia guardians. Lives
//! inside `bsnode publish`, answers `GET_PEERS` with a random sample of
//! registrations plus the publisher's signed Stream Record, and accepts
//! `REGISTER_PEER`. Addresses are always the **observed** UDP source (Ch2
//! §2.2.3), never a self-declared field.

use std::collections::HashMap;
use std::net::SocketAddr;

use bs_crypto::pow::Difficulty;
use bs_crypto::{Identity, Verifier};
use bs_wire::frames::{GetPeers, GetPeersResponse, RegisterPeer};
use bs_wire::{FrameType, NodeId, PeerRecord, StreamId, StreamRecord, WireAddr};
use rand::seq::SliceRandom;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

/// Registration with its expiry.
#[derive(Debug, Clone)]
struct Reg {
    record: PeerRecord,
    last_seen_us: u64,
}

/// The guardian.
pub struct Guardian {
    stream_id: StreamId,
    node_id: NodeId,
    difficulty: Difficulty,
    regs: HashMap<NodeId, Reg>,
    /// Latest Stream Record.
    pub stream_record: Option<StreamRecord>,
    ttl_us: u64,
    rng: ChaCha8Rng,
    /// Queries served.
    pub queries: u64,
    /// Registrations accepted.
    pub registrations: u64,
}

impl Guardian {
    /// New guardian for `stream_id`.
    pub fn new(
        stream_id: StreamId,
        node_id: NodeId,
        difficulty: Difficulty,
        ttl_us: u64,
        seed: u64,
    ) -> Self {
        Self {
            stream_id,
            node_id,
            difficulty,
            regs: HashMap::new(),
            stream_record: None,
            ttl_us,
            rng: ChaCha8Rng::seed_from_u64(seed),
            queries: 0,
            registrations: 0,
        }
    }

    /// Register or refresh a peer (local path for the publisher's own record).
    pub fn register_local(&mut self, record: PeerRecord, now_us: u64) {
        self.regs.insert(
            record.node_id,
            Reg {
                record,
                last_seen_us: now_us,
            },
        );
    }

    /// Handle a REGISTER_PEER frame from `from`. Returns whether it was accepted.
    pub fn handle_register(&mut self, from: SocketAddr, r: &RegisterPeer, now_us: u64) -> bool {
        if r.stream_id != self.stream_id {
            return false;
        }
        let vb = &r.validation;
        let body_len = RegisterPeer::LEN - 32 - bs_wire::ValidationBlock::LEN;
        // Validation-block signature covers FrameType ‖ Timestamp ‖ body after the block.
        let mut body = Vec::with_capacity(body_len);
        body.extend_from_slice(&r.port.to_be_bytes());
        body.push(RegisterPeer::PROTOCOL_UDP);
        body.push(r.node_class as u8);
        body.push(r.assigned_trees.0);
        body.push(r.flags.to_byte());
        body.extend_from_slice(&[0, 0]);
        body.extend_from_slice(r.signature.as_bytes());
        let c2 = self
            .difficulty
            .dynamic_accepted(self.regs.len() as u32 + 1, r.node_class, false);
        if Identity::verify_validation_block(
            vb,
            FrameType::REGISTER_PEER,
            &body,
            from.ip(),
            self.difficulty,
            c2,
        )
        .is_err()
        {
            return false;
        }
        let msg = RegisterPeer::signable_bytes(
            &r.stream_id,
            &vb.node_id,
            r.port,
            r.node_class,
            r.assigned_trees,
            r.flags,
            vb.timestamp_us,
        );
        if Verifier::verify(&vb.public_key, &msg, &r.signature).is_err() {
            return false;
        }
        let record = PeerRecord {
            node_id: vb.node_id,
            node_class: r.node_class,
            assigned_trees: r.assigned_trees,
            flags: r.flags,
            addr: WireAddr(SocketAddr::new(from.ip(), r.port)),
        };
        self.regs.insert(
            vb.node_id,
            Reg {
                record,
                last_seen_us: now_us,
            },
        );
        self.registrations += 1;
        true
    }

    /// Answer a GET_PEERS request.
    pub fn handle_get_peers(
        &mut self,
        from: SocketAddr,
        g: &GetPeers,
        now_us: u64,
    ) -> Option<GetPeersResponse> {
        if g.stream_id != self.stream_id {
            return None;
        }
        let mut body = Vec::with_capacity(36);
        body.extend_from_slice(g.stream_id.as_bytes());
        body.push(g.starved_trees.0);
        body.push(g.wanted_trees.0);
        body.push(g.want_relay_capable as u8);
        body.push(0);
        let c2 = self.difficulty.dynamic_accepted(
            self.regs.len() as u32 + 1,
            bs_wire::NodeClass::Leaf,
            false,
        );
        if Identity::verify_validation_block(
            &g.validation,
            FrameType::GET_PEERS,
            &body,
            from.ip(),
            self.difficulty,
            c2,
        )
        .is_err()
        {
            return None;
        }
        self.queries += 1;
        self.expire(now_us);
        let who = g.validation.node_id;
        let mut pool: Vec<PeerRecord> = self
            .regs
            .values()
            .map(|r| r.record)
            .filter(|r| r.node_id != who)
            .filter(|r| {
                g.wanted_trees.is_empty()
                    || (r.node_class == bs_wire::NodeClass::Relay
                        && !r.assigned_trees.intersection(g.wanted_trees).is_empty())
            })
            .collect();
        pool.shuffle(&mut self.rng);
        pool.truncate(bs_wire::consts::MAX_GET_PEERS_RECORDS);
        Some(GetPeersResponse {
            guardian: self.node_id,
            sampled: false,
            records: pool,
            stream_record: self.stream_record.clone(),
        })
    }

    fn expire(&mut self, now_us: u64) {
        let ttl = self.ttl_us;
        self.regs
            .retain(|_, r| now_us.saturating_sub(r.last_seen_us) < ttl);
    }

    /// Registered peers.
    pub fn len(&self) -> usize {
        self.regs.len()
    }
    /// Whether empty.
    pub fn is_empty(&self) -> bool {
        self.regs.is_empty()
    }
}
