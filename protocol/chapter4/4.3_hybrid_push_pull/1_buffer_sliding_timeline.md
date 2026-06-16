# 1. Buffer Sliding Timeline

To balance propagation speed with network-loss resilience, a peer's playout buffer operates across a strict sliding timeline divided into two functional zones:

```text
                                  Buffer State Sliding Timeline
  <-------------------------------------------------------------------------------------------->
  [ Playout Deadline (T = 0s) ] <--- PULL Zone (Mesh) ---> | <--- PUSH Zone (Trees) ---> [ Live Edge (T = 4.0s) ]
  |                                                        |                                                    |
  v                                                        v                                                    v
  Video Decoder assembly                               Missing symbol blocks                        Proactive slice delivery
  must be finalized.                                   requested via bitfields.                     from parent trees.
```

1.  **PUSH Zone (Live Edge to $T_{\text{now}} - 1.5\text{ s}$):** Chunks are proactively pushed down the pre-configured Multi-Forest tree paths. No explicit request messages are sent, achieving propagation speeds close to IP multicast.
2.  **PULL Zone ($T_{\text{now}} - 1.5\text{ s}$ to Playout Deadline):** If a child detects missing blocks due to UDP packet drops, it switches to reactive mesh-pull mode, requesting missing symbols from its active neighbor set.
