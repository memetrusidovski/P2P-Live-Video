# Desktop client (placeholder, M4)

Plan: a Tauri app that ships `bsnode` as a **sidecar** binary. On launch it
starts `bsnode watch` with a local player-API port, then loads the web client
from `clients/web` pointed at that port. One protocol binary, one UI codebase;
the CLI and the desktop app cannot diverge.

Nothing is scaffolded yet. Scaffold with `npm create tauri-app@latest` once the
web client plays a stream (M2), and register `bsnode` under
`tauri.conf.json > bundle > externalBin`.
