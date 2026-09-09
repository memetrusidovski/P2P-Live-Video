# SOLUTION-043: New Trees Are Pre-Emitted During the Window; Shedding Gets a Grace Period

**Closes:** ISSUE-043 (High)
**Lives in:** `protocol/chapter1/1.2_multi_forest_overlays/4_stream_slicing_architecture.md` §4.5 (steps 1, 3, 4, 5 rewritten; content-change and flash-crowd notes); `protocol/chapter1/1.1_scale_latency/5_capacity_adaptation.md` §5.3 item 6, §5.4 ($K_S$); `protocol/chapter5/5.2_proof_of_upload/1_receipt_cryptography.md` (bitmap semantics); `appendix_d_frame_registry.md` §D.4.8; `appendix_b_parameters.md` ($\tau_{\text{grace}}$, $K_S$)
**Class:** A choreography step written as a requirement with no mechanism that could satisfy it (recurring pattern #4)

---

## The problem in one line

Step 4 of the resize said every peer acquires a parent in each new tree before `EffectiveSegmentSeq`, but nothing was emitted on a new tree before the switch, so nothing could warm up in it, the only joinable parent was the source, and the shed rule then fired on the post-switch fill-in — at every one of the four rungs a growing stream climbs.

## The decision

*   **The source pre-emits every tree that exists only in the new layout**, from the announcement, in parallel with the old layout. A new tree's stripe is always a subset of blocks the old layout already carries, and `BlockIndex` does not depend on the matrix, so to any receiver the pre-emitted blocks are duplicates dropped by the ordinary check. Movers warm up on them inside the window and accept children as their old children leave.
*   **The source's budget is stated**: $K_S(m) = \lfloor (u_S - R_{\text{src}}) / (M B_m \Omega) \rfloor$ ordinary slots; pre-emit slots per new tree come from whatever headroom the old layout's committed egress leaves. The source is never asked to exceed $u_S$; what does not fit fills in after the switch.
*   **Grace period**: for every tree whose matrix entry changed, failed rounds do not count toward the shed threshold until `EffectiveSegmentSeq` $+ \tau_{\text{grace}}$, $\tau_{\text{grace}} = D_{\max} = 8$ segments — one warm-up window per level of the deepest permitted tree. A peer without a parent renders without the layer and repairs from the PULL zone meanwhile.
*   Step 4 is restated honestly; the resize's content changes to *existing* trees and the flash-crowd $2 \to 6$ jump are traced explicitly; a fold (SOLUTION-040) uses the same choreography with no new trees.

## Why this and not the alternatives

*   **Grace period alone** (the issue's second option) stops the spurious shed but leaves the new tree empty until the switch, so every subscriber of the top layer loses it for one warm-up window per level after the switch — $3$–$5$ s at the reference rungs. Pre-emission moves that fill-in inside the window, where nobody is watching the new tree yet.
*   **Lengthen the migration window to $D_{\max}$ segments.** Without pre-emission a longer window changes nothing (still nothing to warm on); with it, five segments cover four levels of fill-in and the grace covers the rest. A longer window also delays every ladder step, which the flash-crowd rule exists to avoid.
*   **Emit the whole new layout early on existing trees.** Impossible: existing trees change *content* at the switch (tree 4 goes from an $L_2$ stripe to an $L_1$ stripe at $5 \to 6$), and a relay cannot carry both.

## Defects found during verification

*   Fill-in arithmetic at $5 \to 6$, $N_{\text{relay}} = 30$: five movers at $K_v = 5$ per $1.5$ Mbps tree give $25$ slots plus the source's; three levels, $\approx 3.75$ s after the switch under the old text, with the shed at $+2$ s and restore at $+12$ s or later. Under pre-emission the same three levels complete inside the 5 s window.
*   The source's slot count had never been defined anywhere; every "the source's slots" in §4.5 was unquantified.
*   Pre-emission requires the receipt bitmap to mean "delivered by this uploader on this tree" rather than "first received": a child in the new tree that already holds every block from the old layout would otherwise issue empty receipts and be evicted after three segments. Ch5 §5.2.1 now says so.
*   Existing trees also change subscriber sets at a resize (tree 4 at $5 \to 6$ goes from 1080p viewers to everyone at 720p and above) and their $K_v(m)$ rises only at the switch; those peers join from the switch under the grace rule. This was not in ISSUE-043 and is now stated.

## The generalisable lesson

**A migration step phrased as "every peer does X before time T" must name the mechanism that makes X possible before T. If the resource X needs is created at T, the step is a wish.**

## Residual risk

*   Pre-emission costs the source $B_{\text{new}} \Omega$ per pre-emit child for five seconds; at a $2 \to 6$ jump that is $\approx 4.5\,\Omega$ Mbps per child across four trees, and a source at capacity pre-emits nothing — the grace rule then carries the whole fill-in.
*   The duplicate blocks a pre-emitted tree delivers to children who already hold them are wasted download for five seconds per resize; negligible against the segment rate.

## Validation owed (Chapter 8)

*   Top-layer PSR across a $5 \to 6$ resize at $N_{\text{relay}} = 30$, with and without pre-emission, with and without the grace rule.
*   Fill-in depth reached by `EffectiveSegmentSeq` as a function of source headroom.
*   Whether $\tau_{\text{grace}} = 8$ is longer than needed once pre-emission is in place.
