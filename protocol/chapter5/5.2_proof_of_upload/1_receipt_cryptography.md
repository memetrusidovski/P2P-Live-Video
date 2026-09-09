# 1. Receipt Cryptography (Non-Repudiable Ledger)

To prevent cheating, collusion, or Sybil reputation pumping (where fake nodes claim to upload to each other), we enforce a zero-trust cryptographic accounting ledger based on **Proof-of-Upload (PoU) Receipts**.

A receipt is a statement, signed by the *downloader* $B$ with the key bound to its S/Kademlia identity, that $B$ received and Merkle-verified a set of blocks from uploader $A$. It is non-repudiable because only $B$ can produce it, and it is third-party verifiable because it names both parties and the stream.

```text
       Peer A (Uploader)                     Peer B (Downloader)
              |                                       |
              | --- segment k, tree m: ~12 blocks --->|   (pushed over ~1 s, each verified)
              |                                       |
              | <-- PoU receipt (segment k, tree m) --|   one signed receipt per segment per tree
              |     block bitmap + link loss rate     |
```

## One Receipt per Segment per Tree

A receipt covers **one segment in one tree**: it carries a bitmap of the blocks of that segment (in that tree's stripe) that $B$ verified, plus the loss rate $B$ measured on the link. An earlier design issued one signed receipt per 16 KB block. That is $\approx 7.6$ receipts per second per child per tree at 1 Mbps, and at the fan-out multi-tree super nodes exist to provide it does not scale:

| Node | Children | Per-block receipts | Per-segment receipts |
| :--- | ---: | ---: | ---: |
| Home relay, $K_v = 10$ | 10 | 76 /s | 10 /s |
| 100 Mbps relay | 100 | 760 /s | 100 /s |
| 10 Gbps super node | $\approx 8{,}700$ | $\approx 66{,}000$ /s — 4–8 cores of Ed25519, $\approx 120$ Mbps inbound, $\approx 4$ M receipts (~800 MB) per 60 s window | $\approx 8{,}700$ /s — under one core, $\approx 17$ Mbps, $\approx 520$ k receipts per window |

The per-segment receipt is the unit the incentive layer runs on. Its payload (Appendix D §D.4.12):

$$\text{PoU} = \text{Sign}_{SK_B}\bigl(\text{SegmentSeq} \parallel \text{TreeID} \parallel \text{RxFlags} \parallel \text{LossRate} \parallel \text{BlockBitmap} \parallel K_{\text{avail}} \parallel \text{AssignedTrees} \parallel \text{NodeClass} \parallel K_s \parallel \text{NodeID}_A \parallel \text{NodeID}_B \parallel \text{Timestamp}\bigr)$$

*   **`BlockBitmap`** — bit $\text{ChunkIndex} \cdot 128 + j$ is set iff block $(\text{ChunkIndex}, j)$ of the segment (Ch4 §4.1.1) was **delivered by $A$ on this tree** and verified — whether or not $B$ already held the block from another path, which matters during a migration window when the source pre-emits a new tree's stripe (Ch1 §1.2.4 §4.5) and a child may receive the same block on two trees. The receipt's **byte value** is $16\text{ KB} \times \text{popcount}$; it is what the contribution score counts (§2).
*   **`K_avail`, `AssignedTrees`, `NodeClass`** — $B$'s **child advertisement**: its free slots in `TreeID` against that tree's declared bitrate, its current assignment bitmap and its class. The receipt is the one child → parent frame that recurs once per segment per tree, so it is what a parent builds its per-tree `ROSTER` from (Ch1 §1.2.3, Appendix D §D.4.15); a third party reading the receipt ignores these three fields.
*   **`LossRate`** — $\lfloor 255 \rho \rfloor$, the fraction of RaptorQ symbols $B$ lost on this link over the trailing 2 s. This is the child→parent loss report that adaptive parity (Ch4 §4.2.2) and the slot count's overhead factor (Ch1 §1.2.1) consume; it needs no frame of its own.
*   **`RxFlags`** — `RELAYED` (bit 0): the blocks reached $B$ through an emergent relay bridge, and this receipt is issued *to the relay* (Ch6 §6.3.2, weighted $3\times$). `PULL` (bit 1): the blocks were served in response to $B$'s `PULL_REQUEST`s rather than pushed; `TreeID` is then `0x00` and the bitmap may span every tree's blocks.
*   **`TreeID`** — the tree whose stripe the receipt covers, so that an auditor can tell a legitimate mutual-parent pair (A serves B in $T_1$, B serves A in $T_3$) from a colluding one (Ch5 §5.3.1).

## Deadline and Consequence

Receipts are judged on a **segment** clock: the receipt for segment $k$ in tree $m$ is due at $A$ before $A$ sends $B$ the first block of segment $k+2$ — at least one full segment period plus the path RTT. A child that misses that deadline is **choked from PULL service at the next Tit-for-Tat cycle** ($\le 500$ ms later, Ch5 §5.1.2) and **dropped from the tree after three consecutive unreceipted segments** (`DISCONNECT`, reason `EVICTION`). An earlier draft demanded each receipt within 50 ms of block delivery; a receipt cannot arrive in less than one RTT, so on the spec's own 80 ms reference link every honest child would have been evicted after its first block.

> **Known Limitation — Reveal-Before-Prove Asymmetry.** $A$ delivers before $B$ can sign, so a $B$ that accepts blocks and never signs obtains up to three segments of one tree's stripe for free before it is dropped — $\approx 3 \times 0.75$–$1.5$ Mb, a few hundred kilobytes. The bound that matters is **not** the cost of a fresh identity: a static puzzle is $\approx 10$ ms of one CPU core (Ch2 §2.2.2), so an attacker can rotate NodeIDs faster than $A$ can evict them. The bound is **address diversity**. The eviction is recorded against the `/24` (IPv4) or `/48` (IPv6) prefix as well as the NodeID, a prefix that has been evicted cannot re-enter $A$'s trees for the same cooldown, and a prefix may never hold more than $\max(1, \lceil K_v(m)/\min(P_{\text{obs}}, 20) \rceil)$ of $A$'s child slots per tree (Ch2 §2.2.2). The free-riding rate is therefore bounded per prefix at a few hundred kilobytes per cooldown period, and an attacker's throughput scales with the number of distinct prefixes it controls — an expensive, not a computational, resource. A full commit-reveal scheme would eliminate the asymmetry at one extra RTT per block, incompatible with the propagation budget; bounded exposure is the correct trade.
