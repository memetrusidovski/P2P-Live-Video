# SOLUTION-060: The Stream Descriptor — What the Bytes Are

**Closes:** ISSUE-060 (Medium)
**Lives in:** `appendix_d_frame_registry.md` §D.3 (`0x1F`), §D.4.8 (`LayerByteLength`), §D.4.16 (`0xFD`), §D.4.20 (new); `protocol/chapter4/4.1_segment_serialization/1_gop_serialization.md` (*What the Bytes Are*); `protocol/chapter4/4.2_fec_raptorq/3_symbol_packet_layout.md` (padding); `protocol/chapter4/4.3_hybrid_push_pull/1_buffer_sliding_timeline.md` (join step 3); `protocol/chapter2/2.3_stream_registration/1_publisher_genesis_key.md`, `3_registration_frames.md` (`DescriptorVersion`, `DescriptorHash`); `protocol/chapter1/1.2_multi_forest_overlays/4_stream_slicing_architecture.md` §4.2.2 (shed by `LayerMode`); `protocol/chapter7/7.1_source_pinning/1_genesis_key_anchoring.md`; `appendix_b_parameters.md`; `schemas/p2p_live.proto`
**Class:** A payload-agnostic protocol that forgot the one consumer that is not payload-agnostic

---

## The problem in one line

Nothing on the wire said what the verified bytes were — no codec, no container, no initialisation segment — so a browser viewer could verify every block of segment $X$ and have nothing it could append to a media source, and the decoder side of "shed $L_2$" was undefined because nobody had said how layers combine.

## The decision

*   A new source-signed record, **`STREAM_DESCRIPTOR` (0x1F)**: `ContentType`, `LayerMode`, and per layer `CodecTag`, resolution, frame rate, bitrate and up to 4 KB of initialisation data. Versioned, effective at a stated segment announced five segments ahead like a matrix change, pushed down every tree once, retained alongside the previous version.
*   **The Stream Record carries the descriptor's version and hash** (+36 B; fixed part 132 B, `GET_PEERS` response $\approx 1{,}400$ B, still inside the MTU). The body comes from the first parent via `MANIFEST_REQUEST(0xFD)`, in the same request as the segment-$X$ manifests, so Startup Join Latency is unchanged.
*   **The payload of layer $l$ in a chunk is defined**: that layer's media bytes for the 250 ms in the named container, cut into 16 KB blocks with the last block padded; the `MANIFEST` gains `LayerByteLength` per layer so the padding is locatable for every layer, not only the last.
*   **Shedding is defined per `LayerMode`**: stop a byte stream (SVC), switch rendition at a segment boundary (`SIMULCAST`), stop an unrelated stream (`INDEPENDENT`). The forest is unchanged in every mode.

## Why this and not the alternatives

*   **Initialisation data in every `MANIFEST`**: 1–2 KB per 250 ms is 4–8 kbps per tree — under the 2% control budget at 6 Mbps but $\approx 1\%$ of a $0.75$ Mbps $L_0$ stripe's own bitrate, for data that changes once an hour.
*   **Out of band over HTTPS from the publisher**: the protocol would be trackerless for everything except the one byte range every viewer needs first, and a NAT-blocked publisher (Ch6 §6.3.3) could not serve it.
*   **Initialisation data in the Stream Record itself**: up to 12 KB in a record that rides every `GET_PEERS` response and is republished every second to twenty guardians; the hash is 32 B.
*   **Leave it to implementations**: two conforming implementations that cannot play each other's streams are not conforming to anything.

## Defects found during verification

*   Ch4 §4.2.3 referred to a `SegmentByteLength` field the manifest never had (it had `ChunkByteLength`), and the chunk total cannot locate the padding of any layer but the last. Both are fixed by `LayerByteLength`.
*   The spec's own "segment sequence numbers are the only synchronization authority" presumed decoding could begin once segment $X$ was anchored; it could not without initialisation data. The descriptor fetch rides the request the joiner already makes at that step.
*   `SlicingMode` (`SVC_SPATIAL` | `MDC`) was the only content hint on the wire and describes the *forest's* view; `LayerMode` describes the *decoder's*. MDC in the slicing sense is `SIMULCAST`-like at the decoder only if the descriptions are independently decodable, which the descriptor now states.
*   The record's fixed-part and response-size arithmetic: $96 + 4 + 32 = 132$ B; $132 + 42 + 42 + 64 = 280$ B at $M = 6 \to 6$; $20 \times 54 + 280 + 40 = 1{,}400$ B.

## The generalisable lesson

**A protocol that is proudly agnostic about its payload still has exactly one consumer that is not, and that consumer needs a signed, versioned description delivered on the same path as the first byte it will consume.** Payload-agnosticism is a property of the forwarding layers; it is not permission to omit the descriptor.

## Residual risk

*   Initialisation data is capped at 4 KB per layer; an exotic container needing more must use `OPAQUE` and carry it in-band. Stated.
*   A descriptor change and a matrix change can be pending simultaneously; both use the same five-segment lead and are independent records, so this is a coordination burden on the publisher, not an ambiguity for peers.
*   `SIMULCAST` switching waits for a segment boundary — up to one second — which the 3.0 s buffer absorbs but which is slower than an SVC shed.

## Validation owed (Chapter 8)

*   Startup Join Latency with the descriptor fetch folded into step 3, against the $1.5$ s target.
*   Interoperability test: a publisher and a browser (MSE) viewer written from the specification alone play each other's streams for `VIDEO_CMAF` at all five ladder rungs and through a fold.
