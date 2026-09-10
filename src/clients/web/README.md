# Web client (placeholder, M2)

A Media Source Extensions player that connects to a local `bsnode`'s player API
over WebSocket (`ws://localhost:PORT/ws`) and appends the fMP4 fragments it
receives to a `SourceBuffer`. It speaks no BitStream protocol itself; the node
does the swarm work.

`index.html` is a skeleton only. It will become functional when `bs-player-api`
serves fragments (M2). A browser *peer* over WebTransport is M4.
