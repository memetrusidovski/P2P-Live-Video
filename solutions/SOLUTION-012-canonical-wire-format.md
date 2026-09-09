# SOLUTION-012: Canonical Wire Format and the Frame Registry

**Closes:** ISSUE-012 (High)
**Lives in:** `protocol/appendix_d_frame_registry.md` (new), `schemas/p2p_live.proto`, frame diagrams across Ch2/Ch3/Ch5
**Class:** Interoperability / normative structure

---

## The problem in one line

The spec described two incompatible wire formats — binary frames over UDP/QUIC in the chapters, and a gRPC service (implying HTTP/2 over TCP) in the protobuf schema — plus a validation block that was 152 bytes in one chapter and 80 in another, and a dozen frames referenced but never specified.

## The decision

**Binary frames are canonical. The proto is a non-normative logical field reference.**

The structural fix is `appendix_d_frame_registry.md`: one normative registry holding the common 4-byte header, the transport mapping, the full type-code table, and byte layouts for every previously unspecified frame.

## Why the proto had to be demoted rather than reconciled

gRPC cannot carry this protocol's DHT layer, and not for stylistic reasons: **DHT RPCs precede any session.** A joining peer must send `PING` and `FIND_NODE` before it has a QUIC connection or a verified peer, which is exactly why those frames carry the 152-byte validation block inline. gRPC presumes an established HTTP/2 connection, so adopting it would mean either a TCP handshake before every DHT probe or a second transport stack alongside QUIC. The chapters were right and the schema was wrong; there was no middle position to reconcile toward.

## The design rules the registry establishes

Three rules do most of the work, and they are worth stating as rules rather than as a list of frames:

1. **Version lives in the frame header, nowhere else.** Per-message version fields in the proto were removed. One authority per fact.
2. **Transport is a property of the frame, declared once.** Plain UDP for pre-session DHT frames; QUIC bidirectional streams for control; QUIC datagrams for media symbols. This is what makes rule 3 well-defined.
3. **The validation block rides only on pre-session frames.** Post-handshake traffic is already authenticated by QUIC plus the per-session HMAC. Repeating 152 bytes on every media packet would be pure overhead — at ~750 pps that is roughly 0.9 Mbps of pointless signature per link.

## The rule with the widest reach: no self-declared IP

The proto's `SKademliaIdentity.external_ip` was deleted, and the principle behind that deletion generalises well beyond one field:

> The verifier checks the dynamic PoW against the **observed UDP source address**, never against anything the sender asserts about itself.

It is strictly stronger than trusting a claim, it removes a spoofing vector, and it makes the block layout independent of address family. Removing a field made the protocol *more* secure, not less — worth remembering when a layout looks incomplete.

## Three inconsistencies that survived the fix

The applied resolution corrected the chapters it touched and left three artefacts, all of which would have produced non-interoperating implementations:

**1. The registration signature still covered a self-declared IP.** Ch2 §2.3.2 defined $\text{Payload} = \text{NodeID} \parallel IP \parallel \text{Port} \parallel \text{Timestamp}$ — contradicting the observed-source-address rule established two sections earlier, and referring to an `IP` field that the `REGISTER_PEER` frame diagram does not contain. Corrected to $K_s \parallel \text{NodeID} \parallel \text{Port} \parallel \text{Timestamp}$.

**2. `GET_PEERS` was specified to carry the Stream Record, but its frame diagram had no field for it.** SOLUTION-004 depends on the record arriving in the `GET_PEERS` response — it is what makes live-edge sync cost zero extra round-trips — and Appendix D said so in prose. The byte layout was never updated, so an implementer following the diagram would omit it and every joiner would fail to anchor. Added as a length-prefixed trailing field, with $L = 0$ defined to mean "no current record" rather than being left to guesswork.

**3. `REGISTER_PEER` carries two Ed25519 signatures with no stated reason.** This reads as redundancy and invites an implementer to remove one. It is not redundant, and the distinction is worth recording: the **validation-block signature** authenticates *this datagram* and matters only on arrival; the **registration signature** authenticates a *durable statement* the guardian stores and re-serves to third parties who never saw the original packet. Same stand-alone-verifiability property that puts the downloader's NodeID inside a PoU receipt.

## The generalisable lesson

**A prose amendment to a frame is not an amendment to the frame.** All three defects have the same shape: a chapter's text was updated and the corresponding ASCII byte layout was not, leaving the normative artefact — the thing an implementer actually builds from — describing the old behaviour. For a wire format specifically, the diagram is the specification and the prose is commentary. Any future change to a frame should be checked at the diagram first.

## Validation owed

* A conformance vector suite: one canonical encoded example per frame type in Appendix D, so implementations can be checked against bytes rather than against prose.
* Arithmetic check that every layout's field sizes sum to a value consistent with its `PayloadLength`. The 152-byte block verifies ($32{+}32{+}8{+}8{+}8{+}64$); the rest are not yet checked mechanically.
