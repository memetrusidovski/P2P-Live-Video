# NAT traversal on a real network

Area: `nat-traversal`. Date: 2026-11-09. Scope: `bs-core/src/node/nat.rs`, the PING/PONG
reflection in `bs-core/src/node.rs`, the DHT bootstrap in `bs-core/src/node/dht.rs`, and
the real driver in `bs-node/src/driver.rs`, `main.rs`, `tls.rs`. Paths below are relative
to `src/`.

Verdict: no NAT path has run through a real NAT, and the first one a home viewer hits
(bootstrap) fails before any traversal code runs. Everything in `nat.rs` is reachable
from the driver. The simulator that validates it never translates an address. So the
rebind to the reflected address, the one mechanism the Internet needs first, has no test.

## What works today

Every mechanism is implemented in the sans-I/O core and the driver carries its frames
without special casing. "Tested" names the only test that exercises the path.

| Path | Core | Driver | Tested |
|---|---|---|---|
| Reflection-only PONG: a PING whose static puzzle and signature verify is answered with the observed address even when the IP binding fails | `Node::on_ping`, `node.rs:2089-2134` | plain UDP, `driver.rs:1087-1103` (`on_plain`) and `send_plain` | nowhere (no test references `reflected`, `AddressRebound` or `on_reflection`) |
| Rebind, once, to the first verified reflection: re-solve the dynamic puzzle for the new IP, re-register | `Node::maybe_rebind`, `nat.rs:344-369` | the driver only logs `AddressRebound` (`driver.rs:1511`) | nowhere; the simulator configures `reachability` so `maybe_rebind` returns at `nat.rs:345` |
| Reachability from two PONG responders (PUBLIC, CONE, SYMMETRIC) | `Node::on_reflection`, `nat.rs:372-407` | `bsnode watch --nat auto` passes `reachability: None` (`driver.rs:148`, `main.rs:329`) | nowhere |
| PUNCH_REQUEST request form to the referrer, forward form to the target, target PINGs the requester, PING re-triggers the PROBE | `punch_if_needed`, `on_punch`, `reprobe_after_punch`, `nat.rs:419-539`; called before every PROBE at `node.rs:763` | both forms are plain UDP (`Channel::Udp`, `nat.rs:458`, `:501`) and the codec round-trips them (`bs-wire/src/frame.rs:261`). bsnode sends and forwards them; this is not simulator-only | `scenarios/nat_cone.toml` (gates `punches_sent >= 10`) |
| tau_nat keepalive (25 s) from a CONE registrant to its guardians | `nat_timers`, `nat.rs:574-587` | driven by the core timer, plain UDP PING | `nat_cone.toml` indirectly |
| RELAY_CAPABLE advertisement and its re-registration when the bit flips | `relay_capable`, `nat.rs:107-117`; `nat_timers`, `nat.rs:568-573` | n/a | `bs-core/tests/node.rs:921` with `relay_capable_uptime = 0` |
| RELAY_PROPOSAL / RELAY_BIND bridge for a peer with no reachable parent | `request_bridge` .. `nat_child_removed`, `nat.rs:642-954` | `Channel::Control` goes on the QUIC control stream (`driver.rs:1278-1296`) | `node.rs:921`, `nat_cone.toml` |
| Source behind a NAT: SOURCE_INGRESS proposals, publisher-key check (ISSUE-085) | `ingress_timers`, `on_ingress_proposal`, `on_ingress_bind`, `nat.rs:143-337` | `bsnode publish --nat cone|symmetric` sets the reachability (`main.rs:657`) | `scenarios/source_behind_nat.toml` with `relay_capable_uptime = 3000000` (3 s; `Duration` is microseconds) |
| One UDP socket for plain frames and QUIC (`DemuxSocket`, `driver.rs:380`, `:421`) | n/a | the NAT mapping a PING opens is the one QUIC uses; QUIC keepalive 1.5 s, idle 4 s (`tls.rs:231-234`) | `bs-node/tests/localhost.rs` on loopback only |
| Client-side QUIC path migration | n/a | quinn default (`ARCHITECTURE.md` §6 "Connection migration") | nowhere |

The identity binding is IP-only (`identity.solve_dynamic(addr.ip())`, `node.rs:449`,
`nat.rs:362`), so a NAT that rewrites the port does not break validation blocks. That is
the right choice.

## What breaks on the Internet

### A viewer behind a NAT never gets its first answer

Code path: `bsnode watch` detects its address by the route to the bootstrap
(`detect_ip`, `driver.rs:491-507`), which is the LAN address behind a NAT. `Node::start`
solves the dynamic puzzle for that LAN IP (`node.rs:449`). `dht_start` sends FIND_NODE to
the seed (`dht.rs:259-269` -> `start_lookup` -> `send_find`, `dht.rs:370-385`) with that
bound validation block. The publisher serves FIND_NODE only after a bound `verify_vb`
(`dht.rs:921`), which checks the puzzle against the source IP it observes (the public IP),
so it rejects the frame and answers nothing. No PING is sent to a seed before a FIND
answer arrives (`dht_ping` is only reached from `dht_learn`, `dht.rs:310`), so the PONG
that `maybe_rebind` needs never comes. The viewer retries FIND_NODE every 2 s
(`dht.rs:1233-1242`) and reports `bootstrap X: no answer in 5 s`.

Concrete failure: every home viewer without `--advertise-ip <public ip>` fails to join.
The runbook's "address rebound to" line (`docs/MULTI_MACHINE.md`, "Over the Internet")
can only appear when the viewer already advertises its public IP and only the port
differs. The rebind corrects a port, not an IP.

### A mobile viewer that changes IP keeps a dead identity

Code path: `maybe_rebind` returns when `bound_addr.is_some()` (`nat.rs:346`), so the
rebind is once per process. After a Wi-Fi to cellular handover every plain frame the node
sends (PING, FIND_NODE, GET_PEERS, PUNCH_REQUEST, PROBE) carries a puzzle bound to the old
IP and every peer rejects it in `verify_vb`. Peers still answer its PINGs with a PONG
(`on_ping` verifies unbound) but the reflection is ignored. The guardians keep the old
address in its registration.

QUIC saves only half of the live sessions. The viewer is the QUIC client toward its
parents and quinn migrates those (`ARCHITECTURE.md` §6). It is the QUIC server toward its
children; a server cannot migrate, so every child connection dies at the 4 s idle timeout
(`tls.rs:231`) and the children re-join through the stale registration.

### The publisher behind a NAT has no way in

Code path: `bsnode publish` has no `--bootstrap` flag and sets `bootstrap: None`
(`main.rs:659`), so `dht_start` does nothing (`dht.rs:263`). `ingress_timers` falls back
to `request_discovery` (`nat.rs:188-192`), which has no guardian to ask. No RELAY_CAPABLE
record ever arrives and no SOURCE_INGRESS proposal is sent. The CLI help says viewers
"need another bootstrap contact" (`main.rs:86-87`) but nothing provides one. Even with a
flag, the publisher's FIND_NODE hits the same bound-verification wall as the viewer
above, and `reachability` is always `Some` for a publisher (`main.rs:657`,
`driver.rs:124`), so it never rebinds. The simulator sidesteps all of this: it bootstraps
everyone from the first public viewer (`bs_sim::Sim::bootstrap_addr`).

### No relay exists for the first 30 minutes

Code path: `relay_capable` requires `reach == Public`, `upload_kbps >= 20_000` and
`uptime >= relay_capable_uptime` (`nat.rs:107-117`), 1800 s by default
(`params.rs:266`). A peer with no reachable parent asks for a bridge after two failed
rounds (`params.rs:267`), finds no RELAY_CAPABLE record, and waits. Every scenario that
needs a relay overrides the uptime to 0 or 3 s. On the Internet a viewer behind a
symmetric NAT or CGNAT gets nothing until a public 20 Mbps viewer has watched for half an
hour. A NATed publisher cannot start a stream at all in that window. The
`bsnode watch` default upload is exactly 20 000 kbps (`driver.rs:152`), so a viewer that
states a lower, honest figure is never a relay.

### A two-node swarm never decides its reachability

Code path: `on_reflection` needs PONGs from two distinct responders (`nat.rs:383`), and
`reach` defaults to CONE until then (`node.rs:235-237`). A viewer whose only contact is
the publisher stays CONE: it is never RELAY_CAPABLE, it registers as CONE, and every
later joiner sends a PUNCH_REQUEST through the referrer before probing it. Harmless on a
LAN, but the first viewers of a small stream are exactly the relays the swarm needs.

### What the driver does with the advertised address after a rebind

Nothing. `Handle::advertise_addr` (`driver.rs:467`) and the join commands `bsnode` prints
(`main.rs:365-369`) keep the pre-rebind address. The core registers the reflected address
and sends `AddressRebound`; the driver logs it (`driver.rs:1511`). `Status` carries no
address or reachability, and `ReachabilityDetected` is not logged, so an operator cannot
tell what a node decided.

### Swarm mode never rebinds

`bsnode swarm` forces `reachability: Some(... unwrap_or(Public))` (`main.rs:657`), so the
viewers of a swarm on a NATed machine neither detect their NAT nor rebind, although the
runbook presents `swarm` as the multi-machine tool.

### The simulator does not model

`bs_sim::Network::can_reach` (`network.rs:102-124`) models one thing: a packet into a CONE
or SYMMETRIC host passes only if that host sent to the sender within 30 s, plus a per-pair
coin for symmetric-to-cone. It does not model:

- Address translation. A host's address inside and outside the NAT is the same
  `SocketAddr`, so the IP-bound puzzle never mismatches, `maybe_rebind` never fires, and
  the bootstrap failure above cannot show up.
- Per-destination ports of a symmetric NAT (the record address of a symmetric peer is
  wrong on the Internet, correct in the simulator).
- Address-restricted versus port-restricted cones, hairpinning, CGNAT (one public IP for
  many viewers, which also trips the prefix caps: ISSUE-087), mapping timeouts shorter
  than 30 s or jittered, mappings refreshed by inbound traffic.
- IP change mid-session (`ARCHITECTURE.md` §6 says so), UDP blocked outright, a host
  firewall on a public viewer, IPv6, MTU.
- The publisher as a normal bootstrap: `source_behind_nat.toml` starts the first viewer
  first and seeds from it.

## Bugs

Already filed in `issues/README.md`, not repeated here:

- ISSUE-084: the requester's PROBE leaves before the target's PING opens the mapping.
- ISSUE-085: SOURCE_INGRESS has no publisher signature; the code checks the session key.
- ISSUE-087: a CGNAT shares one XDP bucket.

### 1. A NATed viewer sends a bound FIND_NODE before any PONG can reflect its address

- Severity: High
- Location: `crates/bs-core/src/node/dht.rs:263-268` (`dht_start`), `:370-385`
  (`send_find`), `:921` (`dht_serve_find` bound verify); `crates/bs-node/src/driver.rs:491`
  (`detect_ip` returns the LAN IP)
- Reproduce: `bsnode publish --advertise-ip <public> --listen 0.0.0.0:4000` behind a port
  forward; `bsnode watch --bootstrap <public>:4000 --stream ...` from any home
  connection without `--advertise-ip`. The publisher's `vb_rejected` rises; the viewer
  reports `bootstrap: no answer in 5 s`. A failing test needs a harness that delivers a
  frame with a source address different from the sender's `cfg.addr`; neither the core
  `Harness` nor `bs_sim::Network` can do that today.
- Fix sketch: in `dht_start`, PING each seed (`dht_ping`) and start the self-lookup from
  `dht_on_pong` once the reflection has been applied; or let `dht_serve_find` verify
  unbound and put the observed address in the FIND response so the first answer carries
  the reflection. Either way `maybe_rebind` runs before the first bound frame.
- Effort: small. Self-contained: yes (`dht.rs`, possibly `nat.rs`).

### 2. Rebind is once per process, so an IP change mid-session strands the node

- Severity: High
- Location: `crates/bs-core/src/node/nat.rs:344-351` (`bound_addr.is_some()` early
  return); `crates/bs-node/src/tls.rs:231` (4 s idle kills the server-side sessions)
- Reproduce: start a viewer on Wi-Fi, switch to cellular. Peers log `validation block
  from ... rejected` for every plain frame; the node's children lose it after 4 s; the
  guardians keep the stale registration until it expires.
- Fix sketch: accept a new reflection from a responder whose PONG verifies when two
  responders agree on the new IP; re-solve, re-register, emit `AddressRebound`; close and
  reopen sessions whose peer verifies us by address. The driver needs an input for
  "local address changed" (interface watcher) to force the pair of PINGs promptly.
- Effort: medium. Self-contained: no (rebind policy versus spoofed reflections, and the
  children's reconnect path are design decisions).

### 3. `bsnode publish` behind a NAT has no bootstrap and cannot bind an ingress relay

- Severity: High
- Location: `crates/bs-node/src/main.rs:36-90` (no `--bootstrap` on `Publish`), `:659`
  (`bootstrap: None`), `:657` (reachability always `Some`); `crates/bs-core/src/node/nat.rs:181-193`
- Reproduce: `bsnode publish --nat cone ...`; `ingress_count()` stays 0 forever,
  `IngressBound` never fires, and the printed join command names an unreachable address.
- Fix sketch: add `--bootstrap` to `Publish` and pass it into `RunConfig.bootstrap`; let
  the publisher take `reachability: None` so it reflects and rebinds like a viewer. This
  depends on bug 1 and on a public seed to point at (see Missing features).
- Effort: small for the flag. Self-contained: no (needs a seed node and a decision on
  who the first guardian is).

### 4. The 30 minute uptime criterion leaves a young swarm with no relay at all

- Severity: High
- Location: `crates/bs-core/src/node/nat.rs:107-117`; `crates/bs-core/src/params.rs:265-266`
- Reproduce: run `scenarios/source_behind_nat.toml` with the `[params]` override
  removed: `ingress_bound` stays 0 and the viewers never attach. Same for
  `bs-core/tests/node.rs:921` without `relay_capable_uptime = 0`.
- Fix sketch: the spec's criterion (Ch6 §6.3.1) is a Sybil guard, not a readiness
  signal. Options: count uptime across streams in a persisted identity; let the publisher
  name trusted ingress relays (its own seed) that skip the criterion; shorten the window
  for ingress relays only. A spec change in any case.
- Effort: medium. Self-contained: no.

### 5. Reachability stays undecided with one responder

- Severity: Medium
- Location: `crates/bs-core/src/node/nat.rs:382-385`; `crates/bs-core/src/node.rs:235-237`
- Reproduce: publisher plus one `watch --nat auto` viewer; `reachability()` on the
  viewer is CONE after any run time, `ReachabilityDetected` never fires.
- Fix sketch: after the first reflection, PING the first FIND_NODE responder that is not
  the bootstrap; after a timeout with one reflection equal to the own address, decide
  PUBLIC provisionally and re-evaluate on the next distinct responder.
- Effort: small. Self-contained: yes.

### 6. Swarm viewers never detect or rebind

- Severity: Medium
- Location: `crates/bs-node/src/main.rs:657`
- Reproduce: `bsnode swarm --nat auto` behind a NAT; no `address rebound to` line, every
  viewer stuck in `Connecting`, publisher `vb_rejected` rising.
- Fix sketch: pass `reachability(&nat)` (which is `None` for `auto`) as `watch` does.
- Effort: small. Self-contained: yes.

### 7. The driver keeps the pre-rebind address in the handle, status and printed commands

- Severity: Low
- Location: `crates/bs-node/src/driver.rs:467`, `:1511`; `crates/bs-node/src/main.rs:365-369`
- Reproduce: any rebind; `Handle::advertise_addr` and the `join from another machine`
  lines still show the detected LAN address.
- Fix sketch: on `AddressRebound`, update a watch channel the handle exposes and add
  `reachability` and `advertised` to `Status`; log `ReachabilityDetected`.
- Effort: small. Self-contained: yes.

### 8. A punch forwarded by a second-hand referrer is dropped silently

- Severity: Low
- Location: `crates/bs-core/src/node/nat.rs:440` (referrer lookup),
  `crates/bs-core/src/node/membership.rs:536`, `:621` (SHUFFLE and GOSSIP_EXCHANGE
  records set the referrer)
- Reproduce: in `nat_cone.toml`, raise the shuffle rate so most CONE records arrive via
  a neighbour that learned them third-hand; the forward goes to a target with no mapping
  toward that neighbour, and `punches_sent` rises with no attach. Only the guardians
  are kept open by tau_nat (`nat.rs:582`).
- Fix sketch: fall back to a guardian as referrer when the referrer is not a guardian
  and has no session with the target; or only note referrers from GET_PEERS.
- Effort: small. Self-contained: yes.

## Missing features

| Feature | Why it is needed | Size |
|---|---|---|
| A NAT test rig: a `bs_sim::Network` that translates addresses (inside and outside `SocketAddr`, per-destination ports for symmetric, mapping timeout per host), or a container fleet with iptables NATs (`fleet/` has none) | Nothing in `nat.rs` that depends on an address mismatch has a test; bugs 1, 2, 5 cannot get a failing test without it | 4 days (sim) or 3 days (docker fleet with `MASQUERADE` and conntrack timeouts) |
| A public seed: a tiny always-on bsnode (relay-class, no stream) that is the default `--bootstrap` and the first guardian | A NATed publisher needs someone to register with (bug 3); viewers need an address that is not the broadcaster's | 2 days (mode in `bsnode`, docs, a deploy script) |
| Re-rebind on IP change plus a driver-side interface watcher | Mobile viewers (bug 2) | 3 days |
| Publisher-named ingress relays that skip the uptime criterion, with the spec change | A home broadcaster with no port forward must be able to stream from minute 0 (bug 4) | 3 days including the spec text and `source_behind_nat.toml` without the override |
| Keepalive robustness: retransmit the PUNCH forward and the punch PING once, and keep the QUIC-session mapping toward non-guardian referrers | One lost plain frame today closes the only path through a cone NAT; tau_nat 25 s against a 30 s mapping has no margin for loss | 2 days |
| Operator visibility: reachability, reflected address and punch/bridge counters in `Status`, the player page and the fleet dashboard | Diagnosing a NAT failure today needs the publisher's `vb_rejected` log line | 1 day |

## Open questions

- tau_nat is 25 s against the spec's 30 s mapping TTL. Real home routers vary from 20 s
  to 180 s for UDP; one lost PING means a 50 s gap. Does the keepalive need a measured
  timeout per guardian, or a shorter fixed period?
- `maybe_rebind` solves the dynamic puzzle on the driver's event loop
  (`identity.solve_dynamic`, `nat.rs:362`). At `c2_min = 8` plus the swarm-size term this
  is milliseconds; at a large swarm it is longer. Does the driver need to move the solve
  off the loop before a re-rebind (bug 2) makes it recurring?
- A symmetric viewer connects to us from a port its record does not name. The driver
  keys the session by `conn.remote_address()` (`driver.rs:702`) and the core keys records
  by the record address; `resolve_addr_full` prefers the session (`nat.rs:544`) so punch
  forwards reach it, but children and receipts are keyed by the session address. Is one
  node under two addresses in `passive` and `sessions` ever reconciled?
- How many responders must agree before a rebind? The bootstrap is operator-chosen, so
  one is enough for the first rebind. For a later one (bug 2) a lying guardian redirects
  the registration unless two responders agree.
- Who is the first guardian for a stream whose publisher is behind a NAT and whose
  viewers are all behind NATs? The spec needs at least one PUBLIC relay-class node in
  every swarm; the product needs to say where it comes from.
