# Multi-machine runs

One publisher, any number of machines, each running one viewer or a swarm of them.
Works on a LAN and over a VPN; see [Over the Internet](#over-the-internet) for the
limits without a VPN.

## Build on every machine

```sh
cargo build --release -p bs-node      # target/release/bsnode
```

Copy the binary or build in place. Only the publisher needs ffmpeg (for video; a file
streams as opaque bytes without it). Allow `bsnode` through the firewall when the OS
asks: viewers accept QUIC from each other on UDP ports the OS picks.

## Start the publisher

On the main laptop:

```sh
bsnode publish --ingest ffmpeg --input movie.mp4 --renditions 360p:800,720p:2500 \
  --listen 0.0.0.0:4000 --player 0.0.0.0:8079
```

It prints the join commands with the address peers must use:

```
join from another machine:
  bsnode watch --bootstrap 10.0.1.212:4000 --stream 25f3d0be... --player 0.0.0.0:8080
  bsnode swarm --bootstrap 10.0.1.212:4000 --stream 25f3d0be... --count 10 --player-base 8080
```

The stream id comes from the key in `~/.bitstream/publisher.key`. It stays the same
across restarts while that file does.

The address is the one the OS routes to the Internet through. On a VPN pass the VPN
address: `--advertise-ip 100.64.0.5` (or `--listen 100.64.0.5:4000`).

A live source takes `--live` and the ffmpeg arguments that open it. One ffmpeg process
decodes the input once and encodes every rendition.

```sh
# camera and microphone on macOS (list devices: ffmpeg -f avfoundation -list_devices true -i "")
bsnode publish --ingest ffmpeg --live --input-args "-f avfoundation -framerate 30" \
  --input "0:0" --renditions 360p:800,720p:2500 --listen 0.0.0.0:4000
# an RTMP listener: point OBS at rtmp://<this machine>:1935/live/stream
bsnode publish --ingest ffmpeg --live --input-args "-listen 1" \
  --input rtmp://0.0.0.0:1935/live/stream --renditions 360p:800,720p:2500 --listen 0.0.0.0:4000
```

Pass `--fps` if the source is not 30 frames per second. The camera and RTMP commands
are untested here: the live path was checked with ffmpeg's test pattern source only.

For a byte-exact check instead of video, publish a file: `--file sample.mp4
--max-bytes 25000000`. The stream ends when the file ends, so make it longer than the
time you need to start every machine.

## Join from each machine

Ten viewers in one process, with a player page per viewer and a result file per viewer:

```sh
bsnode swarm --bootstrap 10.0.1.212:4000 --stream 25f3d0be... --count 10 \
  --player-base 8080 --results ~/bs-results
```

Or one viewer with a page you can watch: the `watch` line the publisher printed, then
open `http://localhost:8080/`.

`--bootstrap` takes several contacts, separated by commas. Any node of the swarm that
accepts incoming UDP is a contact, so add a long-running viewer next to the publisher:
`--bootstrap 10.0.1.212:4000,10.0.1.77:4000`. The viewer asks every contact and joins
through the ones that answer.

Use the printed address everywhere, including on the publisher's own machine. A viewer
that bootstraps via 127.0.0.1 binds its identity to 127.0.0.1. A publisher that
advertises a LAN address then rejects every block from it: its `vb_rejected` counter
rises and the viewer stays in `Connecting`. `bsnode` warns about a loopback bootstrap.

Every 10 seconds the swarm prints how many viewers play, the chunks played and starved,
and any error a viewer reports. At the end it prints one line per viewer and a total:

```
viewer         state         join s  played starved   full  p.lost  hash
desktop-0      Terminated       6.2     133       0    134       3     -
...
desktop: 10 viewers, 10 played, 0 never played, 0 hash mismatches, starvation 0.000 %, join median 5.2 s p90 6.0 s
```

`join s` counts from the viewer's start, so it includes the publisher's start delay for
viewers that joined before the first chunk. The exit code is 3 if a viewer never played
or its hash differed.

Size each machine by cores. On a 10-core M4, 40 viewers ran clean at half load and 100
saturated it (every viewer starved). Plan 5 viewers per core for a measurement and
confirm with the starvation line. Each viewer takes about 18 MiB.

## Watch it live

From any machine that can reach the others:

```sh
python3 fleet/dashboard.py --publisher 10.0.1.212:8079 \
  --viewers 10.0.1.212:8080-8089,10.0.1.30:8080-8099,10.0.1.31:8080-8089
open http://localhost:9000/
```

One row per viewer with state, depth, chunks played and starved, and the per-chunk hash
check against the publisher. Each row links to that viewer's page.

## Collect the verdict

Copy every machine's results directory into one folder, with the publisher's result
beside them, and build the verdict:

```sh
mkdir -p results/swarm/raw
scp -r user@desktop:bs-results/ results/swarm/raw/desktop/
scp -r user@laptop2:bs-results/ results/swarm/raw/laptop2/
cp -r ~/bs-results/* results/swarm/raw/          # the main laptop's swarm
cp pub/result.json results/swarm/raw/publisher/   # from publish --result pub/result.json
python3 fleet/report.py --dir results/swarm/raw --start-delay-s 3
```

`report.py --dir` reads every `result.json` under the folder, scores starvation, join
latency, hash mismatches and viewers that never played with the same bounds as the
docker fleet, and writes `results/swarm/verdict.json`. `--profile lossy` relaxes the
starvation bound for a lossy link. Pass the publisher's `--start-delay-s` so the wait
for the first chunk does not count as join latency.

## Over the Internet

A VPN that gives every machine an address the others can reach (Tailscale, WireGuard,
a corporate VPN) needs nothing else. The publisher advertises its VPN address and
viewers detect theirs from the route to the bootstrap.

Without a VPN, the publisher works behind a port forward: forward UDP 4000 and start it
with `--advertise-ip <public ip>`. To have the publisher look the address up, pass a
STUN server: `--stun stun.l.google.com:19302`. It sends one request to that server,
prints the public address, and warns if the router maps the port to another one. A
publisher that advertises its LAN address to viewers on other networks is rejected by
all of them.

Tell every node its real uplink with `--upload-kbps`. The publisher assumes 100 Mbps
and a viewer 20 Mbps. A node that overstates its uplink accepts more children than it
can feed, and they starve. A viewer behind a NAT binds its identity to the
address the publisher's first PONG reflects, re-registers under it, and logs
`address rebound to <ip:port>`. From then on the publisher accepts its frames. Two
viewers behind different NATs reach each other only through a mapping the NAT keeps
open: a full cone, or a cone that the hole punch opens. A symmetric NAT or CGNAT
viewer is served only by a public relay. This path ran on the LAN and in the
simulator, not yet through a real NAT.

## When a viewer does not play

| Symptom | Cause |
|---|---|
| `bootstrap X: no answer in 5 s` | Wrong address, UDP 4000 blocked, or the publisher's stream ended |
| `Connecting` with no depth, publisher `vb_rejected` rising | Mixed loopback and LAN addresses, or a NAT between the viewer and the publisher |
| Plays, then `parent lost` and starvation on one machine | That machine's firewall drops incoming UDP from other viewers; allow `bsnode` or run it as `--class leaf` so it never relays |
| Everything starves at once | The publisher's machine is saturated; move viewers off it |
| `no media for over 10 s: the publisher is gone or unreachable` | The publisher stopped without ending the stream, or its network changed. The viewer plays again if media returns |
| Publisher exits with code 4 and `ingest: ffmpeg ended with ...` | The encoder crashed. Viewers got a STREAM_END |
| `--stream` is missing | A viewer needs the stream id the publisher printed |
| `UDP receive buffer is N KiB of the 8192 KiB asked for` | The kernel caps socket buffers. On Linux raise `net.core.rmem_max` and `net.core.wmem_max` to 8388608 |
| `plain_rate_limited` above zero in a result file | A source sent more plain datagrams than its budget: 50 per second with no open session |
