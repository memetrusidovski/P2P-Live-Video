# 3. Relay Frame Layouts

```text
RELAY_PROPOSAL Frame (Type 0x30):
Sent by a NAT-blocked node to a RELAY_CAPABLE superpeer (found via GET_PEERS with
ReqFlags.WANT_RELAY_CAPABLE, Ch2 §2.3.2) to request a bridge to Target for tree TreeID.
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|   Version     |  Type (0x30)  |          Payload Length       |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                      S/Kademlia Target NodeID                 |
|                            (32 bytes)                         |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|   Target Port Number (16-bit) |    TreeID     |   BindFlags   |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+

RELAY_BIND Frame (Type 0x31):
Sent by the Relayer to establish the active routing bridge.
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|   Version     |  Type (0x31)  |          Payload Length       |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                      S/Kademlia Client A ID                   |
|                            (32 bytes)                         |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                               |
|                      S/Kademlia Client B ID                   |
|                            (32 bytes)                         |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|    TreeID     |   BindFlags   |          Reserved (0)         |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

*   **`TreeID`:** the tree whose push is bridged from B to A (`0x00` = PULL service only). The relay charges $B_m \Omega$ of its PULL reserve per bridged tree (§6.3.1).
*   **`BindFlags`:** bit 0 `SOURCE_INGRESS` — see below. Other bits reserved.

## The Source Behind NAT

Every join path assumes the broadcaster can be reached: it registers under $K_s$ as a `RELAY`-class peer with `Flags.SOURCE` (Ch2 §2.3.2) and is the root of every tree. A broadcaster on a home connection behind CGNAT — a primary use case at $N = 2$–$10$ — cannot be. It **must** therefore satisfy the `RELAY` reachability requirement (`PUBLIC` or `CONE`, Ch1 §1.2.5), or bind one or more **ingress relays**:

1.  The source finds `RELAY_CAPABLE` peers exactly as any NAT-blocked peer does and sends `RELAY_PROPOSAL` with `BindFlags.SOURCE_INGRESS`, signed with $SK_{\text{Source}}$ inside the QUIC session so the relay can verify it is the publisher.
2.  The relay answers with `RELAY_BIND(SOURCE_INGRESS)`, registers under $K_s$ with `Flags.SOURCE_INGRESS` and `AssignedTrees` covering every tree, and becomes hop 0 of the forest: the source pushes all $M$ slices to it ($\Omega B$ of the source's upload per ingress relay) and it forwards them as the root would.
3.  Joiners treat a `SOURCE_INGRESS` record exactly as they treat the source. The hop penalty (§1.2.2) counts the ingress relay as depth 0.

Two ingress relays are recommended so that the root is not a single point of failure; the source's own reserve obligations (Ch1 §1.1.5 §5.4) are then met through them. A source that can bind no ingress relay cannot broadcast — the specification states this plainly rather than assuming reachability.
