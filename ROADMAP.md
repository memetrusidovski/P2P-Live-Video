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
| Identity and bootstrap | One publisher that never restarts, viewers who know its address | A restart renumbers segments and viewers reject the stream forever; the share link names one contact |
| NAT traversal | All mechanisms exist in `bs-core` and are wired through the driver | The rebind is once per process and has never run through a real NAT; a NATed publisher has nobody to register with; no relay exists for 30 minutes |
| Transport and security | Mutual TLS over QUIC bound to the node identity, BBR, one demuxed socket | No rate limit on QUIC handshakes; no replay window; DHT answers reflect 5 to 6× toward a spoofed address |
| Forest and repair | Join, shed, PULL zone, parity plumbing; PSR 0.01 % on a LAN at 3 % loss | The eviction deadline is seeded once from the probe and never follows the path; repair has no Deputy fast path |
| Media and players | File ingest with two renditions and AAC, signed descriptor, MSE page, desktop shell | No live source (camera, screen, RTMP); a phone has nothing to connect to |
| Operations | Clean start and stop; a dead publisher, a crashed encoder and a silent bootstrap are reported | Nothing persists but the publisher key; tables keyed by peer grow for the life of the process; the player API streams to anyone on the LAN who can reach the port |

The six notes list 55 bugs with file and line, and the missing pieces with a size in
days. The counts below come from them.

## The six blocks of work

Sizes are engineer-days for one person who knows the code. Blocks 1 to 3 are needed
before the first honest Internet test; 4 to 6 make it a product.

### 1. Connect at all: bootstrap, identity, timers (about 11 days)

The first Internet test cannot start until these land.

- Re-rebind on IP change for mobile viewers, with a driver-side interface watcher. A test of the PING-first bootstrap through a real NAT. 3 days.
- The cost of the spec proof of work measured on a phone, and the publisher key in the OS keychain for the desktop app (it is a file in the home directory since round 3). 1 day.
- Publisher restart: persisted segment and manifest counters or a monotonic stream clock, viewer re-anchor. Needs a spec decision first (the cold-start walk, finding F5). 4 days.
- Several bootstrap contacts in the share link and the desktop app. The CLI and the driver take them since round 8. 1 day.
- The bridge trigger scaled to the round trip, and a floor for the eviction timer of one chunk period plus jitter (a spec question, tick walk S4). 1 day.

### 2. Survive the open Internet: transport defences (about 10 days)

Without these the first public stream is a denial-of-service target.

- Per-source budgets for QUIC handshakes, and a cache of verified static puzzles so a repeated PING costs no proof-of-work check. Round 6 capped handshakes in progress and added QUIC Retry under load. 1 day.
- Freshness and replay for pre-session frames: a UTC timestamp in the validation block with a ±60 s window and a per-NodeID LRU. Today the timestamp is process uptime. Spec change. 2 days.
- Reflection defence for DHT RPCs: a cookie round or a response cap on first contact, so the home publisher does not amplify toward spoofed addresses. Spec change. 2 days.
- Bans delivered to the transport, with prefix-keyed escalation; a punch forward that a third party cannot aim. Round 6 added the check of a session's NodeID against the record dialed. 2 days.
- A hostile-network harness for `bs-node`: spoofed flood, replay, Retry, simultaneous open, uplink throttle. Nothing exercises `tls.rs`, `transport.rs` or the driver against an attacker today. 3 days.

### 3. Repair at Internet RTTs (about 11 days)

With timers scaled, repair still costs 1.8 s median at 250 ms RTT against a 2.0 s PULL
window, because it is a full probe round plus a 1.5 RTT handshake.

- ROSTER and Deputy election (Ch1 §1.2.3): the subtree re-attaches as a unit instead of a thundering herd, with the timer scaled per ISSUE-074. 6 days.
- QUIC resumption tickets kept per NodeID so re-attach is 1 RTT (the spec's repair timeline assumes it; `rustls` keys tickets by server name today). 2 days.
- A mobile profile in the simulator's Internet model: an address change mid-run and jitter bursts. Round 2 added the rest. 1 day.
- The simulator's PSR made to match Ch8 §8.2, and the same definition in `fleet/report.py`. 1 day.
- Keepalive and punch robustness: retransmit the punch forward and the punch PING once, keep mappings toward non-guardian referrers. 1 day.

### 4. A broadcaster who is not a developer (about 20 days)

- Live ingest: camera, microphone, screen, and an RTMP or SRT listener so OBS can feed the publisher. One decode feeding every rendition. 7 days.
- A publisher behind a NAT without a port forward: publisher-named ingress relays that skip the 30 minute uptime criterion (spec change to Ch6 §6.3.1), or UPnP and NAT-PMP first. 4 days.
- A public seed: an always-on relay-class `bsnode` with no stream as the default bootstrap and first guardian. This is the one standing piece of infrastructure the no-servers goal has to decide on; without it a NATed publisher has nobody to register with. 2 days of code, one decision.
- Reachability self-check and uplink probe at publish time, the public address in the share link, a health view of the encoder and the swarm on the publisher page. 5 days.
- Persistence and logs: a state directory, settings, a log file with rotation, periodic result snapshots. 2 days.

### 5. A viewer who is not a developer (about 27 days)

- Player controls and a latency readout. Round 4 did switching, hole jumping, error recovery and buffer trim. 1 day.
- Per-rendition subscription so a viewer downloads the rendition it watches, not every layer and two audio tracks; a 240p or audio-only floor in the default ladder. 5 days.
- A viewer health model: connecting, playing, degraded, stalled, ended, failed, with a reason, rendered by the page and the desktop app. Since round 7 a dead publisher sets an error in the status. 2 days.
- A token for a player API bound to a non-loopback address: anyone on the LAN can still open the page (the Origin check of round 4 stops other web sites only). 1 day.
- Mobile viewer: `bs-node` built for iOS and Android inside a thin shell, or `bs-core` in WebAssembly as a WebTransport leaf behind public relays. A phone browser cannot view at all today, because the page is a loopback client of a local node. This is the largest single item and the one most users will judge the product by. 20 days.

### 6. Close the specification (about 15 days)

Fourteen tickets are open (ISSUE-073 to 085 without 078, 087 and 089). Five change
behaviour a real audience will hit: relay balance across trees (089), the startup target (082),
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

About 90 engineer-days in all. One person reaches the first gate in a month and the
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

## Round 2 (2026-11-11)

| Work | Result |
|---|---|
| Live LAN swarm on the spec proof of work | Ten of ten viewers play, join median 2.0 s |
| Internet model in the simulator | Viewers can detect their reachability on a network that translates addresses, including one source port per peer behind a symmetric NAT. A scripted event changes every path's latency mid-run. Five `internet_*` gates run in the check script |
| Probe and NEIGHBOR deadlines | A probe round still ends at `probe_timeout` when a usable parent answered. With none, it waits up to 1.5 times the lower quartile of the recent round trips. The NEIGHBOR deadline covers 3.5 round trips. With every pair 200 to 300 ms apart, active viewers went from 16 % to 100 % |
| `bsnode publish --repeat` | The flag was ignored and the stream ended with the file. It now loops |
| Loss report and parity (ISSUE-078, closed) | The child's loss report was a constant set by the default parity. It now measures loss on the source symbols. Parity is the smallest count that leaves one block in a thousand per hop to PULL. At 2 % loss starvation fell from 0.48 % to 0.09 %, at 5 % from 1.5 % to 0.3 % |

Findings from the round:

- The repair KPIs of a gate rest on about ten repairs, so they move with the seed. `baseline_100` passed two of six seeds before the round and passes two of three after. Judge a timer change over several seeds, never on one.
- A 60-viewer swarm starves one tree when few relays hash to it. Seeds 25 and 27 put 9 of 45 relays in tree 1 and a third of the joins took over 15 s at any latency. This is a forest-balance problem, not a latency one. It is filed as ISSUE-089.
- A deadline built on the median or a high percentile of the round trips made short-path swarms worse. Slow answers there come from overloaded peers, and the fixed 200 ms cut filters those peers out.
- `internet_rtt_step` passed before any fix: a route change alone does not evict parents in the simulator.
- Real loss with the old parity rule was worse than the constant report: 2.4 % starvation at 2 % loss. The measurement and the rule had to change together.
- Above 5 % loss the 30 % parity ceiling leaves 1.4 % of blocks per hop to PULL at 10 % loss. Cellular links need a larger block or a higher ceiling (ISSUE-077).
- The repair p95 limits of `baseline_100` and `internet_nat_detect` were met by seed luck. They are now 6 s and 8 s, from six seeds each way. `scripts/sim_seeds.sh` runs a gate over several seeds.

## Round 3 (2026-11-11)

| Work | Result |
|---|---|
| Parent round-trip estimate | The driver hands the core the QUIC round trip of every open connection once a second. The eviction deadline now follows a path that changed after the join. Test `transport_rtt_samples_move_the_parent_estimator` |
| Identity | A publisher keeps a random key in `~/.bitstream/publisher.key` and so keeps its stream id across restarts. A viewer gets a new random key at every start. `--seed` still derives a fixed key for tests and fleets. A viewer must now pass `--stream` or `--publisher-seed` |

## Round 4 (2026-11-11)

The four player fixes that round 1 did not run, tested in Chrome against an ffmpeg
publisher with two renditions.

| Bug | Result |
|---|---|
| Rendition switch is not keyframe aligned | The cause was in the ingest: `-min_frag_duration` stopped ffmpeg from cutting at keyframes, so every keyframe sat in the middle of a fragment. Without the flag a keyframe starts every fourth fragment. The page reads the fragment's sample flags and switches there. Test `every_fourth_fragment_starts_with_a_keyframe` runs the real ingest |
| The page does not cross a buffer hole | The page seeks to the next buffered range when playback reaches the end of one |
| A media error is fatal | The page drops the MediaSource and rebuilds it at the next keyframe chunk. A node restart under an open page plays again without a reload |
| Player API has no Origin check | A request whose `Origin` differs from its `Host` gets 403, and so does a `Host` that is a public DNS name (DNS rebinding). The allow-all CORS header is gone |

The page also trims media more than 30 s behind the playhead.

## Round 5 (2026-11-11)

| Work | Result |
|---|---|
| Rate limit on plain frames | The socket applies the spec's two-tier token buckets (App B §B.1.4) before a datagram reaches the driver: 2000 packets per second for a source with an open session, 50 for any other source, 5000 for all other sources together. A flood of 300,000 junk datagrams in 3.4 s had 299,729 refused, and the publisher kept running |
| Plain-datagram queue | Bounded at 4096. The result file counts refused and dropped datagrams |

A ten-viewer swarm from one LAN address saw no datagram refused and joined in 1.9 s.
Loopback sources are exempt, so a one-host test swarm is never limited.

## Round 6 (2026-11-11)

Four transport bugs from the investigation, all in `bs-node` or the core's join path.

| Bug | Result |
|---|---|
| A simultaneous open overwrites the live session | Two peers that dial each other keep the connection the lower NodeID dialed. A closed duplicate no longer ends the session |
| A 4 s idle timeout drops every session on a network change | Idle timeout 15 s, keep-alive 3 s. Liveness stays with the core's eviction timer |
| QUIC accept has no cap and no address validation | At most 256 handshakes in progress. Above 32 a new one must pass a QUIC Retry |
| A session's NodeID is never compared with the record dialed | A candidate whose session proved another NodeID is not asked to be a parent. Test `session_with_another_identity_is_not_asked_to_be_a_parent` |

The simultaneous-open rule and the Retry path have no test: nothing in the repository
drives two real nodes into either case yet.

## Round 7 (2026-11-11)

| Bug | Result |
|---|---|
| A viewer with a dead publisher reports no error | A viewer that played and then gets no media for 10 s sets `no media for over 10 s: the publisher is gone or unreachable`. The next chunk clears it. Checked live with a killed publisher |
| ffmpeg failure is reported as a normal end of stream | The ingest reads each ffmpeg exit status. A failure sets `ingest: ffmpeg for <rendition> ended with <status>`, the result file carries it, and `bsnode publish` exits with code 4 |
| The player HTTP server is never shut down | The server stops when its node does |
| Linux relays get a 20th of the intended socket buffer | The node warns once with the size it got and the sysctl to raise |

The runbook's symptom table has a row for each.

## Round 8 (2026-11-11)

| Work | Result |
|---|---|
| Several bootstrap contacts | `--bootstrap` takes a list. The node asks every contact, and a viewer given one dead and one live contact joins. The share link of the desktop app still names one |
| Relay balance across trees | Filed as ISSUE-089 with measurements over eight seeds |

## Round 9 (2026-11-12)

| Bug | Result |
|---|---|
| The 8 MB datagram send buffer serves stale symbols | 512 KiB per connection: about half a second of the whole stream to one child. A twelve-viewer LAN swarm starves the same share of chunks as before |
| Stop does not flush DISCONNECT frames | A node that stops, or reaches `--duration-s`, tells its peers and runs 400 ms more. A closed session's connection stays open 250 ms so queued frames are written. With a publisher stopped under eight viewers, all 24 parent links ended with `disconnect:Quit` |

