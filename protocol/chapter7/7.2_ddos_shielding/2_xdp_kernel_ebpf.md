# 2. XDP Kernel Implementation

The filter enforces the two-tier budget of §1: a real token bucket for allowlisted peers (populated from user space after identity validation), and a strict default-deny budget for everything else arriving on the protocol's data port.

```c
#include <linux/bpf.h>
#include <linux/if_ether.h>
#include <linux/ip.h>
#include <linux/udp.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_endian.h>

#define DATA_PORT          7777    /* protocol UDP port (configured at load time) */

#define MAX_PEERS          65536   /* allowlist: super node neighbor set + churn headroom */
#define MAX_UNKNOWN        16384   /* LRU: spoof floods evict each other, never peers   */

#define RATE_PPS_PEER      2000    /* ~750 pps steady state + PULL/FEC headroom (see 1) */
#define BURST_PEER         1000    /* QUIC is bursty; bucket must absorb a full burst   */
#define RATE_PPS_UNKNOWN   10      /* enough for a handshake / pre-session DHT frames   */
#define BURST_UNKNOWN      20
#define GLOBAL_NEW_PPS     5000    /* ceiling on total handshake work from all sources  */

#define NS_PER_SEC         1000000000ULL

struct bucket {
    __u64 tokens;          /* scaled by NS_PER_SEC to avoid floating point */
    __u64 last_refill_ns;
    __u32 rate;            /* packets per second */
    __u32 burst;           /* bucket capacity in packets */
};

/* Populated by user space AFTER the QUIC handshake and S/Kademlia identity
 * validation succeed; entries are deleted on disconnect or eviction.
 * The kernel enforces the result of validation — it never performs it. */
struct {
    __uint(type, BPF_MAP_TYPE_HASH);
    __type(key, __u32);            /* peer IPv4 source address */
    __type(value, struct bucket);
    __uint(max_entries, MAX_PEERS);
} peer_allowlist SEC(".maps");

/* First-contact budget for sources not yet allowlisted. LRU so that a flood of
 * spoofed sources evicts only other unknown sources. */
struct {
    __uint(type, BPF_MAP_TYPE_LRU_HASH);
    __type(key, __u32);
    __type(value, struct bucket);
    __uint(max_entries, MAX_UNKNOWN);
} unknown_budget SEC(".maps");

/* Global ceiling on new-connection work, independent of source diversity. */
struct {
    __uint(type, BPF_MAP_TYPE_PERCPU_ARRAY);
    __type(key, __u32);
    __type(value, struct bucket);
    __uint(max_entries, 1);
} global_new SEC(".maps");

/* Classic token bucket: refill by elapsed time, cap at burst, spend one token.
 * Returns 1 if the packet may pass, 0 if it must be dropped. */
static __always_inline int bucket_allow(struct bucket *b, __u64 now)
{
    __u64 delta = now - b->last_refill_ns;
    __u64 cap   = (__u64)b->burst * NS_PER_SEC;

    b->tokens += delta * b->rate;
    if (b->tokens > cap)
        b->tokens = cap;
    b->last_refill_ns = now;

    if (b->tokens < NS_PER_SEC)
        return 0;                  /* fewer than one whole token: drop */

    b->tokens -= NS_PER_SEC;
    return 1;
}

SEC("xdp")
int xdp_ratelimit_filter(struct xdp_md *ctx)
{
    void *data_end = (void *)(long)ctx->data_end;
    void *data     = (void *)(long)ctx->data;

    struct ethhdr *eth = data;
    if ((void *)eth + sizeof(*eth) > data_end)
        return XDP_PASS;

    /* IPv6 requires a parallel filter keyed on the 128-bit source address;
     * the maps above are IPv4-only. Non-IPv4 traffic is left to user space. */
    if (eth->h_proto != bpf_htons(ETH_P_IP))
        return XDP_PASS;

    struct iphdr *ip = (void *)eth + sizeof(*eth);
    if ((void *)ip + sizeof(*ip) > data_end)
        return XDP_PASS;

    if (ip->protocol != IPPROTO_UDP)
        return XDP_PASS;           /* not ours — do not filter unrelated traffic */

    struct udphdr *udp = (void *)ip + sizeof(*ip);
    if ((void *)udp + sizeof(*udp) > data_end)
        return XDP_PASS;

    /* Only the protocol's own data port is policed. */
    if (udp->dest != bpf_htons(DATA_PORT))
        return XDP_PASS;

    __u64 now     = bpf_ktime_get_ns();
    __u32 peer_ip = ip->saddr;

    /* Tier 1: established, identity-validated peer. */
    struct bucket *b = bpf_map_lookup_elem(&peer_allowlist, &peer_ip);
    if (b)
        return bucket_allow(b, now) ? XDP_PASS : XDP_DROP;

    /* Tier 2: unknown source. Must fit inside BOTH the per-source handshake
     * budget and the global new-connection ceiling — otherwise a spoofed-source
     * flood (each address seen once) would bypass per-source limits entirely. */
    __u32 zero = 0;
    struct bucket *g = bpf_map_lookup_elem(&global_new, &zero);
    if (!g || !bucket_allow(g, now))
        return XDP_DROP;

    struct bucket *u = bpf_map_lookup_elem(&unknown_budget, &peer_ip);
    if (!u) {
        struct bucket fresh = {
            .tokens         = (__u64)BURST_UNKNOWN * NS_PER_SEC,
            .last_refill_ns = now,
            .rate           = RATE_PPS_UNKNOWN,
            .burst          = BURST_UNKNOWN,
        };
        bpf_map_update_elem(&unknown_budget, &peer_ip, &fresh, BPF_ANY);
        return XDP_PASS;           /* first contact: allow the handshake attempt */
    }

    return bucket_allow(u, now) ? XDP_PASS : XDP_DROP;
}

char _license[] SEC("license") = "GPL";
```

**Default-deny is the key property.** An unknown source gets a handshake attempt and little more; sustained traffic from an address that never completes validation is dropped in the NIC driver, and the global ceiling bounds the damage from a flood that never repeats a source address. Legitimate media traffic is unaffected because allowlisted peers are policed at 2000 pps against a ~750 pps steady state.
