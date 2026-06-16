# 1. Token Bucket Math and Shielding Relays

At a million-peer scale, DDoS attacks (such as UDP packet floods) can quickly exhaust user-space CPU resources by forcing the operating system to continuously execute context switches between kernel space and the P2P application. 

**Shielding Relays:** High-tier supernodes closest to the source communicate only through dynamic, rotating public relays, hiding their physical IPs from the general public.

**Token-Bucket Filters:** Nodes implement eBPF-based Token Bucket filters in the OS kernel. This rate-limits incoming UDP traffic before it reaches user space:

```text
  [ Incoming Packet Flood ] ---> [ NIC Driver: XDP / eBPF filter ] ---> [ Linux Kernel Socket ]
                                               |
                                     (Exceeds limit? Drop!)
```

The eBPF program parses incoming UDP packets and checks their S/Kademlia validation blocks against a kernel-resident hash map of active active set peers. Unauthenticated packets or packets exceeding the rate limit are discarded immediately with zero CPU context-switch penalty.
