use bs_media::fec::{repair_esi_base, FecEncoder, SymbolCollector};
use bs_wire::{BlockIndex, NodeId, SegmentSeq};
use bytes::Bytes;
use rand::{Rng, SeedableRng};

/// Any K of the pushed source+repair symbols must reconstruct the block exactly.
#[test]
fn decode_from_any_k_symbols_is_exact() {
    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(7);
    let mut bad = 0;
    for trial in 0..400 {
        let block: Vec<u8> = (0..16 * 1024).map(|_| rng.random()).collect();
        let block = Bytes::from(block);
        let enc = FecEncoder::new(SegmentSeq(1), BlockIndex::new(0, 0).unwrap(), block.clone());
        let parity = 1 + (trial % 5) as u16;
        let mut syms = enc.push_symbols(parity);
        if trial % 3 == 0 {
            // Some pull-path repair symbols from another node too.
            syms.extend(enc.repair_symbols(repair_esi_base(&NodeId([trial as u8; 32])), 2));
        }
        // Drop `lost` symbols at random, keeping at least K.
        let lost = rng.random_range(0..=parity as usize);
        for _ in 0..lost {
            let i = rng.random_range(0..syms.len());
            syms.remove(i);
        }
        let mut c = SymbolCollector::new();
        for s in syms {
            c.add(s.esi, s.payload).unwrap();
        }
        assert!(c.is_complete());
        let out = c.decode().expect("decodable");
        if out != block {
            bad += 1;
            eprintln!(
                "trial {trial}: parity {parity} lost {lost} all_source {} -> WRONG BYTES",
                c.all_source()
            );
        }
    }
    assert_eq!(bad, 0, "{bad} of 400 decodes returned wrong bytes");
}
