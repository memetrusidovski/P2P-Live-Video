# Operations and clients: internet readiness

Area: running `bsnode` and the desktop app as a non-expert, for days. Judged against
a broadcaster on a home connection and viewers behind NATs, with no servers.
Code under `src/crates/bs-node`, `src/crates/bs-player-api`, `src/clients`, `src/fleet`.
Audited 2026-11-09.

## What works today

- `bsnode publish|watch|swarm|hash|stream-id` (`src/crates/bs-node/src/main.rs`). One
  binary, clap help, no config file. `publish` prints the `watch` and `swarm` lines a
  second machine runs.
- Graceful stop on Ctrl-C and SIGTERM (`main.rs:441`): every node gets `Command::Quit`,
  writes `result.json` when `--result` is set, and `main` exits. ffmpeg is spawned with
  `kill_on_drop(true)` (`src/crates/bs-ingest/src/ffmpeg.rs:208`), so it dies with the
  process.
- Live status: `Handle.status` is a `watch` channel updated on every lifecycle, tree and
  chunk event (`src/crates/bs-node/src/driver.rs:819`). The player API serves it as
  `/api/status`, `/api/descriptor`, the page at `/` and media over `/ws`
  (`src/crates/bs-player-api/src/lib.rs:186`).
- `last_error` covers three cases (`driver.rs:1055`, `:1108`, `:1142`): bootstrap silent
  for 5 seconds, bootstrap rejected the QUIC session, ffmpeg did not start. The silence
  error clears on the first frame from the bootstrap.
- Logs: one `info!` per state change, parent attach, child admit, repair; one `warn!` per
  starved chunk, parent loss, rejected block; one line per peer for a rejected validation
  block (`driver.rs:1518`). `bs-core` logs nothing. The default filter is
  `info,quinn=warn`. Volume is low on a healthy node.
- Memory in the driver is bounded: 96 checkpoints, a 64-chunk broadcast ring per player,
  per-connection writer maps dropped on `Ev::Closed`. The viewer's output file is opt-in.
- Socket buffers are requested at 8 MiB (`src/crates/bs-node/src/transport.rs:27`);
  plain-frame sends that meet a full buffer are held in an 8192-entry backlog instead of
  dropped. QUIC has a keep-alive every 1.5 seconds and an idle timeout
  (`src/crates/bs-node/src/tls.rs:231`), which keeps NAT mappings open.
- Portability: the only `cfg(unix)` is the SIGTERM handler; Windows falls back to
  Ctrl-C. No libc or nix dependency. `bs-ingest` shells out to `ffmpeg` and `ffprobe` by
  name.
- Desktop app (`src/clients/desktop`): one node per process, start and stop commands,
  a one-second status poll, the player page in an iframe on loopback, a `bs://` share
  link. Error strings from the node reach the error line. `stop` waits 10 seconds then
  aborts the task (`src-tauri/src/node.rs:189`). The end-to-end tests cover a raw-bytes
  and a video publisher feeding a viewer over localhost.
- Docker fleet (`src/fleet`): `up.sh`, `down.sh`, `churn.sh`, `report.py` and
  `dashboard.py` work from a clean checkout once `--file` points at an mp4.
  `entrypoint.sh` retries DNS and seeds each viewer from its hostname, so container
  identities differ.

## What breaks on the Internet

### A viewer never learns the stream died, only that it ended

`on_stream_end` (`src/crates/bs-core/src/node.rs:2250`) is the only path to
`Terminated` for a viewer. When the publisher crashes, loses power or the broadcaster's
NAT mapping expires, no STREAM_END is sent. The viewer stays in `Active`, logs
`parent lost` per tree and `starved:` four times a second, and `last_error` stays `None`.
The player page shows `state Active`, a frozen video and a rising starved counter. The
desktop app shows the same. The runbook's symptom table has no row for it.

### A crashed ffmpeg reports as a clean end of stream

`FfmpegSource::next_chunk` returns `None` both at end of file and when the ffmpeg process
dies (`ffmpeg.rs:276`: an `Ended` event or a closed channel sets every rendition ended).
The driver treats `None` as "ingest ended; sending STREAM_END" (`driver.rs:765`). A
broadcaster whose encoder crashed sees `Terminated` with no error, and every viewer gets
a signed STREAM_END and stops. The exit status of ffmpeg is never read.

### The player API is an open, unauthenticated media relay once it leaves loopback

`bs_player_api::serve` adds `Access-Control-Allow-Origin: *` to every response and binds
whatever address it is given (`lib.rs:194`, `:208`). The comment says the server binds
loopback; the runbook (`src/docs/MULTI_MACHINE.md`), the printed join hint
(`main.rs:361`) and `fleet/entrypoint.sh` all pass `0.0.0.0`. On a home router with the
player port forwarded, or on any shared Wi-Fi, anyone can read the node id, depth, the
running hash and checkpoints, and pull the full video from `/ws`. There is no token,
no origin check, no rate limit. The desktop app is safe: it binds `127.0.0.1:0`.

### Nothing persists between runs

There is no config file, no state directory and no key store. Identity is derived from
`seed` and the bound port (`src/crates/bs-core/src/node.rs:178`). The CLI defaults are
seed 1 for every publisher and seed 2 for every viewer. Consequences:

- Every `bsnode publish` on port 4000 with the default seed has the same NodeID and the
  same stream id, on every machine in the world. Two broadcasters cannot coexist, and a
  stream id is not a secret: anyone can run the same publisher identity.
- Two viewers with seed 2 whose OS picked the same ephemeral port collide on NodeID.
  With 64k ports, a swarm of 1,000 CLI viewers expects about 8 collisions.
- A restart is a new identity: rank, receipts, the DHT registration and the address
  binding start from zero, and with `--spec-pow` the proof of work is redone.
- The desktop app seeds from the clock (`node.rs:343`), so it avoids the collision but
  has the same restart cost.

The broadcaster also has to type the public address and port-forward by hand: `detect_ip`
(`driver.rs:491`) returns the LAN address behind a NAT and the share link carries it.

### Memory grows with every peer ever seen

These `bs-core` tables are keyed by peer and have no removal path. Each entry is small,
but a relay that stays up for days in a swarm with churn accumulates one entry per peer
that ever touched it:

| Table | File | Prune sites |
|---|---|---|
| `bad_peers`, `observed_prefixes`, `descriptor_sent` | `src/crates/bs-core/src/node.rs:118-139` | none |
| `incentive.known_keys`, `wanted`, `link_loss`, `pull_ingress` | `src/crates/bs-core/src/node/incentive.rs:80-104` | none for the outer map |
| `audit.seen`, `degree`, `verified`, `accused`, `rate`, `parked` | `src/crates/bs-core/src/node/audit.rs:27-39` | none |
| `nat.reflections`, `referrers`, `punched`, `tried_relays`, `ingress_tried` | `src/crates/bs-core/src/node/nat.rs:48-74` | none |
| `pull.bitfields` outer map | `src/crates/bs-core/src/node/pull.rs:25` | inner map pruned at `:170`, outer never |
| `rejected_peers` | `src/crates/bs-node/src/driver.rs:1518` | none |

`audit.seen` is keyed by `(NodeId, NodeId, tree, segment)`, so it also grows with time,
not only with peers. DHT registrations and receipts do expire (`node/dht.rs:72`,
`incentive.rs`). The 18 MiB per viewer in the runbook was measured over minutes, not days.

### The desktop app has no viewer-side help

The app runs on a laptop. There is no mobile client at all, so "viewers on mobile" is
not covered by any code in the repository. A viewer on a phone can open the player page
of a laptop node on the same Wi-Fi only.

### Linux relays get a 20th of the intended socket buffer

`set_recv_buffer_size(8 MiB)` is silently capped by `net.core.rmem_max` (208 KiB on a
stock Linux kernel, doubled by the kernel). The driver never logs the effective size and
no doc mentions the sysctl. The comment at `transport.rs:25` says the kernel caps the
request "where needed", which hides the cause of the plain-frame backlog the same
comment describes.

## Bugs

1. **A viewer with a dead publisher reports no error.** Severity: High.
   `src/crates/bs-node/src/driver.rs:1055` (only bootstrap silence is checked) and
   `src/crates/bs-core/src/node.rs:2250` (only STREAM_END terminates).
   Reproduce: `bsnode publish --file x --max-bytes 50000000`, `bsnode watch --player
   127.0.0.1:8080`, then `kill -9` the publisher. The viewer stays `Active`,
   `last_error` is `None`, `starved` rises forever. A test on `Driver` that feeds
   no frames for 30 seconds after `FirstChunk` and asserts `last_error.is_some()`
   fails. Fix: in `check_bootstrap_silence` (rename it), also track the last chunk
   delivery time after `FirstChunk`; past `N` starved seconds with every tree
   parentless, set `last_error = "no data for N s: the publisher is gone or
   unreachable"`, and clear it on the next delivery. Effort: small. Self-contained: yes.

2. **ffmpeg failure is reported as a normal end of stream.** Severity: High.
   `src/crates/bs-ingest/src/ffmpeg.rs:276` and `src/crates/bs-node/src/driver.rs:765`.
   Reproduce: `bsnode publish --ingest ffmpeg --input clip.mp4`, then `kill` the ffmpeg
   child. The publisher logs `ffmpeg ingest ended`, sends STREAM_END and exits 0 with
   `state: Terminated` and no `last_error`. Fix: have `FfmpegSource` keep the `Child`,
   call `try_wait` when a rendition ends, and return `Err(status)` from `next_chunk`
   (or expose `exit_status()`); the driver sets `last_error = "ingest: ffmpeg exited
   with ..."` and still ends the stream. Effort: small. Self-contained: yes (two files).

3. **The player API serves media and status to any origin on any interface.**
   Severity: High. `src/crates/bs-player-api/src/lib.rs:208` (`allow_any_origin`),
   `:194` (bind), and the `0.0.0.0` defaults in `src/docs/MULTI_MACHINE.md`,
   `src/crates/bs-node/src/main.rs:361` and `src/fleet/entrypoint.sh`.
   Reproduce: `bsnode watch --player 0.0.0.0:8080`, then from another host `curl
   http://<ip>:8080/api/status` and `websocat ws://<ip>:8080/ws` receive everything.
   Fix: default the player bind to `127.0.0.1`; when the bind address is not loopback,
   require a token (`?t=` or `Authorization`) generated at start and printed with the
   URL, and echo the request `Origin` instead of `*`. Effort: small for the loopback
   default, medium with the token. Self-contained: no (a decision on how a LAN viewer
   gets the token).

4. **Default seeds give every CLI publisher the same identity and stream id.**
   Severity: High. `src/crates/bs-node/src/main.rs:60` (`seed` default 1), `:130`
   (viewer seed 2), `src/crates/bs-core/src/node.rs:178`. Reproduce: `bsnode stream-id`
   on two machines prints the same id; two `bsnode publish` on different machines share
   a NodeID, and `watch` of one can be answered by the other. Fix: default `seed` to
   `None` and draw 64 random bits when unset; print the seed so a run is reproducible.
   Effort: small. Self-contained: yes. The deeper fix, a stored key pair, is the
   persistence feature below.

5. **The player HTTP server is never shut down.** Severity: Medium.
   `src/crates/bs-node/src/driver.rs:456`: the `JoinHandle` from `serve` is dropped, which
   detaches the task. The listener stays open after the node ends. In the desktop app
   every start and stop cycle leaks one listening TCP socket and the open WebSocket
   sessions keep ticking with the last status (`lib.rs:300`, `borrow_and_update` on a
   closed channel never errors). Reproduce: in the app start and stop a viewer ten
   times, `lsof -p <pid> | grep LISTEN` shows ten listeners. Fix: keep the handle in
   `Driver`, abort it after `report()`, and have `ws_session` return when
   `status_rx.changed()` errors. Effort: small. Self-contained: yes.

6. **Stop does not flush DISCONNECT frames.** Severity: Medium.
   `src/crates/bs-node/src/driver.rs:772-773`: on stop the loop feeds `Quit` and breaks
   at once; on `--duration-s` it breaks without `Quit`. The frames `quit()` emits go to
   unbounded channels drained by writer tasks, then `endpoint.close` (`:779`) cuts every
   connection. Children learn of the departure by QUIC close, not by the drain notice,
   and the receipts `flush_all_receipts` signs are lost. Reproduce: a `swarm` of ten
   with `--duration-s 30`; the publisher's `sent` counters show DISCONNECT emitted and
   the viewers' `received` show none. Fix: after `Quit`, wait for each writer channel to
   empty (bounded by 500 ms, like the Terminated grace) before `endpoint.close`, and feed
   `Quit` on the duration deadline too. Effort: small. Self-contained: yes.

7. **Per-peer tables in `bs-core` are never pruned.** Severity: Medium (over days).
   See the table under "Memory grows with every peer ever seen". Reproduce: a simulator
   run with 1,000 joins and leaves over a relay, then assert `node.audit.seen.len()` and
   `incentive.known_keys.len()` are bounded by the live peer count. Fix: on
   `SessionClosed` and on the periodic timer, drop entries for peers not in the active
   set, sessions or trees for longer than the eviction timeout; bound `audit.seen` by
   segment age like `pull.bitfields`. Effort: medium. Self-contained: no (which
   tables must survive a peer's departure, such as `evicted_ids`, is a protocol
   question).

8. **The desktop app hides the player the moment the node ends.** Severity: Medium.
   `src/clients/desktop/ui/main.js:63-67`: `running == false` removes the iframe `src`.
   On a normal STREAM_END the viewer's task returns, the video vanishes mid-buffer, and
   the only trace is `(stopped) state Terminated` in the status line. With bug 1 the
   opposite happens: the video freezes and nothing says why. Fix: keep the frame until
   the user presses Stop, and show a banner from `state` (`Terminated` = "stream
   ended", `failed` = the error). Effort: small. Self-contained: yes.

9. **`stop` holds the app state lock for up to 10 seconds.** Severity: Low.
   `src/clients/desktop/src-tauri/src/node.rs:183-193`: `stop_node` awaits the task
   under the mutex, so the one-second `status` poll blocks and the window freezes its
   counters. Fix: take `stop` and `done` out of the slot, drop the lock, then await.
   Effort: small. Self-contained: yes.

10. **The runbook's collect step writes a file where a directory is needed.**
    Severity: Low. `src/docs/MULTI_MACHINE.md`, "Collect the verdict":
    `cp pub/result.json results/swarm/raw/publisher/` fails or, without the trailing
    slash, creates a file named `publisher`, which `report.py --dir` (searches for
    `result.json`) ignores, so the hash check has no publisher. Fix: `mkdir -p
    results/swarm/raw/publisher` first. Effort: small. Self-contained: yes.

11. **`swarm --player-base` can overflow.** Severity: Low.
    `src/crates/bs-node/src/main.rs:511`: `base + i` on `u16` panics in debug and wraps in
    release for `--player-base 65530 --count 10`. Fix: `checked_add` with an error.
    Effort: small. Self-contained: yes.

12. **`hostname` is shelled out and falls back to `node`.** Severity: Low.
    `src/crates/bs-node/src/main.rs:348`. In a container without the `hostname` binary
    or on a locked-down Windows PATH, every machine's swarm is labelled `node` and
    `--results` directories from different machines collide when merged. Fix: the
    `hostname` crate or `gethostname`, and warn when the fallback is used. Effort:
    small. Self-contained: yes.

13. **A second Ctrl-C does nothing.** Severity: Low. `src/crates/bs-node/src/main.rs:441`:
    tokio's `ctrl_c` disables the default action for the rest of the process, and the
    handler task exits after the first signal. If a node does not stop (a stuck writer,
    a long `report`), the user needs `kill -9`. Fix: loop in `stop_on_signal` and call
    `std::process::exit(130)` on the second signal. Effort: small. Self-contained: yes.

14. **Linux socket buffers are silently capped and never reported.** Severity: Low.
    `src/crates/bs-node/src/transport.rs:66-67`. Fix: read back `recv_buffer_size`
    after the set, log it at `info`, and warn with the sysctl line when it is under
    1 MiB. Effort: small. Self-contained: yes.

## Missing features

| Feature | Why | Size |
|---|---|---|
| Persistent identity and state directory | A key pair, cached proof of work, last share link and settings in `~/.config/bitstream` (or the OS equivalent). Without it every restart is a new peer (bug 4) and the app forgets everything. | 3 days |
| Reachability self-check for the broadcaster | The publisher cannot tell whether UDP 4000 is reachable from outside; the user finds out from a viewer's 5-second timeout. Needs a second node or a reflected PING from a joining viewer, and a clear message in the app. | 3 days |
| Viewer health and error model | A single `health` field (`connecting`, `playing`, `degraded`, `stalled`, `ended`, `failed`) with a reason, derived in the driver from deliveries, parents and the bootstrap. Bugs 1 and 8 are the symptoms. The player page and the app render that field. | 3 days |
| Player API access control | A per-run token and an origin check so a page on the LAN or behind a port forward can be opened by the owner only (bug 3). | 2 days |
| Mobile viewer | No code exists. The smallest path is the web player plus a node on the phone, which means building `bs-node` for iOS and Android (QUIC over UDP works there) and a thin shell; or a mobile viewer that only pulls from a LAN relay. | 20 to 40 days |
| Desktop app polish for non-experts | File picker instead of a typed path, a bundled or auto-downloaded ffmpeg, an "add to firewall" prompt, a log file under the state directory with rotation, a crash report when the node task panics. | 5 days |
| Log file with rotation | Logs go to stderr only; the desktop app has no console in release (`windows_subsystem = "windows"`), so a user cannot report what happened. | 1 day |
| Windows and Linux verification | No CI or doc for Windows. The code has no unix-only dependency, but ffmpeg discovery (`which_ffmpeg` looks for `ffmpeg`, not `ffmpeg.exe`, `node.rs:355`), the firewall prompt and the Tauri bundle are untested. | 2 days |
| Result files on crash | `result.json` is written only on a clean exit. A periodic snapshot (every 10 seconds) would let `report.py` score a run that lost a machine. | 1 day |

## Open questions

- Does WKWebView (macOS) and WebView2 (Windows) allow a `tauri://` page to embed an
  `http://127.0.0.1` iframe without a CSP, and does `autoplay muted` start inside the
  frame? The README says it works; no test covers the window.
- How long does `--spec-pow` take on a laptop, and is that acceptable per start without
  a cached key?
- What is the resident memory of a relay after 24 hours with churn? Nobody has measured
  it; the tables above are the suspects.
- Should the default `--player` stay off for `watch`? A non-expert needs the page, so the
  desktop app is the product surface and the CLI is for fleets.
- Where do protocol-level failures (`vb_rejected` rising, every PROBE refused, address
  rebound loop) surface to the user? Today they are counters in `result.json` and
  one-line warnings; none sets `last_error`.

Related, already filed: ISSUE-084 (punch timing) and ISSUE-087 (CGNAT allowlist) in
`issues/README.md`; the NAT path's "not yet through a real NAT" note in the runbook.
