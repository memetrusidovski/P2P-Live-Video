//! Rendezvous tree assignment (Ch1 §1.2.1 §1.3, §1.2.5 §5.3):
//! `a(v) = argmax_m Blake3(NodeID_v ‖ m)`, and the full ranking used for
//! multi-tree super nodes and coverage grants.

use bs_wire::{NodeId, TreeId};

/// Score of node `v` for tree `m`.
pub fn rendezvous_score(node: &NodeId, tree: TreeId) -> [u8; 32] {
    let mut h = blake3::Hasher::new();
    h.update(node.as_bytes());
    h.update(&[tree.0]);
    *h.finalize().as_bytes()
}

/// Trees `1..=m` ordered by descending rendezvous score; index 0 is the primary
/// assignment, index 1 the coverage-grant candidate.
pub fn rendezvous_rank(node: &NodeId, m: u8) -> Vec<TreeId> {
    let mut v: Vec<(TreeId, [u8; 32])> = (1..=m)
        .map(|t| (TreeId(t), rendezvous_score(node, TreeId(t))))
        .collect();
    v.sort_by_key(|(_, s)| core::cmp::Reverse(*s));
    v.into_iter().map(|(t, _)| t).collect()
}

/// Primary tree assignment.
pub fn assigned_tree(node: &NodeId, m: u8) -> TreeId {
    rendezvous_rank(node, m)[0]
}
