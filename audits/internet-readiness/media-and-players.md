# Internet readiness: media pipeline and players

Area `media-and-players`. Scope: `bs-ingest` (ffmpeg), `bs-player-api`, `clients/web`,
`clients/desktop`, and the opaque file mode. Judged against a broadcaster on a home
connection and viewers in a browser or the desktop app on home and mobile connections.
Reviewed 2026-11-09 against commit `1d6a7e1`.

Verdict: the pipeline plays a pre-recorded file end to end on a LAN with two
renditions and AAC audio. Nothing in this area has run over a real NAT. Three product
facts stand in the way of an Internet audience. A phone has nothing to connect to. The
broadcaster can only stream a file (no camera, no OBS). The browser player corrupts or
freezes the picture on every rendition change, because changes are not keyframe
aligned.

## What works today

- One ffmpeg process per rendition, libx264 veryfast zerolatency, AAC 96 kbps stereo in
  every rendition, 250 ms fragments, keyframe every second
  (`src/crates/bs-ingest/src/ffmpeg.rs`, `FfmpegSource::spawn`). Measured on this
  machine with ffmpeg 8.0.1: fragments of 7 or 8 video frames and 12 AAC frames, init
  segment 1275 bytes.
- Fragments split into INIT and MEDIA units and framed as TLV records inside each layer
  (`src/crates/bs-ingest/src/mp4.rs`, `container.rs`). The INIT and a META record
  (fps, height, kbps, label) ride the first chunk only; the publisher builds the signed
  `STREAM_DESCRIPTOR` from them (`src/crates/bs-node/src/driver.rs`,
  `build_descriptor`), so a late joiner gets initialisation data without waiting for a
  segment boundary.
- fps probed with ffprobe `r_frame_rate` when `--fps` is absent (`probe_fps`).
- Player API: axum on the address given, `GET /` serves the page embedded with
  `include_str!`, `GET /api/status`, `GET /api/descriptor`, `GET /ws` with hello,
  descriptor plus one kind-1 init frame per layer on connect, then kind-2 media frames
  and a status JSON frame per second (`src/crates/bs-player-api/src/lib.rs`).
- Web player: one `MediaSource`, one `SourceBuffer` in `segments` mode, codec string
  from the init (`avc1.PPCCLL` plus `mp4a.40.2` when an `mp4a` box exists), picks the
  highest delivered layer under the selector's cap per chunk, appends the cached init on
  a switch, seeks to the live edge when more than 3 s ahead, trims on
  `QuotaExceededError`, reconnects the socket 2 s after close
  (`src/clients/web/index.html`).
- Desktop app: Tauri 2 shell linking `bs-node` as a library, one node per app, viewer
  from a `bs://host:port/stream-id` link, publisher from a file path with ffmpeg or raw
  bytes, player bound to `127.0.0.1:0` and shown in an iframe, status polled every
  second, stop with a 10 s timeout (`src/clients/desktop/src-tauri/src/node.rs`,
  `link.rs`, `ui/main.js`). Two end-to-end tests cover raw and ffmpeg publish to a
  viewer over localhost.
- Opaque file mode: `Media::File` with the reference ladder, descriptor
  `Opaque`/`Independent`/`data`, the page shows no video and the sink hashes the bytes
  for `--expect-hash`.
- Shed and fold reach the player as a missing layer in the delivered set; the page falls
  back to the highest layer present. The mechanism is fine; the switch it triggers is
  bug 1.

## What breaks on the Internet

### A phone browser cannot view at all

The page is served by the viewer's own `bsnode`; the browser is a loopback client of a
node, not a peer. A phone has no node. On the LAN a phone can open the laptop's player
port and become a second screen of that laptop's node; over the Internet, with no
servers, there is nothing for it to open. The browser cannot run the protocol either:
QUIC datagrams and a listening UDP socket are not available to web pages, so even a
wasm build of `bs-core` can only be a leaf behind a publicly reachable relay that
speaks WebTransport. "Viewers on mobile connections" today means laptops on hotspots.
Product consequence: either a native mobile app (Tauri 2 mobile around `bs-node`, with
the cellular NAT answered by relays) or a WebTransport leaf mode plus volunteer public
relays. See missing features.

### Every rendition change damages the picture

`clients/web/index.html` `flushBatch` switches layers on any chunk. Only one fragment in
four starts with a keyframe (measured: fragments 1, 4, 8, 12 of a 30 fps clip). After
an init append MSE drops coded frames until the next random access point, so a switch
on the other three chunks drops up to 750 ms of video, leaves a hole in the buffer and
stalls the element at the hole (see bug 3 for how long). On a lossy home or mobile
link, a chunk where layer 1 is incomplete but layer 0 is complete drops the player to
layer 0 and the next chunk lifts it back: two bad switches per loss event. A fold or a
shed is one bad switch each way. Bug 1.

### A 3 to 5 s stall freezes the player for up to 3 s longer than the stall

Starved chunks are never delivered, so the buffer has a hole. The video element stops
at the hole. The only seek in the page fires when the buffered end is more than 3 s
ahead of `currentTime`, so after the network recovers the picture stays frozen until
3 s of new media sit in the buffer, then jumps. The node's own 3 s playout anchor sits
in front of this, so a 4 s network stall costs the viewer about 7 s of frozen picture.
Bug 3. A decode error during any of this ends the `MediaSource` and the page is dead
until reload (bug 4).

### The broadcaster can only stream a file

`FfmpegSource::spawn` takes a path and runs `ffmpeg -re -i <path>`. No camera, no
microphone, no screen, no RTMP or SRT listener for OBS. The desktop app requires
`path.is_file()`. A broadcaster on a home connection has no way to go live with a
camera. The `-re` flag and the counter-based chunk timestamp (`timestamp_us =
k * 250 ms`) are both wrong for a live input: a stalled input shifts every later
chunk's timeline without any signal to viewers. Missing feature 1, bug 11.

### The player port is open to whoever can reach it

`bsnode publish` prints join commands with `--player 0.0.0.0:8080`
(`src/crates/bs-node/src/main.rs:364`) and the runbook uses `0.0.0.0:8079`. The API
has no authentication and no `Origin` check; the CORS middleware allows every origin
and its comment assumes a loopback bind. On a port-forwarded or VPN box anyone who
reaches the port gets the stream, the node id and the running hash. Even on
`127.0.0.1`, any web page the viewer visits can open `ws://127.0.0.1:8080/ws`
(WebSockets are not CORS gated) and read the stream. Bug 6.

### A home uplink is not 100 Mbps

The desktop publisher form defaults to 100000 kbps upload and the viewer form to
20000 kbps (`src/clients/desktop/ui/index.html`); nothing measures the link. The node
sizes its slots from this number. On a 20 Mbps home uplink the publisher admits five
times the children it can serve. The share link carries the LAN address and the README
tells the user to edit in the public address and forward UDP 4000 by hand. NAT and
slot sizing belong to other areas; the desktop flow that exposes them is here (missing
feature 5).

### Simulcast is downloaded in full

A viewer wants every layer (`--top_layer` defaults to all; `bs-core/src/node.rs:1118`)
and the page plays one of them. Watching 720p on the default ladder costs
360p + 720p plus two audio tracks, 3.3 Mbps instead of 2.5 Mbps, and a 3-rendition
ladder costs the sum. On a mobile connection the viewer pays for renditions it never
shows. The layer model in the spec is additive (SVC); simulcast rides it without a
per-rendition subscription. Missing feature 3; bug 2 is the same mismatch on the
delivery side.

## Bugs

### 1. Rendition switch is not keyframe aligned

- Severity: High
- Where: `src/clients/web/index.html`, `flushBatch`, the `if (best !== currentLayer)`
  block; `src/crates/bs-ingest/src/ffmpeg.rs` `-frag_duration 250000` with a 1 s GOP.
- Reproduce: publish `--ingest ffmpeg --renditions 360p:800,720p:2500`, open the
  viewer page, set "max rendition" to 0 while layer 1 plays. Three times in four the
  picture stops or shows decode artefacts for up to 750 ms and the buffer shows a hole.
  Script check: run the ffmpeg command from `spawn` on any clip and read the first
  sample flags of each `moof` (done for this audit: only fragments 1, 4, 8, 12 are
  sync samples).
- Fix: switch only on a chunk whose fragment starts with a sync sample. The page can
  read it from `moof/traf/trun` first-sample flags (or the `tfhd` default flags). Or
  the ingest marks keyframe fragments with a flag byte in the MEDIA record and the page
  reads that. Keep the old layer's fragment for non-key chunks.
- Effort: medium. Self-contained: yes (the page alone can do it).

### 2. Simulcast delivery drops a complete higher rendition when a lower one is incomplete

- Severity: Medium
- Where: `src/crates/bs-core/src/swarm.rs:774` (`if !cb.layer_complete(l) break`),
  `src/crates/bs-node/src/driver.rs:1400` `deliver` (`take(layers_complete)`).
- Reproduce: a chunk whose layer 0 blocks are lost and layer 1 complete. The chunk
  counts as starved and the page gets nothing, although the 720p fragment is playable
  on its own. A `bs-sim` scenario that drops tree-0 symbols for one chunk under a
  `Simulcast` descriptor shows `layers_complete = 0`.
- Fix: for `LayerMode::Simulcast` and `Independent`, report a completeness bitmask and
  deliver every complete layer; keep the leading-prefix rule for `SvcTemporal`.
- Effort: medium. Self-contained: no (`Output::Deliver` carries a count; the change is
  a design decision shared with the simulator KPIs).

### 3. The page does not cross a buffer hole until 3 s of media pile up

- Severity: Medium
- Where: `src/clients/web/index.html`, `flushBatch`, "Stay near the live edge"
  (`if (end - video.currentTime > 3.0)`).
- Reproduce: run two viewers, one the other's parent, kill the parent; or run the
  docker fleet `lossy` profile and watch the page: after each starved chunk the picture
  freezes and resumes with a jump about 3 s later. A `broadcast` lag in
  `bs-player-api` (`Lagged(n)` in `ws_session`) makes the same hole.
- Fix: on `waiting` or a 250 ms stall timer, if `currentTime` is within 100 ms of a
  buffered range end and a later range exists, seek to its start plus 50 ms. Use the
  same path for the first play instead of the 3 s rule.
- Effort: small. Self-contained: yes.

### 4. A SourceBuffer or media error is fatal until the page reloads

- Severity: Medium
- Where: `src/clients/web/index.html`, `sb.addEventListener('error', ...)` logs only;
  no `video.onerror`.
- Reproduce: append any malformed fragment (bug 14 does it with a raw file), or trigger
  bug 1 on Firefox, which reports a decode error instead of dropping frames.
  `MediaSource.readyState` becomes `ended`, the queue keeps growing, nothing plays.
- Fix: on either error revoke the object URL, drop `ms`, `sb`, the queue and
  `currentLayer`, and rebuild on the next keyframe chunk with the cached init.
- Effort: small. Self-contained: yes.

### 5. Socket reconnect keeps the old MediaSource timeline

- Severity: Low
- Where: `src/clients/web/index.html`, `connect` and `ws.onclose`.
- Reproduce: `bsnode watch --player 127.0.0.1:8080`, open the page, restart `bsnode`
  on the same stream. The socket shows `open` again, the new fragments land in a second
  buffered range at timestamp 0, `currentTime` stays in the old range, and the live-edge
  check reads the last range so it never seeks.
- Fix: reset `ms`, `sb`, `inits`, `currentLayer` and `pendingBatch` on `hello`.
- Effort: small. Self-contained: yes.

### 6. Player API has no origin check and the printed commands bind it to every interface

- Severity: Medium
- Where: `src/crates/bs-player-api/src/lib.rs` `serve`, `ws_upgrade`,
  `allow_any_origin`; `src/crates/bs-node/src/main.rs:364-368` (prints
  `--player 0.0.0.0:8080`); `src/docs/MULTI_MACHINE.md` (`--player 0.0.0.0:8079`).
- Reproduce: start a viewer with the printed command, then from another machine on the
  LAN or VPN open `http://<ip>:8080/` and watch the stream. From any website in the
  viewer's browser, `new WebSocket('ws://127.0.0.1:8080/ws')` receives the media frames.
- Fix: bind `127.0.0.1` by default and print that; reject `/ws` and `/api/*` requests
  whose `Origin` is not the bound host (the desktop iframe has that origin); for a
  deliberate non-loopback bind require a token in the page URL that the shell or the
  CLI prints.
- Effort: small. Self-contained: yes.

### 7. An ffmpeg that dies after start is silent and never restarted

- Severity: Medium
- Where: `src/crates/bs-ingest/src/ffmpeg.rs` `next_chunk` (`Ev::Ended` logs at
  `info`), `_children` exit status never read; `src/crates/bs-node/src/driver.rs`
  `setup_ingest` reports `IngestFailed` only when `spawn` fails.
- Reproduce: publish two renditions, `kill` one ffmpeg. That layer is empty for the
  rest of the stream, every viewer on it switches down (bug 1), `last_error` stays
  empty, the desktop app shows nothing. Kill both: the publisher sends `STREAM_END` as
  if the file ended.
- Fix: `child.wait()` per rendition; a non-zero exit or an exit before the input ends
  sets `last_error` with the last stderr line (capture stderr, it goes to the terminal
  today and the desktop app has none); for a live input restart the encoder.
- Effort: small. Self-contained: yes.

### 8. A rendition slower than real time adds 2 s of latency for good and desynchronises the layers

- Severity: Medium
- Where: `src/crates/bs-ingest/src/ffmpeg.rs` `ready` (`MAX_LAG = 8`), `build`.
- Reproduce: `--renditions 360p:800,1080p:6000` on a machine that cannot encode 1080p
  at real time. `ready` holds every chunk until the slow rendition has a fragment, so
  the whole stream runs at the slow encoder's rate. Once the fast queue holds 8
  fragments chunks ship with the slow layer empty, but that queue never drains below 8:
  2 s of added latency on every layer. When the slow rendition does fill a chunk, its
  fragment covers a time range 2 s older than layer 0's in the same chunk, so a switch
  jumps 2 s backward.
- Fix: a 250 ms chunk clock driven by the fastest rendition, and a lagging rendition
  drops fragments older than the chunk's time range instead of queueing them. Or one
  ffmpeg per input with `-filter_complex split` and one output per rendition, so the
  renditions share a decode and cannot drift. Either way, take the chunk's time range
  from the fragment `tfdt`, not the counter.
- Effort: medium. Self-contained: no (who owns the chunk clock is a design decision
  shared with the publisher driver).

### 9. The media buffer grows until the browser's quota

- Severity: Low
- Where: `src/clients/web/index.html` `trimBuffer`, called only from the
  `QuotaExceededError` path.
- Reproduce: watch for ten minutes with the DevTools memory panel open; `sb.buffered`
  spans the whole session. On a WebKit view the quota is smaller and the first trim
  comes with a visible hiccup.
- Fix: every 16 chunks remove `[0, currentTime - 30 s)`.
- Effort: small. Self-contained: yes.

### 10. An init segment over 4096 bytes is dropped from the descriptor without a warning

- Severity: Low
- Where: `src/crates/bs-node/src/driver.rs` `build_descriptor`
  (`.filter(|b| b.len() <= LayerDescriptor::MAX_INIT).unwrap_or_default()`),
  `src/crates/bs-wire/src/frames/media.rs:470`.
- Reproduce: an input with `udta` metadata or several audio tracks produces a `moov`
  above 4 KiB. The descriptor says `has_init: false`, the INIT record rides only chunk 0
  of the stream, so every late joiner holds media it can never start.
- Fix: refuse to publish (or strip `udta` with `-map_metadata -1`) and log which
  rendition overflowed; decide separately whether to raise `MAX_INIT`.
- Effort: small. Self-contained: yes for the warning; the limit is a spec constant.

### 11. Chunk timestamps are a counter, not a clock

- Severity: Low today, High once a live input exists
- Where: `src/crates/bs-ingest/src/ffmpeg.rs` `build` (`timestamp_us: k * CHUNK_US`).
- Reproduce: not reachable with a file input. With an input URL that stalls for 5 s,
  the publisher emits nothing, then resumes with the next `k`; viewers' chunk deadlines
  (anchored on segment numbers) have passed, so every chunk after the stall is late.
- Fix: stamp chunks from a monotonic publisher clock, and let the playout anchor follow
  a jump (the bs-core side is another area's question).
- Effort: medium. Self-contained: no.

### 12. Descriptor width assumes 16:9 and names no audio codec

- Severity: Low
- Where: `src/crates/bs-node/src/driver.rs` `build_descriptor`
  (`width: height * 16 / 9`, `codec_tag: b"avc1"`).
- Reproduce: publish a vertical phone video; the descriptor reports 640x360 for a
  360x640 rendition. The page ignores width, so cosmetic today; a native player that
  sizes from the descriptor would be wrong, and the descriptor cannot say whether audio
  exists.
- Fix: read width from the `tkhd` or `avcC`-adjacent `avc1` sample entry in the init;
  carry the audio codec in META.
- Effort: small. Self-contained: yes.

### 13. fps probe uses `r_frame_rate`

- Severity: Low
- Where: `src/crates/bs-ingest/src/ffmpeg.rs` `probe_fps`.
- Reproduce: a variable-frame-rate screen or phone recording reports `1000/1` or
  `90000/1`; the ingest passes `-g 1000` and the META says fps 1000. The stream still
  plays because `-force_key_frames` keeps a keyframe per second.
- Fix: use `avg_frame_rate`, fall back to `r_frame_rate`, clamp to 1 to 120.
- Effort: small. Self-contained: yes.

### 14. The player API decodes TLV records in opaque payloads

- Severity: Low
- Where: `src/crates/bs-player-api/src/lib.rs` `ws_session` runs `decode(payload)` on
  every layer regardless of the descriptor's `content_type`.
- Reproduce: publish `--file` with a file whose first byte is 0x01 or 0x02 followed by
  a length that fits the layer. The page receives a bogus init or media frame, appends
  it, and bug 4 ends the MediaSource.
- Fix: skip decoding unless the descriptor says `VideoCmaf`.
- Effort: small. Self-contained: yes.

## Missing features

1. Live ingest for a broadcaster: camera and microphone (`avfoundation`, `dshow`,
   `v4l2`), screen capture, and an RTMP or SRT listener so OBS can push. Drop `-re`
   for live inputs, one decode with `-filter_complex split` feeding all renditions,
   hardware encoders where present (VideoToolbox, NVENC, QSV), stderr capture. Depends
   on bugs 7, 8 and 11. 6 to 8 days.
2. A mobile viewer. Option A: Tauri 2 mobile (iOS and Android) around `bs-node` with
   the player in the WebView; needs QUIC and a UDP socket on the device, background and
   battery limits, and a relay path for the symmetric NAT every carrier uses (another
   area). Option B: `bs-core` in wasm as a leaf over WebTransport to a public relay,
   which needs relays that someone runs and a WebTransport channel in the protocol.
   Option A 15 to 20 days for a first build; option B 25 to 30 days plus the relay
   question. Until one exists, the audience is desktop only.
3. Per-rendition subscription for simulcast: a viewer subscribes to one rendition (plus
   the base as fallback) chosen from measured download capacity and the player's
   buffer, and re-subscribes on a change. Today a viewer downloads every rendition.
   Needs a `bs-core` design (tree subscription by rendition under `LayerMode::Simulcast`).
   4 to 6 days.
4. A player people can use: keyframe-aligned switching, hole jumping, error recovery
   and trimming (bugs 1, 3, 4, 5, 9), plus unmute and fullscreen controls, a latency
   readout, and a start that does not wait for the 3 s rule. 3 days.
5. Desktop publisher setup: measure the uplink (a short upload probe) instead of a
   100 Mbps default, learn the public address (STUN or the bootstrap's reflection) and
   put it in the share link, try UPnP or NAT-PMP for UDP 4000, and show the result.
   The NAT work belongs to another area; the app flow is 2 days once it exists.
6. Player API access control: loopback default, origin check, token for non-loopback
   binds (bug 6). 1 day.
7. Publisher preview and health: the publisher's page shows its own stream, ffmpeg
   restart and exit status in `last_error` (bug 7), encoder load and dropped-fragment
   counters in status. 2 days.
8. Audio-only and a lower rendition floor for mobile: a 240p or audio-only layer in the
   default ladder so a 1 Mbps link gets something. Half a day of ladder work once
   feature 3 exists.

## Open questions

- Does the publisher's own player page receive deliveries, or only viewers' pages? The
  desktop README calls it a "preview page"; `deliver` runs on `Output::Deliver`, which
  the publisher may not emit.
- Glass-to-glass latency has not been measured. Estimate from the code: ffmpeg about
  300 ms, chunk cut 250 ms, playout anchor 3 s, player at least 500 ms behind the
  buffered end; about 4 to 5 s. Is that the product target?
- Tauri on Linux uses WebKitGTK, where MSE depends on GStreamer plugins. Has the
  desktop app played video on Linux?
- iOS Safari exposes `MediaSource` only as `ManagedMediaSource` from 17.1. If option B
  of missing feature 2 is chosen, the page needs that path.
- Should `MAX_INIT` (4096 bytes) grow for HEVC or AV1 inits, or should the descriptor
  reference the init by hash and let the first chunk carry it?
