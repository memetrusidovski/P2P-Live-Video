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
#define RATE_PPS_UNKNOWN   50      /* handshakes + pre-session DHT frames; keyed by IPv4,
                                    * so sized for a CGNAT address shared by many (see 1) */
#define BURST_UNKNOWN      100
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

/* Ceiling on new-connection work, independent of source diversity.
 *
 * PERCPU_ARRAY is used to keep the hot path lock-free, which means each CPU
 * holds its OWN bucket: the aggregate ceiling is (per-CPU rate x online CPUs),
 * not the per-CPU rate. User space therefore MUST populate this map with
 *     rate = GLOBAL_NEW_PPS / num_online_cpus()
 * at load time. Writing GLOBAL_NEW_PPS directly into each CPU's bucket — the
 * obvious reading of the constant — yields a ceiling N_cpu times higher than
 * intended (160,000 pps on a 32-core host, not 5,000).
 *
 * The trade-off accepted here: RX-queue hashing may load CPUs unevenly, so one
 * CPU can exhaust its share while others idle. The alternative, a single shared
 * atomic counter, costs a cross-CPU cacheline bounce on every packet. */
struct {
    __uint(type, BPF_MAP_TYPE_PERCPU_ARRAY);
    __type(key, __u32);
    __type(value, struct bucket);
    __uint(max_entries, 1);
} global_new SEC(".maps");

/* Classic token bucket: refill by elapsed time, cap at burst, spend one token.
 * Returns 1 if the packet may pass, 0 if it must be dropped.
 *
 * Concurrency: XDP runs simultaneously on every RX queue, so two CPUs may
 * read-modify-write the same bucket. The accounting is therefore approximate
 * by design (a per-CPU-locked bucket would cost a cacheline bounce per packet,
 * which is exactly what XDP exists to avoid) — but it must never become
 * *unbounded*, which is what the two clamps below prevent. */
static __always_inline int bucket_allow(struct bucket *b, __u64 now)
{
    __u64 cap = (__u64)b->burst * NS_PER_SEC;
    __u32 rate = b->rate ? b->rate : 1;

    /* Clamp 1 — unsigned underflow. Another CPU may already have advanced
     * last_refill_ns past `now`. Plain subtraction would wrap to ~2^64,
     * refill the bucket to cap on every packet, and silently disable the
     * limiter for precisely the multi-queue flood it is meant to stop. */
    __u64 delta = (now > b->last_refill_ns) ? (now - b->last_refill_ns) : 0;

    /* Clamp 2 — multiplication overflow. A long-idle entry can accumulate a
     * delta large enough that delta*rate exceeds u64; clamping to the delta
     * that already fills the bucket is both safe and exact. */
    __u64 max_delta = cap / rate;
    if (delta > max_delta)
        delta = max_delta;

    b->tokens += delta * rate;
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

    /* Non-first fragments carry no UDP header, so they cannot be classified by
     * port. The protocol never fragments (symbols are sized to fit the MTU),
     * so a fragment on this path is either an attack or misconfiguration. */
    if (ip->frag_off & bpf_htons(0x1FFF))
        return XDP_DROP;

    /* The UDP header sits at ihl*4, NOT at a fixed 20 bytes. Assuming 20
     * misparses any packet carrying IP options: the port check then reads
     * payload bytes, so an attacker can dodge the filter (or trip it against
     * unrelated traffic) simply by setting an option. Bounds are explicit so
     * the verifier can prove the access. */
    __u32 ihl_bytes = ip->ihl * 4;
    if (ihl_bytes < sizeof(*ip) || ihl_bytes > 60)
        return XDP_DROP;

    struct udphdr *udp = (void *)ip + ihl_bytes;
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
            /* Spend a token for THIS packet as the bucket is created, or a
             * source that is seen once gets burst+1 rather than burst. */
            .tokens         = (__u64)(BURST_UNKNOWN - 1) * NS_PER_SEC,
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
