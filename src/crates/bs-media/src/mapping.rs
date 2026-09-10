//! Layer → tree allocation and the slicing matrix (Ch1 §1.2.4 §4.2.1).
//!
//! The **source** builds the matrix from `(M, ladder)` with the minimax rule and
//! publishes it; peers only read it.

use bs_wire::frames::ManifestBody;
use bs_wire::{TreeId, TreeMappingEntry};

use crate::ladder::Ladder;
use crate::MediaError;

/// Per-tree view of the matrix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TreeInfo {
    /// Tree.
    pub tree_id: TreeId,
    /// Lowest layer carried (for bundles, the first of the bundle).
    pub layer: u8,
    /// Highest layer carried (equals `layer` unless bundled, M < L).
    pub layer_end: u8,
    /// Stripe index within the layer (0 for bundles).
    pub stripe_index: u8,
    /// Stripes of the layer (1 for bundles).
    pub stripe_count: u8,
    /// Shed priority; lower is shed first.
    pub priority: u8,
    /// Declared per-tree bitrate.
    pub bitrate_kbps: u16,
}

/// The slicing matrix in force.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlicingMatrix {
    /// Version, strictly increasing at each change.
    pub version: u8,
    /// One entry per tree, ordered by tree id from 1.
    pub trees: Vec<TreeInfo>,
    /// Layers in the ladder this matrix was built for.
    pub layer_count: u8,
}

impl SlicingMatrix {
    /// Build the matrix for forest size `m` over `ladder` by the allocation rule.
    ///
    /// * `M >= L`: every layer gets one tree, then each spare tree goes to the
    ///   layer with the largest current per-tree bitrate (ties → lower layer).
    ///   Layer `l` is striped over its `t_l` trees; tree ids run in layer order.
    /// * `M < L`: layers are cut into `M` contiguous bundles minimising the largest
    ///   bundle bitrate (ties → the cut giving Tree 1 the smaller bundle).
    pub fn allocate(version: u8, m: u8, ladder: &Ladder) -> Result<Self, MediaError> {
        let l = ladder.len();
        if m == 0 || l == 0 {
            return Err(MediaError::Mapping("M and L must be >= 1"));
        }
        if m as usize > bs_wire::consts::MAX_TREES {
            return Err(MediaError::Mapping("M exceeds 8 trees"));
        }
        let rates: Vec<u32> = ladder.layers.iter().map(|x| x.bitrate_kbps).collect();
        let mut trees = Vec::with_capacity(m as usize);

        if m as usize >= l {
            // Striping.
            let mut t = vec![1u32; l];
            for _ in 0..(m as usize - l) {
                // Largest b_l / t_l, ties to lower layer.
                let mut best = 0;
                for i in 1..l {
                    // compare rates[i]/t[i] > rates[best]/t[best] using cross-multiplication
                    if (rates[i] as u64) * (t[best] as u64) > (rates[best] as u64) * (t[i] as u64) {
                        best = i;
                    }
                }
                t[best] += 1;
            }
            let mut id = 1u8;
            for (layer, &tl) in t.iter().enumerate() {
                let per_tree = (rates[layer] / tl) as u16;
                for stripe in 0..tl {
                    trees.push(TreeInfo {
                        tree_id: TreeId(id),
                        layer: layer as u8,
                        layer_end: layer as u8,
                        stripe_index: stripe as u8,
                        stripe_count: tl as u8,
                        priority: (l - 1 - layer) as u8,
                        bitrate_kbps: per_tree,
                    });
                    id += 1;
                }
            }
        } else {
            // Bundling: choose M-1 cut points among L-1 gaps minimising the max bundle.
            let cuts = best_cuts(&rates, m as usize);
            let mut start = 0usize;
            let mut bounds = Vec::new();
            for c in cuts.iter().chain(std::iter::once(&l)) {
                bounds.push((start, *c));
                start = *c;
            }
            for (i, (s, e)) in bounds.iter().enumerate() {
                let rate: u32 = rates[*s..*e].iter().sum();
                trees.push(TreeInfo {
                    tree_id: TreeId(i as u8 + 1),
                    layer: *s as u8,
                    layer_end: (*e - 1) as u8,
                    stripe_index: 0,
                    stripe_count: 1,
                    priority: (m as usize - 1 - i) as u8,
                    bitrate_kbps: rate as u16,
                });
            }
        }
        Ok(Self {
            version,
            trees,
            layer_count: l as u8,
        })
    }

    /// Number of trees.
    pub fn m(&self) -> u8 {
        self.trees.len() as u8
    }

    /// Info for a tree.
    pub fn tree(&self, id: TreeId) -> Option<&TreeInfo> {
        self.trees.get(id.index())
    }

    /// Trees carrying layer `l` (in stripe order).
    pub fn trees_for_layer(&self, l: u8) -> impl Iterator<Item = &TreeInfo> + '_ {
        self.trees
            .iter()
            .filter(move |t| t.layer <= l && l <= t.layer_end)
    }

    /// Trees carrying any layer in `0..=top_layer`: the subscribed set for a
    /// layer prefix (Ch1 §1.3.1 "The Subscribed Tree Set").
    pub fn trees_for_layer_prefix(&self, top_layer: u8) -> bs_wire::TreeSet {
        let mut s = bs_wire::TreeSet::EMPTY;
        for t in &self.trees {
            if t.layer <= top_layer {
                s.insert(t.tree_id);
            }
        }
        s
    }

    /// Which tree carries block `j` of a chunk described by `manifest`.
    /// Block `j'` of layer `l` travels on the tree with `StripeIndex = j' mod t_l`.
    pub fn tree_for_block(&self, manifest: &ManifestBody, j: u16) -> Option<TreeId> {
        let (layer, j_in_layer) = manifest.layer_of(j)?;
        let stripes: Vec<&TreeInfo> = self.trees_for_layer(layer).collect();
        if stripes.is_empty() {
            return None;
        }
        let idx = (j_in_layer as usize) % stripes.len();
        Some(stripes[idx].tree_id)
    }

    /// Wire entries for `MANIFEST_UPDATE` / Stream Record.
    pub fn to_entries(&self) -> Vec<TreeMappingEntry> {
        self.trees
            .iter()
            .map(|t| TreeMappingEntry {
                tree_id: t.tree_id,
                layer: t.layer,
                stripe_index: t.stripe_index,
                stripe_count: t.stripe_count,
                priority: t.priority,
                bitrate_kbps: t.bitrate_kbps,
            })
            .collect()
    }

    /// Reconstruct from wire entries (bundles are recovered by contiguity: a
    /// tree whose `layer` is followed by a gap owns the layers up to the next
    /// tree's `layer`).
    pub fn from_entries(version: u8, entries: &[TreeMappingEntry], layer_count: u8) -> Self {
        let mut trees: Vec<TreeInfo> = entries
            .iter()
            .map(|e| TreeInfo {
                tree_id: e.tree_id,
                layer: e.layer,
                layer_end: e.layer,
                stripe_index: e.stripe_index,
                stripe_count: e.stripe_count,
                priority: e.priority,
                bitrate_kbps: e.bitrate_kbps,
            })
            .collect();
        trees.sort_by_key(|t| t.tree_id);
        // Fill bundle ends.
        let n = trees.len();
        for i in 0..n {
            let next_layer = trees.get(i + 1).map(|t| t.layer).unwrap_or(layer_count);
            if trees[i].stripe_count == 1 && next_layer > trees[i].layer + 1 {
                trees[i].layer_end = next_layer - 1;
            }
        }
        Self {
            version,
            trees,
            layer_count,
        }
    }
}

/// Choose `m-1` cut indices (exclusive ends) over `rates` minimising the maximum
/// bundle sum; ties prefer the cut set giving the first bundle the smaller sum.
fn best_cuts(rates: &[u32], m: usize) -> Vec<usize> {
    let mut best: Option<(u32, Vec<u32>, Vec<usize>)> = None;
    // Enumerate combinations of m-1 cuts in 1..l (L ≤ 8 so this is tiny).
    fn rec(
        rates: &[u32],
        start: usize,
        left: usize,
        cur: &mut Vec<usize>,
        best: &mut Option<(u32, Vec<u32>, Vec<usize>)>,
    ) {
        let l = rates.len();
        if left == 0 {
            let mut sums = Vec::new();
            let mut s = 0;
            for c in cur.iter().chain(std::iter::once(&l)) {
                sums.push(rates[s..*c].iter().sum::<u32>());
                s = *c;
            }
            let mx = *sums.iter().max().unwrap();
            let better = match best {
                None => true,
                Some((bmx, bsums, _)) => mx < *bmx || (mx == *bmx && sums < *bsums),
            };
            if better {
                *best = Some((mx, sums, cur.clone()));
            }
            return;
        }
        for c in start..l {
            if l - c < left {
                break;
            }
            cur.push(c);
            rec(rates, c + 1, left - 1, cur, best);
            cur.pop();
        }
    }
    rec(rates, 1, m - 1, &mut Vec::new(), &mut best);
    best.map(|b| b.2).unwrap_or_default()
}
