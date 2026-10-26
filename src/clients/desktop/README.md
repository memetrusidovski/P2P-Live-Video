# Desktop client

A Tauri 2 app that links `bs-node` as a library and runs one BitStream node in
the app process. The node is a viewer that joins a stream from a share link, or
a publisher that streams a video file. The window shows a control panel and the
web player the node serves on loopback (`src/clients/web/index.html`, served by
`bs-player-api`).

The app exists to show that the protocol crates work as a library behind a
narrow public surface. It changes nothing under `src/crates/`.

## Run

Needs Rust (the workspace toolchain), Node 18 or later, ffmpeg on PATH for
video, and the Tauri platform prerequisites for your OS (Xcode command line
tools on macOS).

```sh
cd src/clients/desktop
npm install              # the Tauri CLI only; the UI has no build step
npm run tauri dev        # window plus live reload of ui/
npm run tauri build      # a bundle under src-tauri/target/release/bundle/
```

`cargo build`, `cargo run` and `cargo test` inside `src-tauri/` also work. The
tests cover the share link, a raw-bytes publisher feeding a viewer over
localhost, and a video publisher through ffmpeg (skipped without ffmpeg).

Logs go to stderr; `RUST_LOG=debug` raises the level.

## Use

- Publish: enter the path of a video file, keep the ingest on "Video through
  ffmpeg", press Start publisher. ffmpeg encodes the renditions in the field,
  lowest first as `<height>p:<kbps>`. Each rendition is one layer. "Raw bytes"
  sends the file as it is, which proves transport but shows no picture. The
  panel shows the share link with the address of the machine's default route
  and UDP port 4000. Before you send the link across the Internet, replace the
  host with your public address and forward UDP 4000.
- Watch: paste a share link, press Start viewer. The player appears once the
  first chunk arrives. The status line shows the node id, stream id, state and
  the chunks played and starved.
- Copy next to a field puts its value on the clipboard. The fields only change
  when the value changes, so a selection survives the one-second refresh.
- The error line shows the node's last problem. The usual ones: a bootstrap
  that did not answer in 5 s (wrong host, closed port), a bootstrap that
  rejected the session (proof-of-work parameters differ), an ffmpeg that did
  not start.
- Stop ends the node and shows its final state.

Every node the app starts uses the `bsnode` CLI defaults: the simulation
proof-of-work parameters, parity 1, forest size 3. Viewers are relay class with
NAT class auto and want every layer. A publisher started here and a viewer
started with `bsnode watch --player` interoperate, and the other way round.

## Share link

```
bs://<host>:<port>/<stream-id-hex>
```

`host:port` is a bootstrap contact, normally the publisher's public UDP
address. `stream-id-hex` is the 64-character hex form of `bs_wire::StreamId`.
The protocol defines no link format; this one belongs to the desktop client
and is parsed in `src-tauri/src/link.rs`.

The link carries no protocol parameters. The proof-of-work difficulty is a
network constant in the specification, so two nodes of the same network agree
on it without the link. A mismatch shows up as a rejected bootstrap session in
the error line.

## Layout

| Path | Role |
|---|---|
| `src-tauri/src/main.rs` | Tauri entry point, registers the commands |
| `src-tauri/src/node.rs` | Commands `start_viewer`, `start_publisher`, `stop`, `status`; builds the `RunConfig` and keeps the `Handle` |
| `src-tauri/src/link.rs` | Share link parse and format |
| `ui/` | Control panel, static files; the player is the node's own page in a frame |

The commands run on the Tauri tokio runtime, so `bs_node::start` spawns its
tasks there. `src-tauri/Cargo.toml` has an empty `[workspace]` table because
the package sits inside the workspace directory without being a member.

## Crate items the app uses

- `bs_node::{start, RunConfig, Handle, Media, RunReport, Status}`:
  `RunConfig::{publisher, viewer}`, `Media::{Ffmpeg, File}`;
  `Handle.{advertise_addr, player_addr, stream_id, node_id, status, stop,
  done}`; `RunReport.{state, chunks_played, chunks_published, steady_starved,
  first_chunk_s}`; `Status.{state, played, starved, layers, first_chunk_s,
  last_error}`.
- `bs_ingest::RenditionSpec::{parse_list, ladder}`.
- `bs_media::Ladder::reference` for raw bytes.
- `bs_wire::StreamId`: `FromStr` and `Display`.
