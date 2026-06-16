# 2. XDP Kernel Implementation

```c
#include <linux/bpf.h>
#include <linux/if_ether.h>
#include <linux/ip.h>
#include <linux/udp.h>

#define SEC(NAME) __attribute__((section(NAME), used))
#define MAX_PEERS 1024
#define RATE_LIMIT_PPS 100 // Packets Per Second limit for control packets

struct {
    __uint(type, BPF_MAP_TYPE_HASH);
    __type(key, __u32); // Peer IP Address
    __type(value, __u64); // Last Packet Timestamp / Token Count
    __uint(max_entries, MAX_PEERS);
} peer_rate_map SEC(".maps");

SEC("xdp")
int xdp_ratelimit_filter(struct xdp_md *ctx) {
    void *data_end = (void *)(long)ctx->data_end;
    void *data = (void *)(long)ctx->data;

    struct ethhdr *eth = data;
    if ((void *)eth + sizeof(*eth) > data_end)
        return XDP_PASS;

    if (eth->h_proto != __constant_htons(ETH_P_IP))
        return XDP_PASS;

    struct iphdr *ip = (void *)eth + sizeof(*eth);
    if ((void *)ip + sizeof(*ip) > data_end)
        return XDP_PASS;

    if (ip->protocol != IPPROTO_UDP)
        return XDP_PASS;

    struct udphdr *udp = (void *)ip + sizeof(*ip);
    if ((void *)udp + sizeof(*udp) > data_end)
        return XDP_PASS;

    __u32 peer_ip = ip->saddr;
    __u64 *last_seen = bpf_map_lookup_elem(&peer_rate_map, &peer_ip);
    
    if (last_seen) {
        __u64 now = bpf_ktime_get_ns();
        __u64 delta = now - *last_seen;
        
        // Discard packet if delta is too small (exceeds PPS limit)
        if (delta < (1000000000 / RATE_LIMIT_PPS)) {
            return XDP_DROP; // Drop packet in NIC driver!
        }
        *last_seen = now;
    }

    return XDP_PASS;
}
```
