# SOLUTION-025: Slot Count and Sustainability Include FEC and Framing Overhead

**Closes:** ISSUE-025 (High)
**Lives in:** `protocol/chapter1/1.2_multi_forest_overlays/1_graph_theory_and_slicing.md` §1.3; `protocol/chapter1/1.1_scale_latency/1_bandwidth_paradox.md`, `5_capacity_adaptation.md` §5.1–5.2; `2_parent_selection_algorithm.md` (`advertise_K_avail`); `protocol/chapter4/4.2_fec_raptorq/2_systematic_dispersion.md`; `appendix_b_parameters.md`
**Class:** A count expressed at the media bitrate when the wire carries more (recurring pattern #1)

---

## The problem in one line

$K_v = \lfloor u_v / B_m \rfloor$ counted children at exactly the media bitrate, while every pushed slice actually costs $B_m \cdot (1 + E/K)(1 + f_{\text{frame}})$ — so every relay was oversubscribed by $15$–$40\%$, and the adaptive-parity loop turned that deficit into positive feedback.

## The decision

One expression owns the total:

$$K_v(m) = \left\lfloor \frac{(1 - r_{\text{pull}})\, u_v}{t_v \cdot B_m \cdot \Omega_v} \right\rfloor, \qquad \Omega_v = \left(1 + \frac{\bar{E}_v}{K}\right)\left(1 + f_{\text{frame}}\right)$$

with $f_{\text{frame}} = 0.085$ fixed, $\bar{E}_v$ the relay's current mean parity per block (recomputed per segment, initialised to $E_{\min}$), and $r_{\text{pull}} = 0.10$ reserved for PULL service. The same $\Omega$ and $r_{\text{pull}}$ appear in the multi-tree threshold, the global and per-tree sustainability conditions, the layer-set table, the source reserve, and both $\sigma$ thresholds. A $K_v$ that falls is honoured by draining, never dropping.

## Why this and not the alternatives

*   **A fixed 20% headroom** is simpler and was the issue's fallback suggestion. Rejected because the parity term is the *variable* part and is exactly what the feedback loop runs through: at the $30\%$ ceiling the true overhead is $42\%$, so a fixed 20% still oversubscribes the lossy links by $\sim 18\%$ — the regime where headroom matters most.
*   **Worst-link parity instead of mean parity.** Using $\max_c E_c$ protects every child but wastes slots when one child on a bad WiFi link drives the whole relay's count down. Total egress is $\sum_c B_m (1 + E_c/K)(1 + f)$, which the *mean* reproduces exactly; the mean is the right statistic for a bound on aggregate egress.
*   **Leaving $r_{\text{pull}}$ out of $K_v$** and budgeting PULL separately (as the earlier draft implicitly did — it did not budget it at all). PULL is real egress on the same link: late-joiner backfill alone is $\sim 3$ MB per join. Reserving it here is what lets Chapter 5 define a PULL budget that does not compete with tree slots.

## Defects found during verification

*   The $\sigma$ table in §1.1.5 was off by the whole overhead: "full quality at $\bar{u} \ge 6$ Mbps" is $7.7$ Mbps once every pushed byte carries its parity and headers and 10% of the link is held back; "depth proof at $8$ Mbps" is $10.2$. Both thresholds were $\sim 25\%$ optimistic.
*   §5.2's layer table already divided by $(1 + \text{Overhead}_{\text{fec}})$ while §1.3's slot count did not — two formulas for the same physical quantity in adjacent sections, one of them ignoring what the other included. Neither included framing.
*   The multi-tree threshold ("$u_v \ge 2B$") moves from $12$ to $\approx 13.8$ Mbps; a 12 Mbps node is single-tree again.
*   **Framing arithmetic:** 12 B `RAPTORQ_SYMBOL` header + 3 B QUIC datagram frame + ~11 B QUIC short header + 16 B AEAD tag + 28 B UDP/IPv4 $\approx 70$ B per 1024 B symbol ($6.8\%$); one `BLOCK_PROOF` ($\approx 204$ B plus its own stream framing) per 16 KB block ($\approx 1.5\%$). $f_{\text{frame}} = 0.085$. IPv6 adds 20 B per packet ($\approx 2\%$); the constant is conservative for v4 and slightly generous for v6, which Chapter 8 should tighten.

## The generalisable lesson

**Any count of "how many X can I afford" must be computed against what X actually costs on the wire, and the same expression must be used everywhere that cost appears.** Two sections each got half of the overhead right and neither noticed the other. The concrete check: grep for every occurrence of the divisor and confirm they are the same symbol.

## Residual risk

$f_{\text{frame}}$ is an estimate of QUIC header sizes that depends on connection-ID length and packet-number encoding. A relay that measures its own egress and finds it above the model should treat the measured ratio as $\Omega_v$; the spec does not yet say so.

## Validation owed (Chapter 8)

*   Measured egress per child against the model at $E = 1, 3, 5$ over IPv4 and IPv6.
*   Whether the damped parity loop actually converges under correlated last-mile loss, or oscillates between shedding a child and re-admitting one.
*   Whether $r_{\text{pull}} = 0.10$ covers late-joiner backfill at realistic join rates ($\sim 3300$ joins/s at $N = 10^6$ with 5-minute sessions ≈ 80 kbps per peer, well inside the reserve — but bursty).
