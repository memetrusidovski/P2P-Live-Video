//! Discovery oracle: stands in for the S/Kademlia guardians in M1. Answers
//! `GET_PEERS` the way a guardian would — a uniformly random sample of active
//! registrations whose `AssignedTrees` intersect the wanted set — plus the
//! publisher's latest signed Stream Record.

use std::collections::HashMap;

use bs_wire::{NodeId, PeerRecord, StreamRecord, TreeSet};
use rand::seq::SliceRandom;
use rand_chacha::ChaCha8Rng;

/// The oracle.
#[derive(Default)]
pub struct Oracle {
    records: HashMap<NodeId, PeerRecord>,
    /// Latest Stream Record.
    pub stream_record: Option<StreamRecord>,
    /// Queries served.
    pub queries: u64,
}

impl Oracle {
    /// Register / refresh.
    pub fn register(&mut self, r: PeerRecord) {
        self.records.insert(r.node_id, r);
    }
    /// Remove a dead node.
    pub fn remove(&mut self, id: &NodeId) {
        self.records.remove(id);
    }
    /// Store the publisher's record.
    pub fn store(&mut self, r: StreamRecord) {
        self.stream_record = Some(r);
    }
    /// Answer a query from `who`.
    pub fn get_peers(
        &mut self,
        rng: &mut ChaCha8Rng,
        who: &NodeId,
        wanted: TreeSet,
    ) -> Vec<PeerRecord> {
        self.queries += 1;
        let mut pool: Vec<PeerRecord> = self
            .records
            .values()
            .filter(|r| r.node_id != *who)
            .filter(|r| {
                wanted.is_empty()
                    || (r.node_class == bs_wire::NodeClass::Relay
                        && !r.assigned_trees.intersection(wanted).is_empty())
            })
            .copied()
            .collect();
        pool.shuffle(rng);
        pool.truncate(bs_wire::consts::MAX_GET_PEERS_RECORDS);
        pool
    }
}
