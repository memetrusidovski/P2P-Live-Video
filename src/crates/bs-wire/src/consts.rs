//! Protocol constants that fix **byte layouts** (Appendix B). Timing and overlay
//! sizing parameters live in `bs_core::params`.

/// One Merkle / RaptorQ source block, bytes (`Size_block`).
pub const BLOCK_SIZE: usize = 16 * 1024;
/// RaptorQ symbol size, bytes (`T_symbol`).
pub const SYMBOL_SIZE: usize = 1024;
/// Source symbols per block (`K_block`).
pub const K_BLOCK: usize = BLOCK_SIZE / SYMBOL_SIZE;
/// Chunks per one-second segment (Ch4 §4.1.1, `C`).
pub const CHUNKS_PER_SEGMENT: u8 = 4;
/// Bits of `BlockIndex` holding the in-chunk index `j`.
pub const BLOCK_INDEX_CHUNK_SHIFT: u32 = 12;
/// Mask selecting `j` from a `BlockIndex`.
pub const BLOCK_INDEX_J_MASK: u16 = (1 << BLOCK_INDEX_CHUNK_SHIFT) - 1;
/// Maximum blocks per chunk (`j < 4096`).
pub const MAX_BLOCKS_PER_CHUNK: usize = 1 << BLOCK_INDEX_CHUNK_SHIFT;
/// Maximum forest size the layouts allow (`AssignedTrees` is one bit per tree).
pub const MAX_TREES: usize = 8;
/// Peer Records per `GET_PEERS` response.
pub const MAX_GET_PEERS_RECORDS: usize = 20;
/// Adjacent-pair samples in a `RANK_PROOF`.
pub const RANK_PROOF_PAIRS: u8 = 8;
/// Accusations per `REPUTATION_AUDIT_GOSSIP`.
pub const MAX_ACCUSATIONS: u8 = 2;
/// Entries in a `ROSTER` (`R_roster`).
pub const ROSTER_ENTRIES: usize = 8;
/// S/Kademlia validation block length (Ch2 §2.2.3).
pub const VALIDATION_BLOCK_LEN: usize = 152;
/// NodeID / StreamID / hash length.
pub const HASH_LEN: usize = 32;
/// Ed25519 public key length.
pub const PUBKEY_LEN: usize = 32;
/// Ed25519 signature length.
pub const SIGNATURE_LEN: usize = 64;
