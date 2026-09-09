# 2. Replay Attack Prevention

The following pseudocode outlines how incoming manifests are validated and how replays are rejected.

All protocol timestamps are **microseconds** (unsigned 64-bit), matching the validation, PoU, MANIFEST, and STREAM_END frame layouts. The manifest's timestamp is carried and signed, but it is **not** an acceptance criterion: manifests are admitted by *sequence*, relative to the live edge the peer itself observes.

```python
import cryptography  # Ed25519 library

RETAIN_SEGMENTS   = 8      # tau_retain: segments every peer keeps for late joiners (Ch4 §4.3.1)
LOOKAHEAD         = 2      # segments ahead of the observed live edge a manifest may claim
CHUNKS_PER_SEGMENT = 4

# Rolling window of verified (segment, chunk) -> merkle_root.
# Bounded: entries older than the retention window are evicted.
verified = {}                       # (seq_num, chunk) -> merkle_root
X_edge   = -1                       # highest segment seen on the PUSH path (Ch4 §4.3.1)

ACCEPT, DUPLICATE, OUT_OF_WINDOW, EQUIVOCATION, FORGED = range(5)

def validate_incoming_manifest(packet, pinned_pubkey):
    global X_edge

    # 1. Parse fields from binary frame (Appendix D §D.4.8)
    body        = packet.read_bytes(packet.payload_length - 64)   # everything the signature covers
    signature   = packet.read_bytes(64)
    stream_id, seq_num, chunk, chunk_count, timestamp, merkle_root = parse_manifest(body)

    # 2. Cryptographic validation FIRST. Only a signature failure is a verification
    #    failure; only a verification failure is penalised (Ch7 §7.1.1).
    try:
        pinned_pubkey.verify(signature, body)
    except InvalidSignature:
        return FORGED          # drop; disconnect + local ban of the delivering peer

    # 3. Duplicates are normal — every peer hears each manifest from up to M parents.
    if (seq_num, chunk) in verified:
        if verified[(seq_num, chunk)] == merkle_root:
            return DUPLICATE   # ignore silently; never penalise
        return EQUIVOCATION    # the SOURCE signed two roots for one chunk: keep the first, log

    # 4. Sequence window relative to the live edge this peer has itself observed.
    #    Below the window: older than anything we would still serve or play.
    #    Above it: further ahead than the source can honestly be.
    if X_edge >= 0 and not (X_edge - RETAIN_SEGMENTS <= seq_num <= X_edge + LOOKAHEAD):
        return OUT_OF_WINDOW   # drop; not penalised (a stale relay, not a forger)

    # 5. Accept, remember, and evict beyond the window
    verified[(seq_num, chunk)] = merkle_root
    if seq_num > X_edge and arrived_on_push_path(packet):
        X_edge = seq_num       # the anchor moves only forward, only on the push path
    cutoff = X_edge - RETAIN_SEGMENTS
    for key in [k for k in verified if k[0] < cutoff]:
        del verified[key]
    return ACCEPT
```

## Why There Is No Wall-Clock Check

An earlier version rejected any manifest whose timestamp differed from local time by more than $\pm 2$ s, *before* checking the signature. That rule was wrong in three ways that the sequence window is not:

*   **It rejected every manifest a late joiner needs.** A joiner backfills segment $X$ when its manifest is already $1.7$–$3$ s old (record lag, lookup, handshake, PULL round-trip — Ch4 §4.3.1), and fills a $3$ s buffer from retained segments older still. Under the $\pm 2$ s rule none of those blocks could be Merkle-verified, so a late join could never play.
*   **It made an unsynchronised clock fatal.** Step 2 ran before the signature check, so a device whose clock was off by more than 2 s admitted *no* manifests at all — directly contradicting the design rule that playback is driven by sequence numbers, never wall-clock time.
*   **It added nothing the sequence watermark did not already provide.** Replay is bounded by sequence: a manifest for a segment the peer has already played, or one beyond the retention window, is rejected by step 4 regardless of what its timestamp says; a manifest *inside* the window is one the peer would legitimately accept from any parent.

The signed timestamp remains in the frame for coarse staleness estimates (Ch4 §4.3.1's anchor table) and for audit, not for admission.

## The Verdicts Are Not Interchangeable

`DUPLICATE` and `OUT_OF_WINDOW` return the same *action* as `FORGED` — the frame is not used — but they must **never** produce the same *consequence*. Every peer receives every manifest from up to $M$ parents; if a repeat were treated as a forgery and the delivering peer banned under §7.1.1, every parent would be banned within one segment. Only `FORGED` — a signature that fails against the pinned key — is a verification failure and penalised. `EQUIVOCATION` indicts the source, not the relay, and is logged.

Playback position and live-edge tracking are driven **exclusively by segment sequence numbers** (Ch4 §4.3), never by wall-clock time, so a peer with a drifted system clock plays correctly.
