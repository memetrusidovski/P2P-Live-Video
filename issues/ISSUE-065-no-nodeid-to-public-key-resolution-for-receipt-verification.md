# ISSUE-065: Nothing resolves a NodeID to its public key, so third-party receipts cannot be verified

**Status:** OPEN
**Filed:** 2026-09-16 (surfaced by the reference implementation, `src/`, while implementing PROOF_OF_UPLOAD and RANK_PROOF)
**Spec sections:** Ch5 §5.2.1 (receipt verification), §5.2.2 (`RANK_PROOF` verify step), §5.2.3 (frame layout), Ch2 §2.2.1 (NodeID = static puzzle over the public key), Ch2 §2.3.3 (Peer Record)

## Problem

A `PROOF_OF_UPLOAD` carries the downloader's **NodeID**, not its public key, and Ch5 §5.2.3 says a
third party "resolves the downloader's public key from its NodeID and checks the signature". The
NodeID is `Blake3(PublicKey ‖ StaticNonce)` (Ch2 §2.2.1): it cannot be inverted, and no record in the
protocol carries the mapping. The Peer Record (Ch2 §2.3.3) has NodeID, class, trees, flags and
address; `FIND_NODE` results are Peer Records; `GET_PEERS` returns Peer Records. The only frames that
carry a public key are the pre-session frames with a validation block (PING/PONG, JOIN, PROBE, the
DHT RPCs), and those bind the key to a NodeID only for the peer that sent them.

Consequences:

*   An **uploader** can verify a receipt only if the downloader has sent it a validation block.
    Tree children have (PROBE), gossip neighbours may not have (a NEIGHBOR(0) after a SHUFFLE
    carries no block), so PULL receipts from such peers are unverifiable.
*   A **`RANK_PROOF` verifier** almost never knows the sampled downloaders' keys, so it cannot
    perform the "each sampled receipt verifies" check of §5.2.2 without a further round trip per
    receipt (a PING to each downloader), which does not fit an admission decision.

## Options

1.  **Carry the key with the receipt when it leaves the pair.** The receipt frame stays as it is
    between downloader and uploader; a `RANK_PROOF` pair carries, per leaf, the signer's
    `[PublicKey 32][StaticNonce 8]` so the verifier can recompute the NodeID and check the
    signature. Cost: 40 bytes per sampled receipt (≈ 640 bytes per proof).
2.  **Put the key in the Peer Record** (+32 bytes, +8 for the nonce). Every discovery frame grows
    and a 20-record IPv6 `GET_PEERS` response overshoots the MTU (§2.3.2 already rejected a
    smaller growth for that reason).
3.  **PING every new session peer** so both ends learn each other's key before any receipt flows.
    Solves the uploader side only.

## Implementation decision (pending spec resolution)

Options 1 and 3 together: the `RANK_PROOF` pair layout in `bs-wire` carries `Signers` (count, then
`PublicKey ‖ StaticNonce` per leaf), and every new QUIC session is PINGed once. Receipts whose signer
key is unknown are counted (`receipts_rejected_by_reason.unknown_key`) and dropped.
