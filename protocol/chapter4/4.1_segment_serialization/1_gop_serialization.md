# 1. GOP and Serialization Constraints

To achieve ultra-low latency with high scalability, the primary video stream is encoded with strict Group of Pictures (GOP) constraints. The encoder must generate closed-GOP segments of exactly $1.0\text{ second}$ duration (typically 60 frames for a 1080p60 stream). This ensures that every segment is completely self-contained and decodable without reference to prior segments.

Each $1.0$-second segment is serialized into a binary payload. For a $6\text{ Mbps}$ video stream, the average segment size is:
$$\text{SegmentSize} = 6\text{ Mbps} \cdot 1.0\text{ s} = 6\text{ Mb} = 750\text{ KB}$$

## What the Bytes Are: The Stream Descriptor

The forwarding, verification, discovery and incentive layers of this protocol never look inside a block, and a decoder cannot use one without knowing what it is. The two facts are reconciled by one source-signed record, the **`STREAM_DESCRIPTOR`** (Appendix D §D.4.20), which carries what the decoder needs and nothing the protocol reads: a `ContentType` (`VIDEO_CMAF`, `VIDEO_ANNEXB`, `AUDIO`, `OPAQUE`), a `LayerMode` (`SVC_SPATIAL`, `SVC_TEMPORAL`, `SIMULCAST`, `INDEPENDENT`), and per layer a `CodecTag`, resolution, frame rate, bitrate and the **initialisation data** the container requires before any media segment — the `ftyp`+`moov` of fMP4/CMAF, the parameter sets of Annex-B — up to 4 KB per layer. The Stream Record carries the descriptor's version and hash (Ch2 §2.3.3); a joiner fetches the body from its first parent with `MANIFEST_REQUEST(0xFD)` alongside the manifests it already needs (Ch4 §4.3.1), so playback can begin the moment segment $X$ is verified.

**The payload of layer $l$ in a chunk** is the media bytes of that layer for that 250 ms in the container the descriptor names — for `VIDEO_CMAF`, one `moof`+`mdat` media segment per layer per chunk, decodable after the initialisation segment; for `VIDEO_ANNEXB`, the layer's NAL units. It is cut into 16 KB blocks, the last block of the layer zero-padded, with the exact byte length in the chunk `MANIFEST`'s `LayerByteLength` (Appendix D §D.4.8). Layers are separate byte streams that the decoder combines as `LayerMode` says (Ch1 §1.2.4 §4.2.2), so a shed layer is simply a byte stream that stops. `OPAQUE` content — audio-only, data, game state — rides the same machinery with an application-defined `CodecTag`; the protocol is payload-agnostic by construction, and this is the record that makes that a usable property rather than an accident.

An earlier draft said only that a segment is "serialized into a binary payload" and named H.264/SVC and AV1 as examples; a browser viewer written from it could verify every block of a segment and still have nothing it could append to a media source, and a publisher and viewer written independently would have invented two incompatible side channels for the one byte range that gates every viewer.

## Chunks: The Signed Unit Is Smaller Than the GOP

The GOP is the *decoding* unit; it is not the *signing* unit. A manifest signs a Merkle root over a set of blocks, so the source cannot sign until the last block in that set is encoded, and verify-before-forward (§2) means no relay may emit any block of the set before the manifest exists. If the set were the whole segment, every segment would wait a full second at the source before its first byte left — a barrier that appears nowhere in the hop-count latency model of Ch1 §1.1.3 and that alone consumes a fifth of the playout budget.

Each segment is therefore cut into **$C = 4$ chunks of $250\text{ ms}$**, and the source signs one `MANIFEST` per chunk (Appendix D §D.4.8) as soon as that chunk's blocks are encoded. Blocks within a chunk are numbered $j = 0, 1, \ldots$ layer-major (all of $L_0$'s blocks, then $L_1$'s, …), and the **global block index** used in every frame is

$$\text{BlockIndex} = (\text{ChunkIndex} \ll 12) \;|\; j, \qquad j < 4096$$

so the chunk a block belongs to is `BlockIndex >> 12` and no frame needs a separate chunk field. $4096$ blocks is $64$ MB per chunk, far above any live bitrate. Segment sequence numbers, the live-edge anchor and the retention window (Ch4 §4.3.1) remain per segment; only the signature, the Merkle tree and the per-layer block counts are per chunk.

The barrier is now $250$ ms and the source begins emitting a chunk's symbols the moment its manifest is signed, pacing them over at most half the chunk period (Ch3 §3.3.2) so the forest sees a continuous stream rather than four bursts per second.
