# 1. GOP and Serialization Constraints

To achieve ultra-low latency with high scalability, the primary video stream is encoded with strict Group of Pictures (GOP) constraints. The encoder must generate closed-GOP segments of exactly $1.0\text{ second}$ duration (typically 60 frames for a 1080p60 stream). This ensures that every segment is completely self-contained and decodable without reference to prior segments.

Each $1.0$-second segment is serialized into a binary payload. For a $6\text{ Mbps}$ video stream, the average segment size is:
$$\text{SegmentSize} = 6\text{ Mbps} \cdot 1.0\text{ s} = 6\text{ Mb} = 750\text{ KB}$$

## Chunks: The Signed Unit Is Smaller Than the GOP

The GOP is the *decoding* unit; it is not the *signing* unit. A manifest signs a Merkle root over a set of blocks, so the source cannot sign until the last block in that set is encoded, and verify-before-forward (§2) means no relay may emit any block of the set before the manifest exists. If the set were the whole segment, every segment would wait a full second at the source before its first byte left — a barrier that appears nowhere in the hop-count latency model of Ch1 §1.1.3 and that alone consumes a fifth of the playout budget.

Each segment is therefore cut into **$C = 4$ chunks of $250\text{ ms}$**, and the source signs one `MANIFEST` per chunk (Appendix D §D.4.8) as soon as that chunk's blocks are encoded. Blocks within a chunk are numbered $j = 0, 1, \ldots$ layer-major (all of $L_0$'s blocks, then $L_1$'s, …), and the **global block index** used in every frame is

$$\text{BlockIndex} = (\text{ChunkIndex} \ll 12) \;|\; j, \qquad j < 4096$$

so the chunk a block belongs to is `BlockIndex >> 12` and no frame needs a separate chunk field. $4096$ blocks is $64$ MB per chunk, far above any live bitrate. Segment sequence numbers, the live-edge anchor and the retention window (Ch4 §4.3.1) remain per segment; only the signature, the Merkle tree and the per-layer block counts are per chunk.

The barrier is now $250$ ms and the source begins emitting a chunk's symbols the moment its manifest is signed, pacing them over at most half the chunk period (Ch3 §3.3.2) so the forest sees a continuous stream rather than four bursts per second.
