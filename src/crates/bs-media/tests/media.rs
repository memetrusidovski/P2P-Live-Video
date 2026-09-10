//! bs-media tests. Section references are to the protocol specification.

use bs_crypto::{Difficulty, Identity};
use bs_media::chunk::{verify_block, verify_manifest};
use bs_media::fec::{parity_for_loss, repair_esi_base};
use bs_media::source::CHUNK_MS;
use bs_media::*;
use bs_wire::consts::{BLOCK_SIZE, K_BLOCK};
use bs_wire::{BlockIndex, SegmentSeq, TreeId};
use bytes::Bytes;
use rand::prelude::*;
use rand_chacha::ChaCha8Rng;

fn publisher() -> Identity {
    Identity::from_seed([7; 32], Difficulty::TEST)
}

/// Ch1 §1.2.4 §4.2.1: the reference-ladder table, every rung.
#[test]
fn allocation_rule_reference_ladder() {
    let ladder = Ladder::reference();
    let rows: [(u8, Vec<u16>); 5] = [
        (2, vec![3000, 3000]),
        (3, vec![1500, 1500, 3000]),
        (4, vec![1500, 1500, 1500, 1500]),
        (5, vec![750, 750, 1500, 1500, 1500]),
        (6, vec![750, 750, 750, 750, 1500, 1500]),
    ];
    for (m, rates) in rows {
        let mx = SlicingMatrix::allocate(1, m, &ladder).unwrap();
        let got: Vec<u16> = mx.trees.iter().map(|t| t.bitrate_kbps).collect();
        assert_eq!(got, rates, "M={m}");
        assert_eq!(
            mx.tree(TreeId::BASE).unwrap().layer,
            0,
            "Tree 1 always carries L0"
        );
        // Trees carrying lower layers are never harder to relay than higher ones.
        for w in mx.trees.windows(2) {
            assert!(w[0].bitrate_kbps <= w[1].bitrate_kbps || w[0].layer == w[1].layer);
        }
    }
    // M=2 bundles {L0+L1},{L2}: tree 1 carries layers 0..=1.
    let m2 = SlicingMatrix::allocate(1, 2, &ladder).unwrap();
    assert_eq!((m2.trees[0].layer, m2.trees[0].layer_end), (0, 1));
    assert_eq!((m2.trees[1].layer, m2.trees[1].layer_end), (2, 2));
    // Folded mappings: M=6 over (L0, L1) → (3,3).
    let folded = Ladder {
        layers: ladder.layers[..2].to_vec(),
    };
    let f6 = SlicingMatrix::allocate(2, 6, &folded).unwrap();
    assert_eq!(f6.trees.iter().filter(|t| t.layer == 0).count(), 3);
    assert_eq!(f6.trees.iter().filter(|t| t.layer == 1).count(), 3);
    // Wire roundtrip.
    let back = SlicingMatrix::from_entries(1, &m2.to_entries(), 3);
    assert_eq!(back, m2);
    let m6 = SlicingMatrix::allocate(1, 6, &ladder).unwrap();
    assert_eq!(SlicingMatrix::from_entries(1, &m6.to_entries(), 3), m6);
}

/// Ch1 §1.2.4 §4.2.1: block j' of layer l travels on stripe j' mod t_l.
#[test]
fn block_to_tree_striping() {
    let ladder = Ladder::reference();
    let mx = SlicingMatrix::allocate(1, 6, &ladder).unwrap();
    let mut src = SyntheticSource::new(ladder, 1, Some(1));
    let built = ChunkBuilder::new(publisher(), 1)
        .build(&src.next_chunk().unwrap())
        .unwrap();
    let m = &built.manifest.body;
    // L0 is striped over trees 1,2.
    let l0 = m.layer_block_counts[0];
    for j in 0..l0 {
        let t = mx.tree_for_block(m, j).unwrap();
        assert_eq!(t, TreeId(1 + (j % 2) as u8));
    }
    // Subscribed set for layer prefix L0..L1 is trees 1..4.
    assert_eq!(mx.trees_for_layer_prefix(1).0, 0b0000_1111);
    assert_eq!(mx.trees_for_layer_prefix(0).0, 0b0000_0011);
}

/// Ch4 §4.1: layer-major block numbering, signed manifest verifies, proofs verify,
/// tampering fails.
#[test]
fn chunk_build_and_verify() {
    let pk = publisher();
    let builder = ChunkBuilder::new(pk.clone(), 1);
    let mut src = SyntheticSource::new(Ladder::reference(), 42, Some(4));
    let chunk = src.next_chunk().unwrap();
    let built = builder.build(&chunk).unwrap();
    let m = &built.manifest;
    assert_eq!(
        m.body.layer_block_counts,
        vec![3, 3, 6],
        "46875 B → 3 blocks, 46875 → 3, remainder 89196 → 6"
    );
    assert_eq!(built.block_count(), 12);
    assert_eq!(m.body.chunk_byte_length as usize, chunk.byte_len());
    verify_manifest(m, &pk.public_key()).unwrap();
    assert!(verify_manifest(
        m,
        &Identity::from_seed([8; 32], Difficulty::TEST).public_key()
    )
    .is_err());
    for j in 0..built.block_count() {
        let proof = built.block_proof(j, 2);
        assert_eq!(proof.siblings.len(), 4);
        verify_block(
            &m.body,
            proof.block,
            &built.blocks[j as usize],
            &proof.siblings,
        )
        .unwrap();
        let mut bad = built.blocks[j as usize].to_vec();
        bad[5] ^= 1;
        assert!(verify_block(&m.body, proof.block, &bad, &proof.siblings).is_err());
    }
}

/// Ch4 §4.2: systematic RaptorQ; any 16 distinct symbols decode; fewer do not;
/// fast path when all source symbols present.
#[test]
fn fec_roundtrip_with_loss() {
    let mut rng = ChaCha8Rng::seed_from_u64(3);
    let block: Vec<u8> = (0..BLOCK_SIZE).map(|_| rng.random()).collect();
    let block = Bytes::from(block);
    let enc = FecEncoder::new(SegmentSeq(1), BlockIndex::new(0, 0).unwrap(), block.clone());
    let symbols = enc.push_symbols(5);
    assert_eq!(symbols.len(), K_BLOCK + 5);
    assert!(symbols[..K_BLOCK].iter().all(|s| s.is_source()));
    assert_eq!(
        &symbols[3].payload[..],
        &block[3 * 1024..4 * 1024],
        "source symbols are raw slices"
    );

    // No loss: fast path.
    let mut c = SymbolCollector::new();
    for s in &symbols[..K_BLOCK] {
        c.add(s.esi, s.payload.clone()).unwrap();
    }
    assert!(c.all_source());
    assert_eq!(c.decode().unwrap(), block);

    // Lose 4 source symbols, keep 5 repair: decodes.
    let mut c = SymbolCollector::new();
    for s in symbols.iter().filter(|s| !(2..6).contains(&s.esi)) {
        c.add(s.esi, s.payload.clone()).unwrap();
    }
    assert!(!c.all_source() && c.is_complete());
    assert_eq!(c.decode().unwrap(), block);

    // 15 symbols: undecodable.
    let mut c = SymbolCollector::new();
    for s in symbols.iter().take(15) {
        c.add(s.esi, s.payload.clone()).unwrap();
    }
    assert_eq!(c.missing(), 1);
    assert!(c.decode().is_err());

    // Duplicates are ignored.
    let mut c = SymbolCollector::new();
    assert!(c.add(0, symbols[0].payload.clone()).unwrap());
    assert!(!c.add(0, symbols[0].payload.clone()).unwrap());
}

/// Ch4 §4.2.2: E = max(1, ceil(clamp(2ρK, 0.05K, 0.30K))) — worked examples.
#[test]
fn parity_sizing() {
    assert_eq!(parity_for_loss(0.0), 1);
    assert_eq!(parity_for_loss(0.02), 1, "clean link clamps to floor");
    assert_eq!(parity_for_loss(0.15), 5, "cellular: 0.30·16 = 4.8 → 5");
    assert_eq!(parity_for_loss(0.9), 5, "ceiling");
    assert_eq!(parity_for_loss(0.1), 4, "2·0.1·16 = 3.2 → 4");
}

/// Ch4 §4.1.2: repair ESI base is in [K, 2^16 − 256).
#[test]
fn repair_base_range() {
    for i in 0..50u8 {
        let b = repair_esi_base(&bs_wire::NodeId([i; 32]));
        assert!(b as usize >= K_BLOCK && (b as u32) < (1 << 16) - 256);
    }
}

/// End-to-end data correctness: file → chunks → blocks → symbols (lossy) →
/// verified blocks → reassembled file, byte-identical, hash-equal.
#[test]
fn file_roundtrip_is_byte_exact() {
    let mut rng = ChaCha8Rng::seed_from_u64(11);
    let file: Vec<u8> = (0..1_000_003).map(|_| rng.random()).collect();
    let file = Bytes::from(file);
    let ladder = Ladder::reference();
    let mut src = FileSource::new(ladder.clone(), file.clone());
    let pk = publisher();
    let builder = ChunkBuilder::new(pk.clone(), 1);
    let mut sink = ChunkAssembler::new(ladder.len(), true);
    let total_chunks = src.total_chunks();
    let mut n = 0;
    while let Some(chunk) = src.next_chunk() {
        n += 1;
        let built = builder.build(&chunk).unwrap();
        verify_manifest(&built.manifest, &pk.public_key()).unwrap();
        sink.begin_chunk(&built.manifest.body);
        for j in 0..built.block_count() {
            let idx = BlockIndex::new(chunk.chunk_index, j).unwrap();
            let enc = FecEncoder::new(chunk.segment, idx, built.blocks[j as usize].clone());
            let syms = enc.push_symbols(3);
            // Drop up to 3 random symbols on this "link".
            let mut c = SymbolCollector::new();
            let drop: Vec<usize> = (0..3).map(|_| rng.random_range(0..syms.len())).collect();
            for (i, s) in syms.iter().enumerate() {
                if !drop.contains(&i) {
                    c.add(s.esi, s.payload.clone()).unwrap();
                }
            }
            let block = c.decode().unwrap();
            let proof = built.block_proof(j, 0);
            verify_block(&built.manifest.body, idx, &block, &proof.siblings).unwrap();
            let done = sink.add_block(chunk.segment, chunk.chunk_index, j, block);
            assert_eq!(done.is_some(), j + 1 == built.block_count());
        }
    }
    assert_eq!(n, total_chunks);
    assert_eq!(sink.completed_chunks(), total_chunks);
    assert_eq!(sink.output().len(), file.len());
    assert_eq!(sink.output(), &file[..]);
    assert_eq!(sink.output_hash(), src.file_hash());
    assert_eq!(sink.layer_hashes(), src.layer_hashes());
    // Layer sizes: non-last layers are block aligned (the reassembly rule).
    let sizes = bs_media::source::aligned_layer_sizes(&ladder, CHUNK_MS);
    assert_eq!(sizes[0] % BLOCK_SIZE, 0);
    assert_eq!(sizes[1] % BLOCK_SIZE, 0);
    assert_eq!(
        sizes.iter().sum::<usize>(),
        ladder.bytes_per_chunk(CHUNK_MS).iter().sum::<usize>()
    );
}

/// Synthetic sources are deterministic per seed and differ across seeds.
#[test]
fn synthetic_is_deterministic() {
    let a: Vec<_> = SyntheticSource::new(Ladder::single(1000), 5, Some(4)).collect_chunks();
    let b: Vec<_> = SyntheticSource::new(Ladder::single(1000), 5, Some(4)).collect_chunks();
    let c: Vec<_> = SyntheticSource::new(Ladder::single(1000), 6, Some(4)).collect_chunks();
    assert_eq!(a, b);
    assert_ne!(a, c);
    assert_eq!(a[0].segment, SegmentSeq(1));
    assert_eq!(a[3].chunk_index, 3);
}

trait CollectChunks {
    fn collect_chunks(&mut self) -> Vec<LayeredChunk>;
}
impl<S: Source> CollectChunks for S {
    fn collect_chunks(&mut self) -> Vec<LayeredChunk> {
        let mut v = Vec::new();
        while let Some(c) = self.next_chunk() {
            v.push(c);
        }
        v
    }
}
