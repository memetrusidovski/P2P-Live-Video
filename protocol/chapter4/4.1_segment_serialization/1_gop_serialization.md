# 1. GOP and Serialization Constraints

To achieve ultra-low latency with high scalability, the primary video stream is encoded with strict Group of Pictures (GOP) constraints. The encoder must generate closed-GOP segments of exactly $1.0\text{ second}$ duration (typically 60 frames for a 1080p60 stream). This ensures that every segment is completely self-contained and decodable without reference to prior segments.

Each $1.0$-second segment is serialized into a binary payload. For a $6\text{ Mbps}$ video stream, the average segment size is:
$$\text{SegmentSize} = 6\text{ Mbps} \cdot 1.0\text{ s} = 6\text{ Mb} = 750\text{ KB}$$
