# Web player

Single-file Media Source Extensions player served by `bs-player-api` at `GET /`
(the HTML is embedded into the binary with `include_str!`). No build step.

Run a viewer with `bsnode watch ... --player 127.0.0.1:8080` and open
http://127.0.0.1:8080/.

Protocol between `bs-player-api` and the page:

- `GET /ws`: on connect a JSON text frame `{"type":"hello","layers":N}`; then
  binary frames `[kind u8][layer u8][segment u32 BE][chunk u8][payload]` with
  kind 1 = fMP4 init segment, 2 = fMP4 media fragment; and a JSON status text
  frame every second (`state`, `depth`, `played`, `starved`, `first_chunk_s`,
  `layers`).
- `GET /api/status`: the same status as JSON.

The page picks the highest layer whose media fragment arrived for the chunk
(capped by the selector), appends that layer's cached init segment when it
switches, and keeps playback within 3 s of the buffered end.
