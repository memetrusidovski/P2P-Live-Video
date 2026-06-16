# 4.3 Hybrid Push-Pull Scheduling Logic

This subchapter explains the dual-phase buffer strategy, moving from aggressive pushing at the live edge to reactive pulling as deadlines approach.

## Deep Dive Topics:
*   [1. Buffer Sliding Timeline](1_buffer_sliding_timeline.md): The boundary between the PUSH zone and PULL zone.
*   [2. Bitfield Frames](2_bitfield_frames.md): Compressed Bloom filters and state arrays.
*   [3. Rarest-First Heuristics](3_rarest_first_heuristics.md): The chunk-urgency algorithm for reactive pulling.
