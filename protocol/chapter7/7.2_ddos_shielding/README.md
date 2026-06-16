# 7.2 DDoS Shielding & eBPF

This subchapter explains how the protocol shields high-capacity root nodes from socket saturation using kernel-level packet drops.

## Deep Dive Topics:
*   [1. Token Bucket Math](1_token_bucket_math.md): Relay clusters and shielding logic.
*   [2. XDP Kernel Implementation](2_xdp_kernel_ebpf.md): The NIC-level C-code filter to drop floods before user-space.
