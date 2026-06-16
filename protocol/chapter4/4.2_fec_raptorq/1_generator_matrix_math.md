# 1. Generator Matrix Math (RFC 6330)

To achieve reliable real-time packet distribution over lossy UDP/IP networks without the high latency penalty of ARQ (Automatic Repeat reQuest) retransmissions, our protocol incorporates a **RaptorQ Forward Error Correction (FEC)** layer. RaptorQ is a systematic fountain code with linear-time encoding and decoding complexity.

Let a video segment be divided into $K$ source symbols: $\mathcal{S} = \{s_0, s_1, \dots, s_{K-1}\}$. The RaptorQ encoder mathematically maps these source symbols to an infinite stream of encoding symbols:
$$\mathcal{E} = \{e_0, e_1, e_2, \dots\}$$

The encoding symbols are generated via a generator matrix $G$:
$$[e_0, e_1, \dots, e_{K+E-1}]^T = G \cdot [s_0, s_1, \dots, s_{K-1}]^T$$

The receiver can fully reconstruct the $K$ original source symbols upon receiving *any* subset of $K'$ encoding symbols (where $K' \ge K$). The probability of successful decoding $P_{\text{decode}}$ is defined by:
$$P_{\text{decode}}(K') \ge \begin{cases}
  0.99 & \text{if } K' = K \\
  0.9999 & \text{if } K' = K + 1 \\
  0.999999 & \text{if } K' = K + 2
\end{cases}$$
