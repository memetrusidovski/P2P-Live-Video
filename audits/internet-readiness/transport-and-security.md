# Transport and security on a hostile Internet

Area: `transport-and-security`. Scope: `bs-node` transport.rs, tls.rs and the quinn
configuration; the plain-UDP path (PING, PONG, DHT, PUNCH) from the socket to the
core's verifiers; bans and replay. Judged against a home publisher behind a port
forward with viewers behind home and mobile NATs, no servers.

Already filed, cited here and not repeated: ISSUE-084 (punch before the mapping
opens), ISSUE-086 (prefix caps not applied), ISSUE-087 (XDP allowlist keyed by
address). The tick walk (`audits/2026-11-02-tick-walkthrough.md`) records as
"holds, not filed": forged-block eviction after 3 blocks, the heartbeat as a signed
188-byte PING, and the ban keyed on NodeID so that 10 ms identity rotation defeats
escalation.

## What works today

- One UDP socket carries QUIC and plain frames, demultiplexed on the QUIC fixed bit
  (`src/crates/bs-node/src/transport.rs:150`). Plain sends that meet a full socket
  buffer are held in an 8192-entry backlog and flushed, never dropped
  (`transport.rs:85`).
- Sessions are mutual TLS 1.3 over QUIC with self-signed Ed25519 certificates. The
  SAN carries `bitstream://<nodeid>/<nonce>`; both ends check
  `Blake3(PK ‖ nonce) == NodeID` and the static puzzle, no CA
  (`src/crates/bs-node/src/tls.rs:63`). The TLS CertificateVerify proves key
  possession, so a man in the middle cannot impersonate a NodeID or read a session.
- The gossip HMAC key comes from the TLS exporter (`driver.rs:1125`), identical on
  both ends and unforgeable off path.
- quinn runs BBR, initial MTU 1280, MTU discovery on by default, 64 bidi streams,
  8 MB datagram buffers each way (`tls.rs:224-237`). A 1024-byte symbol plus frame
  and QUIC overhead is about 1110 bytes, under the 1232-byte IPv6 floor.
- Connection migration is QUIC-native; quinn validates the new path (ARCHITECTURE
  §6 row "Connection migration").
- Every pre-session frame carries a 152-byte validation block. The receiver checks
  the static puzzle, the dynamic puzzle against the observed source IP, and the
  Ed25519 signature (`src/crates/bs-crypto/src/identity.rs:106`). A PING is answered
  on static puzzle and signature alone so a NAT peer can learn its reflected
  address (`src/crates/bs-core/src/node.rs:2089`).
- Consensus evictions ban by NodeID for `tau_ban` = 600 s, doubling per strike to
  24 h, strikes kept for a day (`src/crates/bs-core/src/node/audit.rs:490`).
  Receipt-deadline evictions ban the NodeID and its /24 for 60 s
  (`src/crates/bs-core/src/node/incentive.rs:229`).
- A port-forwarded publisher and NAT viewers that rebind to the reflected address
  run end to end (`src/docs/MULTI_MACHINE.md`, "Over the Internet").

## What breaks on the Internet

Costs below are for one laptop core: Ed25519 verify about 50 µs, sign about 20 µs,
Blake3 of a 200-byte frame under 1 µs, a full server-side QUIC handshake about
250 µs. Identity minting at C1 = 16 costs about 10 ms. Solving the dynamic puzzle for
any IP at the 8 to 14 bit tiers costs 25 µs to 1.6 ms, once, and the nonce is valid
forever because nothing checks a timestamp.

### A spoofed PING flood takes any node offline at 20 Mbps

Path: `transport.rs:150` copies every non-QUIC datagram into an unbounded channel;
`driver.rs:749` hands each to `on_plain`; `node.rs:2089` verifies the static puzzle
and the signature, then signs and sends a PONG, whether or not the dynamic puzzle
binds the sender. No code between the socket and the core counts packets per
source. One PING costs the responder about 75 µs of CPU on the single driver task,
so 13,000 PINGs per second (188 bytes each, 20 Mbps) saturate it and the core
stops ticking: timers, pacing and playback stall. Above that rate the unbounded
channel grows at the excess, 16 MB/s at 100 Mbps, until the process is killed.
Without XDP (macOS, Windows, and every home Linux box) there is no kernel filter.

### PONG, FIND_NODE and GET_PEERS reflect toward a victim

A PING with the victim's source address yields a 163-byte PONG to the victim per
188-byte PING (factor 0.87, no amplification, but the responder pays the signing
cost and the victim sees the swarm as the source). A FIND_NODE or FIND_VALUE
request is about 190 bytes. Any relay-class node answers with up to 20 records
of 42 bytes (54 for IPv6) plus the block: 996 bytes IPv4, 1236 bytes IPv6, a
factor of 5.2 to 6.5 (`src/crates/bs-core/src/node/dht.rs:912`, `:969`). GET_PEERS caps at
20 records (`MAX_GET_PEERS_RECORDS`) with the same factor. The dynamic puzzle does
not stop this: the attacker solves it for the victim's IP in under 2 ms. The home
publisher is always a relay-class public contact, so 10 Mbps of spoofed FIND_NODE
makes it send 52 Mbps toward the victim, more than a home uplink, and starves its
own stream.

### A viewer can make the swarm PING a third party

`nat.rs:505`: a PUNCH_REQUEST Forward is accepted from any source whose validation
block binds to its own IP; the target then sends a PING to `requester.addr`. The
signed body is `requester.node_id ‖ stream_id` (`nat.rs:483`), not the address, and
nothing checks that the sender is a guardian or a session peer. One 230-byte
Forward per swarm node yields one 188-byte PING to the victim from that node. With
a 1000-node swarm learned from GET_PEERS, 1 Mbps of Forwards becomes about 0.8 Mbps
of PINGs from 1000 distinct sources. An on-path attacker can also rewrite the
address in a legitimate Forward.

### Every pre-session frame replays forever

`identity.rs:93` and `:106` check puzzles and signature only. The timestamp is
microseconds since the process started (`driver.rs:686`, `time.rs:8`), so no
receiver can compare it to its own clock. A captured REGISTER_PEER re-registers a
departed node at the guardian for `tau_ttl` on every replay
(`node/dht.rs:1016`); a captured PONG re-triggers `maybe_rebind`; a captured DHT
request draws a fresh 1 KB answer. Replayed accusations in `audit.rs:388` are
compared against the local uptime, so a remote accusation is dropped or accepted
by the difference between two machines' uptimes, not by its age.

### Unvalidated QUIC handshakes

`driver.rs:695` accepts every `Incoming` with no Retry and no cap. A spoofed
1200-byte Initial costs a 250 µs handshake attempt and up to 3600 bytes of
reflection (the RFC 9000 limit quinn enforces). 4000 Initials per second (38 Mbps)
pin a core. Banned prefixes are not consulted here or in `on_plain`; a ban changes
only NEIGHBOR and JOIN admission.

### Sessions die on a 4-second stall

`tls.rs:225`: idle timeout 4 s, keep-alive 1.5 s. A Wi-Fi roam, an LTE handover or a
CGNAT rebinding of more than 4 s closes every session at once. Re-attach then costs
a full handshake per peer because no resumption ticket is kept (see Missing
features), and the QUIC-native migration never gets to run.

### Slow uplinks serve stale symbols

`tls.rs:231`: an 8 MB datagram send buffer per connection. A relay whose uplink
falls under the paced rate queues symbols in arrival order. At a 0.75 Mbps stripe,
8 MB is 85 s of symbols; the playback deadline is 3 s. quinn drops the oldest only
when the buffer is full. A child then receives symbols for chunks it has already
starved on, while current chunks wait behind them.

## Bugs

1. **Validation blocks have no freshness or replay check, and the timestamp is
   process uptime.** Severity High.
   `src/crates/bs-crypto/src/identity.rs:93-119`, `src/crates/bs-core/src/node.rs:2047`,
   `src/crates/bs-node/src/driver.rs:686`, `src/crates/bs-core/src/node/audit.rs:388`.
   Reproduce: capture one REGISTER_PEER from a viewer, stop the viewer, replay the
   datagram to the guardian an hour later; `dht.store.register` accepts it and the
   record is served again. A test: feed `Node` the same `Frame::Ping` twice and
   assert the second is rejected (today both get a PONG).
   Fix: give the core a wall-clock offset at start (UTC µs), stamp blocks with it,
   reject blocks outside ±60 s, and keep a per-NodeID LRU of (frame type,
   timestamp) for that window. Effort medium. Self-contained: no (the core's time
   model and the spec's timestamp semantics are a design decision).

2. **The plain-datagram queue is unbounded.** Severity High.
   `src/crates/bs-node/src/transport.rs:69` (`mpsc::unbounded_channel`),
   `driver.rs:749`.
   Reproduce: blast a node with 100,000 random 188-byte non-QUIC datagrams per
   second for 60 s and watch RSS; `Frame::from_slice` failures are logged at debug
   and the queue is never drained faster than one decode per loop turn.
   Fix: a bounded channel of about 4096 with `try_send`, a dropped counter in
   `Status`. Effort small. Self-contained: yes.

3. **No per-source rate limit on plain frames; every PING is verified and
   answered with a signed PONG.** Severity High.
   `src/crates/bs-node/src/driver.rs:1087`, `src/crates/bs-core/src/node.rs:2089-2135`.
   Reproduce: 13,000 PINGs per second with spoofed sources against a viewer; the
   viewer's playback stalls (the core runs on the same task) and `vb_rejected`
   stays at zero because each PING is valid.
   Fix: in `on_plain`, before decode, a token bucket per source IP (the spec's
   `RATE_PPS_UNKNOWN` = 50, burst 100, in a 16k-entry LRU) and a global budget
   (`GLOBAL_NEW_PPS` = 5000); sources with an open session or membership record
   get the peer tier. In `on_ping`, cache verified (NodeID, static nonce) pairs so
   repeats skip the static puzzle. Effort medium. Self-contained: yes (driver only
   for the buckets).

4. **DHT answers reflect 5× toward a spoofed source.** Severity Medium.
   `src/crates/bs-core/src/node/dht.rs:912` (`dht_serve_find`), `:1028`
   (`dht_serve_get_peers`), `:969` (20 records).
   Reproduce: solve the dynamic puzzle for the victim's IP at the current tier
   (under 2 ms), send FIND_NODE with the victim's source address to the publisher;
   the publisher sends 996 bytes to the victim per 190-byte request.
   Fix: answer a 20-record request only to a source that completed a PING/PONG
   round from that address within the last minute (a reflection cookie); a first
   contact gets at most 3 records. Effort medium. Self-contained: no (protocol
   behaviour; the spec has no cookie).

5. **PUNCH Forward lets any identity make any node PING any address.** Severity
   Medium. `src/crates/bs-core/src/node/nat.rs:483-486` (signed body omits the
   address), `:505-520` (no sender check).
   Reproduce: with one valid identity, send `PunchFrame::Forward{requester.addr =
   victim}` to every record from GET_PEERS; each target sends a PING to the victim.
   Fix: sign `requester.addr` in the body; accept a Forward only from a guardian we
   registered with or a session peer; one Forward per requester NodeID per second.
   Effort small. Self-contained: yes.

6. **QUIC accept does no address validation and has no cap.** Severity Medium.
   `src/crates/bs-node/src/driver.rs:695-711`, `tls.rs:239`.
   Reproduce: 4000 spoofed Initials per second; CPU pins and each gets 3600 bytes
   of reply.
   Fix: `if !incoming.remote_address_validated() { incoming.retry() } else {
   incoming.accept() }`, `server_cfg.max_incoming(256)`, and `incoming.refuse()`
   for banned or over-budget source IPs. Effort small. Self-contained: yes.

7. **Bans never reach the transport, and the receipt-deadline ban does not
   escalate.** Severity Medium.
   `src/crates/bs-core/src/node/incentive.rs:229` (fixed 60 s),
   `src/crates/bs-core/src/node/audit.rs:64` (NodeID only), consulted only at
   `node.rs:1387` and `membership.rs:293`.
   Reproduce: evict a peer; it reconnects at once, completes a handshake, and its
   PINGs and DHT requests are verified and answered in full.
   Fix: an `Output::Ban { prefix, until }` the driver turns into a refuse list for
   `Incoming` and `on_plain`; strike counts in `ban_peer` keyed by prefix with the
   same doubling as `audit_evict`. Effort medium. Self-contained: no (new core
   output; the spec leaves the ban key open, audit §"Spec silences" line 412).

8. **A simultaneous open overwrites the live session.** Severity Medium.
   `src/crates/bs-node/src/driver.rs:1127-1141`.
   Reproduce: two peers dial each other within one RTT (both hold the other in the
   Passive Set after a parent loss). The second `Ev::Connected` replaces the
   `Conn`; the first connection's writers vanish while its readers keep feeding
   frames; `Ev::Closed` from either removes the entry and `SessionClosed` fires
   for a session still open.
   Fix: on `Connected` for a peer already in `conns`, keep the connection opened by
   the lower NodeID, close the other, and key `Closed` by `conn.stable_id()`.
   Effort small. Self-contained: yes.

9. **The 8 MB datagram send buffer serves symbols up to 85 s stale.** Severity
   Medium. `src/crates/bs-node/src/tls.rs:231`.
   Reproduce: cap a relay's uplink at half its stripe rate with `tc` or `dummynet`;
   its child logs symbols for chunks past the deadline and starves on current
   ones.
   Fix: size the buffer to two chunk periods of the connection's paced rate (about
   256 KB), or drop a queued symbol whose chunk deadline has passed. Effort small.
   Self-contained: yes.

10. **A 4 s idle timeout drops every session on a mobile handover.** Severity Low.
    `src/crates/bs-node/src/tls.rs:225-228`.
    Reproduce: pull the viewer's network for 5 s; every session closes; the
    repair starts with full handshakes.
    Fix: idle timeout 15 s, keep-alive 3 s. Liveness is the RTT-scaled eviction
    (Ch3 §3.3), not the QUIC idle timer. Effort small. Self-contained: yes.

11. **A session's verified NodeID is never compared with the record dialed.**
    Severity Low. `src/crates/bs-core/src/node.rs:1865`, `io.rs` (`OpenSession`
    carries only an address).
    Reproduce: a gossiper serves `{victim NodeID, attacker address}`; the joiner
    dials the attacker, accepts its own valid certificate, and attributes rank and
    ban state to the attacker's NodeID while treating the record as the victim's.
    Fix: carry the expected NodeID in `Output::OpenSession`, compare in
    `on_session_opened`, close on mismatch. Effort small. Self-contained: yes.

12. **`known_keys` and `observed_prefixes` grow without bound on verified
    blocks.** Severity Low. `src/crates/bs-core/src/node/incentive.rs:246`,
    `src/crates/bs-core/src/node.rs:677`.
    Reproduce: mint identities at 100 per second (10 ms each) and PING a node;
    `known_keys` gains 100 entries per second, about 36 MB per hour;
    `observed_prefixes` gains one per spoofed /24 and moves `prefix_cap`.
    Fix: an LRU of 4096 keys; count prefixes from sessions and membership only.
    Effort small. Self-contained: yes.

13. **No tests for tls.rs, transport.rs or the driver's hostile paths.**
    Severity Low. Zero `#[test]` in the three files; `tests/localhost.rs` only.
    A test that would fail today: a replayed PING gets no second PONG; a
    non-QUIC flood leaves RSS flat.
    Fix: see Missing features, hostile-network harness. Effort medium.
    Self-contained: yes.

## Missing features

| Feature | Why | Days |
|---|---|---|
| User-space rate limiting (the spec's two-tier buckets mapped onto the driver: per-IP and global budgets for plain frames and handshakes, allowlist from open sessions) | No XDP on macOS, Windows or home Linux; bug 3 and 6 are the symptoms | 3 |
| Replay and freshness for pre-session frames (UTC timestamp in the block, ±60 s window, per-NodeID LRU; spec Ch2 §2.2 claims replay protection the frame cannot deliver) | Bug 1 | 2 |
| 0-RTT re-attach with resumption tickets kept per NodeID (Ch3 §3.3.1 "must retain resumption tickets"). Today rustls stores tickets under the one server name `bitstream` (`tls.rs:24`), so a ticket from A is offered to B and refused; `into_0rtt` is unused | Re-attach costs 2 RTT instead of 1; after bug 10 fires, every repair pays it | 3 |
| Ban delivery to the transport and prefix-keyed escalation | Bug 7 | 2 |
| Reflection defence for DHT RPCs (cookie round or response cap for first contact) | Bug 4; needs a spec decision | 2 |
| Hostile-network test harness for `bs-node` (spoofed flood, replay, Retry, simultaneous open, uplink throttle) | Bug 13; nothing today exercises the driver against an attacker | 2 |
| XDP shield for Linux relays (Ch7 §7.2, with ISSUE-087 fixed) | Only for public Linux relays; home peers cannot run it | 5 |

## Open questions

- Must the home publisher serve DHT at all? It is the one guaranteed public
  relay-class contact, so it absorbs every FIND_NODE reflection and spends its own
  uplink on the answers.
- Does the spec intend the dynamic puzzle as a spoofing defence? At 8 to 14 bits it
  costs under 2 ms per (identity, IP), so it rate-limits identities but does not
  bind a packet to a source.
- What clock does the validation-block timestamp use? Ch2 §2.2 says it protects
  against replay; the core has only monotonic time since start.
- Bans by NodeID or by prefix, and does a plain-frame drop count as a strike?
  Audit line 412 lists this as a spec silence.
- Who owns the 3 s datagram queue bound: the driver (drop by deadline) or the core
  (stop pacing when the uplink falls behind)?
