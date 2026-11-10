# Roadmap to a working system on the public Internet

What stands between the current prototype and a product: a broadcaster on a home
connection, viewers on home and mobile connections behind NATs, no servers, video and
audio in a browser or the desktop app. Written 2026-11-09 from six subsystem
investigations in [`audits/internet-readiness/`](audits/internet-readiness/), the
tick-walk audit of 2026-11-02 and the open tickets in [`issues/`](issues/).

## Where the system stands

The protocol core is complete in simulation and runs on real sockets on a LAN: fifteen
simulator gates pass, a twenty-container fleet plays a file byte-exact under 2 % loss,
ten viewers join a LAN publisher in two seconds, and the desktop app streams video with
audio to a browser page. What has never run is the Internet case. The investigations
found that a home viewer failed before any traversal code executed
([round 1](#round-1-2026-11-09) fixed that in the core). Once it connects, every
timer, default and defence assumes a LAN.

| Area | Works today | Breaks on the Internet |
|---|---|---|
| Identity and bootstrap | One publisher that never restarts, viewers who know its address | Every default publisher shares one key and stream id; a restart renumbers segments and viewers reject the stream forever; one home IP is the only contact |
| NAT traversal | All mechanisms exist in `bs-core` and are wired through the driver | The rebind is once per process and has never run through a real NAT; a NATed publisher has nobody to register with; no relay exists for 30 minutes |
| Transport and security | Mutual TLS over QUIC bound to the node identity, BBR, one demuxed socket | No per-source rate limit on plain frames, so a 20 Mbps spoofed PING flood takes any node down; no replay window; DHT answers reflect 5 to 6× toward a spoofed address |
| Forest and repair | Join, shed, PULL zone, parity plumbing; PSR 0.01 % on a LAN at 3 % loss | Probe, NEIGHBOR and eviction deadlines are LAN constants; candidates over 200 ms RTT are unusable; the receipt loss byte is a constant, so parity never follows loss; repair has no Deputy fast path |
| Media and players | File ingest with two renditions and AAC, signed descriptor, MSE page, desktop shell | No live source (camera, screen, RTMP); rendition switches are not keyframe aligned; a phone has nothing to connect to |
| Operations | Clean start and stop, three surfaced errors | A dead publisher is never reported; a crashed encoder reads as end of stream; nothing persists; the player API streams to anyone who can reach the port |

The six notes list 55 bugs with file and line, and the missing pieces with a size in
days. The counts below come from them.

## The six blocks of work

Sizes are engineer-days for one person who knows the code. Blocks 1 to 3 are needed
before the first honest Internet test; 4 to 6 make it a product.

### 1. Connect at all: bootstrap, identity, timers (about 18 days)

The first Internet test cannot start until these land.

- Re-rebind on IP change for mobile viewers, with a driver-side interface watcher. A test of the PING-first bootstrap through a real NAT. 3 days.
- Random identity by default and a persisted keypair (a data directory for `bsnode`, the keychain for the desktop app), so a broadcaster keeps one stream id across restarts and the secret key of the default publisher stops being public. The cost of the spec proof of work measured on a phone. 3 days.
- Publisher restart: persisted segment and manifest counters or a monotonic stream clock, viewer re-anchor. Needs a spec decision first (the cold-start walk, finding F5). 4 days.
- Several bootstrap contacts in the config, the CLI and the share link, tried in parallel with backoff. 2 days.
- RTT-scaled timers fed from the QUIC estimator: probe, NEIGHBOR, eviction and the bridge trigger from one `rtt_budget(peer)` helper instead of 200 ms, 600 ms and a once-seeded SRTT. 2 days.
- Loss measurement the parent can act on: the child counts symbols past decode and knows the parity it was sent, so the receipt loss byte drives parity (ISSUE-078 sizes it). 3 days.

### 2. Survive the open Internet: transport defences (about 14 days)

Without these the first public stream is a denial-of-service target.

- User-space rate limiting in the driver: the spec's two-tier token buckets (per source and global) for plain frames and QUIC handshakes, with an allowlist from open sessions; a bounded plain-datagram queue; QUIC Retry under load. 4 days.
- Freshness and replay for pre-session frames: a UTC timestamp in the validation block with a ±60 s window and a per-NodeID LRU. Today the timestamp is process uptime. Spec change. 2 days.
- Reflection defence for DHT RPCs: a cookie round or a response cap on first contact, so the home publisher does not amplify toward spoofed addresses. Spec change. 2 days.
- Bans delivered to the transport, with prefix-keyed escalation; a session's verified NodeID compared with the record dialed; a punch forward that a third party cannot aim. 3 days.
- A hostile-network harness for `bs-node`: spoofed flood, replay, Retry, simultaneous open, uplink throttle. Nothing exercises `tls.rs`, `transport.rs` or the driver against an attacker today. 3 days.

### 3. Repair at Internet RTTs (about 14 days)

With timers scaled, repair still costs 1.8 s median at 250 ms RTT against a 2.0 s PULL
window, because it is a full probe round plus a 1.5 RTT handshake.

- ROSTER and Deputy election (Ch1 §1.2.3): the subtree re-attaches as a unit instead of a thundering herd, with the timer scaled per ISSUE-074. 6 days.
- QUIC resumption tickets kept per NodeID so re-attach is 1 RTT (the spec's repair timeline assumes it; `rustls` keys tickets by server name today). 2 days.
- An Internet network model in `bs-sim`: RTT above 160 ms, jitter, loss at parity 1, mid-run RTT steps and address translation, with gates. None of the failures above is visible to the simulator today. 4 days.
- The simulator's PSR made to match Ch8 §8.2, and the same definition in `fleet/report.py`. 1 day.
- Keepalive and punch robustness: retransmit the punch forward and the punch PING once, keep mappings toward non-guardian referrers. 1 day.

### 4. A broadcaster who is not a developer (about 20 days)

- Live ingest: camera, microphone, screen, and an RTMP or SRT listener so OBS can feed the publisher. One decode feeding every rendition. 7 days.
- A publisher behind a NAT without a port forward: publisher-named ingress relays that skip the 30 minute uptime criterion (spec change to Ch6 §6.3.1), or UPnP and NAT-PMP first. 4 days.
- A public seed: an always-on relay-class `bsnode` with no stream as the default bootstrap and first guardian. This is the one standing piece of infrastructure the no-servers goal has to decide on; without it a NATed publisher has nobody to register with. 2 days of code, one decision.
- Reachability self-check and uplink probe at publish time, the public address in the share link, a health view of the encoder and the swarm on the publisher page. 5 days.
- Persistence and logs: a state directory, settings, a log file with rotation, periodic result snapshots. 2 days.

### 5. A viewer who is not a developer (about 30 days)

- A usable player: keyframe-aligned rendition switching (1 fragment in 4 starts with a sync sample today, so every switch damages the picture), hole jumping, error recovery, buffer trim, controls, a latency readout. 3 days.
- Per-rendition subscription so a viewer downloads the rendition it watches, not every layer and two audio tracks; a 240p or audio-only floor in the default ladder. 5 days.
- A viewer health model: connecting, playing, degraded, stalled, ended, failed, with a reason, rendered by the page and the desktop app. The publisher dying is invisible today. 3 days.
- Player API access control: loopback by default, an Origin check, a token for any other bind. 1 day.
- Mobile viewer: `bs-node` built for iOS and Android inside a thin shell, or `bs-core` in WebAssembly as a WebTransport leaf behind public relays. A phone browser cannot view at all today, because the page is a loopback client of a local node. This is the largest single item and the one most users will judge the product by. 20 days.

### 6. Close the specification (about 15 days)

Fourteen tickets from the tick-walk audit are open (ISSUE-073 to 085, 087). Five
change behaviour a real audience will hit: the startup target (082), FEC sizing (078),
the per-hop latency budget (077), the M = 2 wire encoding (073, interoperability only)
and the XDP allowlist key (087). Fifty-nine Medium and Low candidates from the same
audit are unverified. Each ticket is a design decision plus a spec edit plus a core
change, through [`FIXER-AGENT.md`](FIXER-AGENT.md).

## Order and schedule

| Weeks | Deliverable | Gate |
|---|---|---|
| 1 to 3 | Block 1, and the rate limiter and bounded queue from block 2 | A viewer behind a home NAT joins a port-forwarded publisher on another network and plays for an hour; a publisher restart does not strand viewers |
| 4 to 6 | Rest of block 2, block 3 | The Internet gates in `bs-sim` pass; the two-site test holds PSR below 0.1 % at 80 ms RTT and 2 % loss, and repairs a parent loss inside the PULL window at 250 ms |
| 7 to 10 | Block 4, the player half of block 5 | A broadcaster streams a camera from behind a NAT to ten viewers on three networks without editing a command line; rendition switches are clean |
| 11 to 16 | Mobile viewer, block 6 | A phone on cellular watches the stream; the open tickets are closed or accepted with a recorded reason |

About 110 engineer-days in all. One person reaches the first gate in a month and the
last in about six; three people reach the last in two to three months, because blocks
2, 3 and 5 are independent of each other.

## How bugs get fixed along the way

The investigations also found bugs that need no design decision. Agents fix those in
rounds: each round takes the open self-contained High and Medium bugs from the notes,
one fixer at a time on the main tree, each required to add a failing test, run its
crate's tests and clippy, and keep the simulator baseline green; the round ends with
the full check script and a push. The first round is recorded below. The remaining
list is in each area's notes under *Bugs*, marked `self_contained: yes`.

## Round 1 (2026-11-09)

Ten fixers were queued. Five landed a fix with a test, one found the report wrong,
and four did not run because the agent usage limit was reached.

| Bug | Result |
|---|---|
| A NATed viewer sent a bound FIND_NODE before any PONG | Fixed. A viewer that detects its reachability PINGs the seeds, rebinds to the reflected address, then starts the self-lookup. A node that states its reachability skips the PING. Test `nated_viewer_pings_seed_before_first_find_node` |
| A viewer whose contacts all died never asked the seed again | Fixed. A contact that answers no FIND for 5 s leaves the table, and an empty table re-arms the bootstrap with a 2 s to 30 s backoff. Test `viewer_rebootstraps_when_every_contact_dies` |
| Guardians served a dead run's Stream Record forever | Fixed. The record expires `tau_ttl` after its last write. Test `guardian_expires_stale_stream_record` |
| Reachability was never decided with one PONG responder | Fixed. One reflection equal to the node's own address decides PUBLIC after `reflection_timeout` (1 s). Test `lone_responder_reflecting_own_address_decides_public` |
| The default proof of work was the simulation difficulty | Fixed. `bsnode` and `RunConfig` default to the spec difficulty. `--spec-pow` is gone and `--sim-pow` selects the cheap puzzle. Tests in `bs-node/tests/defaults.rs` |
| `bsnode swarm` forces a known reachability | Not a bug. The cited line is the `publish` branch |
| Rendition switch not keyframe aligned, player does not cross a buffer hole, media error is fatal, player API has no Origin check | Not run |

The first version of the PING-first fix failed three simulator gates and slowed
repairs under churn: a lost or late PONG left a node without its self-lookup. The
seed PONG is now accepted however late it arrives, and the PING repeats until a seed
answers.

Open points from the round:

- The 5 s one-strike eviction and the 1 s `reflection_timeout` are choices the spec does not state.
- A NATed join still sends one discovery FIND_VALUE bound to the old address. The seed rejects it and the 1 s retry succeeds.
- The test `detecting_viewer_repeats_the_seed_ping_after_a_lost_pong` passes without the PING retry, so it does not yet prove the retry.
- The PING-first path has not run through a real NAT, and no simulator scenario uses detected reachability.
