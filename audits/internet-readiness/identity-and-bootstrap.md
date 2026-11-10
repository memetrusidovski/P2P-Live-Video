# Identity and bootstrap

Area: how a node gets its NodeID and keypair, what proof-of-work it solves, what a viewer
needs to join, and what happens when the bootstrap or the publisher goes away.
Judged against: a broadcaster on a home connection, viewers behind NATs, no servers.

## What works today

- **Identity derivation is consistent across crates.** `bs_core::Node::new`
  (`src/crates/bs-core/src/node.rs:178-181`) builds a 32-byte Ed25519 seed from
  `cfg.seed` (8 bytes) and the bound UDP port (8 bytes), with 16 zero bytes. The driver
  mirrors it in `node_seed` (`src/crates/bs-node/src/driver.rs:478-482`) so the TLS
  certificate carries the same key, and `bsnode stream-id` mirrors it again
  (`src/crates/bs-node/src/main.rs:243-256`). A `debug_assert_eq!` at `driver.rs:409`
  checks the two agree.
- **Stream id = Blake3 of the publisher's public key** (`node.rs:267-270`). A viewer
  verifies every Stream Record it receives against the stream id and the record's
  signature (`node.rs:592-604`) and guardians refuse a record whose key does not hash to
  their stream id (`src/crates/bs-core/src/node/dht.rs:1136-1150`). A joiner cannot be
  handed another publisher's stream.
- **Proof of work is implemented and verified end to end.** `bs_crypto::pow`
  (`src/crates/bs-crypto/src/pow.rs`) solves the static puzzle at identity creation and
  the dynamic puzzle per external IP; the QUIC verifier checks the static puzzle and the
  NodeID binding on every handshake (`src/crates/bs-node/src/tls.rs:92`).
- **Bootstrap with one seed works on a LAN and over a VPN.** `dht_start`
  (`dht.rs:259-268`) sends the self-lookup straight to the `--bootstrap` address as an
  address-only placeholder (`seed_records`, `dht.rs:272-287`). It retries every 2 s while
  the routing table is empty (`dht.rs:1232-1243`). The driver reports
  `bootstrap X: no answer in N s` after 5 s of silence (`driver.rs:571`, `1056-1069`). The
  test `handle_reports_player_address_status_and_bootstrap_errors`
  (`src/crates/bs-node/tests/localhost.rs:123`) covers the dead-bootstrap report.
- **Any public relay-class node serves DHT requests.** `serves_dht` (`dht.rs:251-253`)
  is true for `Relay` class with reachability other than `Symmetric`; such a node answers
  `FIND_NODE`, `GET_PEERS`, `STORE_RECORD` and `REGISTER_PEER` (`dht.rs:890-907`) and
  returns its stored Stream Record in `GET_PEERS` (`dht.rs:1111`). A viewer that knows a
  public viewer's address can bootstrap from it.
- **The share link carries exactly what a viewer needs**: `bs://host:port/<stream-id>`
  (`src/clients/desktop/src-tauri/src/link.rs`). The desktop viewer resolves the host,
  builds `RunConfig::viewer` and starts (`src/clients/desktop/src-tauri/src/node.rs:114-125`).
  `bsnode publish` prints the equivalent `watch` and `swarm` lines (`main.rs:362-370`).
- **The desktop app gives every run a different identity** (`seed_now`,
  `node.rs:301-307`), so two app instances on one machine do not share a NodeID.

## What breaks on the Internet

### Every default `bsnode publish` in the world is the same node

`RunConfig::publisher` sets `seed: 1` (`driver.rs:133`), `bsnode publish --seed`
defaults to 1 (`main.rs:60-61`) and the port is 4000. The identity is a pure function of
those two numbers, so two strangers who both run the defaults have the same NodeID, the
same TLS key and the same stream id. A viewer that looks up that stream id finds whichever
publisher its bootstrap knows. Worse, the secret key is public knowledge: anyone can run
`Identity::from_seed([1,0,..,0xA0,0x0F,..])` and sign manifests, `STREAM_END` and Stream
Records for every default publisher. `Identity::generate` with a CSPRNG exists
(`src/crates/bs-crypto/src/identity.rs:29`) and nothing outside tests calls it.

Viewers have the same defect with a smaller key space: `RunConfig::viewer` sets
`seed: 2` (`driver.rs:160`) and binds port 0, so the identity is a function of the
OS-chosen port (`driver.rs:381-386`). Two viewers behind different NATs that land on the
same ephemeral port are one NodeID: the same routing-table entry (`dht.rs` keys the table
by NodeID), the same receipt key in `learn_key`, the same admission ban. Across 16 384
usable ports the first collision is expected around 150 concurrent viewers.

### A publisher restart strands every viewer

Nothing is persisted to disk: no keypair, no segment counter, no routing table, no record.
(`std::fs::write` in `driver.rs:622` and `:787` writes media output and `result.json`;
`node.rs:354` in the desktop crate is a test fixture.) Consequences, in order:

1. The ffmpeg source numbers segments from 1 on every process start
   (`src/crates/bs-ingest/src/ffmpeg.rs:303-306`: `k / CHUNKS_PER_SEGMENT + 1` with `k` a
   field that starts at 0). The Stream Record version also restarts
   (`src/crates/bs-core/src/publisher.rs:46`, `record_version: 0`).
2. A viewer that played through segment S rejects every manifest with
   `seg <= played_through` as `OutsideWindow` (`src/crates/bs-core/src/swarm.rs:283-285`).
   `played_through`, `live_edge` and `anchor` are never cleared on parent loss or on the
   fall to DISCOVERY (`swarm.rs:343` is the only assignment to `live_edge`, and it only
   moves forward). The ISSUE-070 widening (`swarm.rs:286-298`) opens the window upward
   only. This is case (A) of the audit's finding F5
   (`audits/traces/cold-start-n2.md:67`, `audits/2026-11-02-tick-walkthrough.md:103`),
   walked there as a spec silence. In the implementation it is the actual path: the
   viewer re-attaches, drops every manifest, signs no receipts, is evicted after three
   unreceipted segments, rejoins, and loops.
3. Any guardian that still holds the old record refuses the new one:
   `dht_serve_store` accepts only `rec.manifest_version >= current.manifest_version`
   (`dht.rs:1143-1150`), and `Store::expire` (`dht.rs:73-78`) drops registrations and
   queriers but never `record`. With the CLI (same seed, same key) the restarted publisher
   is its own guardian with an empty store, so its own `GET_PEERS` works. Every public
   viewer that stored the record keeps serving the stale one (old live edge, old matrix)
   to joiners for the life of its process.
4. With the desktop app the publisher gets a new key on every run (`seed_now`), so the
   stream id changes and the share link every viewer holds is dead. Viewers stay in
   DISCOVERY re-querying every second (`ARCHITECTURE.md` status row "re-queries every
   second") with no terminal state.

### A viewer that loses every contact never re-bootstraps

`next_bootstrap` is armed once in `dht_start` (`dht.rs:267`) and cleared the first time
the routing table is non-empty (`dht.rs:1241-1243`); `self_lookup_started` is never reset
(`dht.rs:264`). When every known contact later goes dead (the publisher restarts on a new
port, or the viewer's router reboots and the reflected address changes), dead pings remove
the contacts (`dht.rs:1220-1230`), the table is empty again, and nothing contacts the seed
again. On a LAN this never happens because nobody restarts.

### A dead bootstrap is reported, never acted on

After 5 s the driver sets `last_error` (`driver.rs:1062-1069`); the core keeps retrying
every 2 s forever. `bsnode watch` never exits and the desktop app shows the error with no
retry policy, no backoff and no terminal state. The spec's 180 s bound on DISCOVERY without
records (Ch1 §1.3.1) is not implemented: `grep -n 180 node.rs` finds nothing.

### One bootstrap address, no fallback

`RunConfig.bootstrap` is `Option<SocketAddr>` (`driver.rs:89`), `--bootstrap` is one
`String` (`main.rs:95`, `:146`), and the share link holds one `host:port`. The core already
takes `Vec<PeerAddr>` (`node.rs:81`, `dht.rs:136`). Ch2 §2.1.3 (`3_bootstrap_sequence.md:5-10`)
asks for DNS-SRV seeds from several operators plus a hardcoded list, and PINGs to two seeds
from different operators to classify reachability. With "no servers" the only contact is
the publisher's home IP: when that changes (DHCP lease, CGNAT) every link breaks.

### The default proof of work is the simulation one

`Params::for_simulation` is the default in `RunConfig::publisher`, `RunConfig::viewer`
(`driver.rs:132`, `:159`) and the CLI (`main.rs:231-239`): `c1 = 4`, `c2_min = 2`, and
`scale_cap` (`pow.rs:77-84`) caps the dynamic puzzle at 4 bits. An identity costs 16
hashes. `--spec-pow` (`main.rs:21-23`) is opt-in and global; a `--spec-pow` publisher
rejects a default viewer at the TLS handshake ("identity pow", `tls.rs:92`) after the DHT
exchange succeeded, which the driver reports as `bootstrap X: <err>` (`driver.rs:1143-1148`).
The desktop app has no way to set it.

Cost at spec difficulty: the static puzzle is 2^16 Blake3 hashes of 40 bytes on average.
Scaling the audit's measured tier-14 dynamic puzzle (2.5 ms desktop, 160 ms on a
Cortex-A53, `2026-11-02-tick-walkthrough.md:311`) by 4 gives about 10 ms on a laptop and
0.6 s on a low-end phone, with a geometric tail (3x at p95). The dynamic puzzle is 8 to 14
bits (0.04 ms to 2.5 ms desktop) and is re-solved for every new reflected IP. Both are
affordable on a phone; neither is run today.

## Bugs

1. **Default identities are shared and their secret keys are public**
   - Severity: High
   - Where: `src/crates/bs-node/src/driver.rs:133`, `:160`, `:478-482`;
     `src/crates/bs-node/src/main.rs:60-61`, `:243-256`; `src/crates/bs-core/src/node.rs:178-181`
   - Repro: start `bsnode publish` on two machines with defaults; `bsnode stream-id` prints
     the same id for both. For the key: `Identity::from_seed` with seed 1 and port 4000
     yields a key that verifies every default publisher's manifests. Test that fails:
     two `RunConfig::viewer` handles bound to the same port on two hosts report equal
     `node_id`.
   - Fix: add `identity: Option<Identity>` to `NodeConfig` and `RunConfig`; `bs-node`
     fills it with `Identity::generate(&mut OsRng, difficulty)` when unset and the
     simulator keeps the seed path. Drop the `seed`-derived `publisher_stream_id`
     default for `watch` (`main.rs:287-298`), which cannot survive the change.
   - Effort: medium. Self-contained: no (API change across bs-core, bs-node, bs-sim, desktop;
     ties to the persistence feature below).

2. **Publisher restart renumbers segments from 1 and viewers reject the new stream forever**
   - Severity: High
   - Where: `src/crates/bs-ingest/src/ffmpeg.rs:303-306`; `src/crates/bs-core/src/swarm.rs:283-285`,
     `:343`; `src/crates/bs-core/src/publisher.rs:46`
   - Repro: `bsnode publish --seed 1`, `bsnode watch`, wait 30 s, kill and restart the
     publisher. The viewer logs `ManifestRejected OutsideWindow` for every manifest, is
     evicted for unreceipted segments, rejoins, repeats. A simulator scenario with a source
     restart does not exist (`grep -ril restart src/scenarios` is empty).
   - Fix: when a viewer leaves ACTIVE with no parent, clear `played_through`, `live_edge`,
     `live_edge_at` and `anchor` so CONNECTING re-anchors from the next record or manifest;
     or persist `k` and `record_version` on the publisher. Both are the audit's F5 options;
     the spec picks none.
   - Effort: medium. Self-contained: no (spec decision F5; sibling of ISSUE-070).

3. **Guardians keep a stale Stream Record forever and reject the restarted publisher's**
   - Severity: Medium
   - Where: `src/crates/bs-core/src/node/dht.rs:73-78` (`expire` skips `record`),
     `:1143-1150` (`>=` version check), `src/crates/bs-core/src/publisher.rs:46`
   - Repro: publisher, one public viewer A, one joiner B that bootstraps from A. Restart the
     publisher. A refuses the new `STORE_RECORD` and serves B the old record (old
     `live_edge_segment`, old matrix) until A's process ends.
   - Fix: expire `store.record` after `tau_ttl` like registrations, and accept a record
     with a newer `live_edge_timestamp_us` even when `manifest_version` is lower.
   - Effort: small. Self-contained: yes.

4. **A viewer whose routing table empties never contacts the seed again**
   - Severity: Medium
   - Where: `src/crates/bs-core/src/node/dht.rs:264`, `:267`, `:1232-1243`
   - Repro: viewer joins; publisher restarts on another port (or `--advertise-ip` changes).
     Dead pings empty the table; `next_bootstrap` is `None`; the viewer stays in DISCOVERY
     with nobody to ask. Test: feed a `Node` a successful bootstrap, then time out every
     contact, and assert a `FindNode` to the seed address is emitted again.
   - Fix: re-arm `next_bootstrap` whenever `table.is_empty()` is observed in `dht_timers`,
     with backoff (2 s doubling to 30 s).
   - Effort: small. Self-contained: yes.

5. **The default proof-of-work difficulty on the real network is the simulation one**
   - Severity: Medium
   - Where: `src/crates/bs-node/src/driver.rs:132`, `:159`; `src/crates/bs-node/src/main.rs:231-239`;
     `src/crates/bs-crypto/src/pow.rs:77-84`
   - Repro: `bsnode publish` with defaults; an attacker mints an identity in 16 hashes and
     the dynamic puzzle never exceeds 4 bits. Also: `bsnode publish --spec-pow` with a
     default `bsnode watch` fails at the handshake with `identity pow` after DHT discovery
     succeeded.
   - Fix: make `Params::default()` the `RunConfig` and CLI default and add `--sim-pow`
     for the localhost tests; the desktop app follows `RunConfig`.
   - Effort: small. Self-contained: yes.

6. **Desktop identity seed has about 20 bits of entropy per second and changes every run**
   - Severity: Medium
   - Where: `src/clients/desktop/src-tauri/src/node.rs:121-123`, `:175-177`, `:301-307`
   - Repro: the seed is `SystemTime::now()` in nanoseconds (microsecond resolution on
     macOS). An attacker who knows roughly when a stream started enumerates about 10^6
     seeds per second of window (one Ed25519 keygen plus one Blake3 each) until the stream
     id matches, then holds the publisher's secret key. Separately, every publisher run has
     a new stream id, so the share link dies on restart.
   - Fix: `Identity::generate` with `OsRng` (depends on bug 1's API), then persist the key
     (missing feature below).
   - Effort: small for the RNG. Self-contained: no (depends on bug 1).

7. **A dead bootstrap never ends the viewer**
   - Severity: Low
   - Where: `src/crates/bs-node/src/driver.rs:571`, `:1056-1069`; `src/crates/bs-core/src/node.rs`
     (no 180 s DISCOVERY bound)
   - Repro: `bsnode watch --bootstrap 203.0.113.1:4000 --stream <any>`; it prints the
     5 s error and runs forever at one `FIND_NODE` every 2 s.
   - Fix: implement the Ch1 §1.3.1 bound (TERMINATED after 180 s in DISCOVERY with no
     record) in the core, and let `watch` and the desktop status show "gave up".
   - Effort: small. Self-contained: yes.

## Missing features

| Feature | Why | Size |
|---|---|---|
| Random identity by default and a persisted keypair (file under the data dir for `bsnode`, OS keychain for the desktop app), loaded on start | A broadcaster must keep one stream id across restarts; a viewer must keep its NodeID so reconnect discounts and bans mean anything. Today nothing is stored. | 2 days |
| Persisted publisher counters (`SegmentSeq`, `ManifestVersion`) or a monotonic stream clock, plus viewer re-anchor on restart | Audit F5; without it a restart is a new stream and every viewer loops (bug 2). Needs a spec decision first. | 3 days after the decision |
| Several bootstrap contacts in `RunConfig`, the CLI and the share link, tried in parallel with backoff; the link refreshed with the publisher's public relays | One home IP as the only contact is one failure point; the spec wants two seeds from different operators. With no servers, the publisher and its public relays are the operators. | 3 days |
| DNS-SRV seed resolution and a compiled-in fallback list (Ch2 §2.1.3 step 1) | Only if the product accepts operator-run seeds; "no servers" excludes it, so decide. | 2 days |
| Viewer terminal state and UI for bootstrap failure, publisher gone, stream id changed | Today the only signal is a status string after 5 s. | 1 day |
| Spec proof of work as the default with a measured cost on a phone (static 2^16, dynamic tier 8 to 14) and a progress indicator in the desktop app | Bug 5 is the default flip; the measurement on a real ARM device and the UI are new. | 1 day |

## Open questions

- Is a stream id bound to the publisher's key forever (the spec's derivation,
  `stream_id = Blake3(PK_publisher)`), so one broadcaster has one permanent link? Or
  does it include a per-broadcast nonce, so an old link cannot land on a new broadcast?
  This decides what "persist the key" has to persist.
- With no servers, who is the second seed? A public relay's address is only good while
  that relay is online. The share link would need to be re-shared, or the link needs a
  rendezvous that is not an IP.
- Does a restarted publisher continue the segment counter (needs the persisted counter)
  or restart with a re-anchor rule on the viewer? The audit's F5 lists three numberings
  and the spec picks none. ISSUE-070's widening handles only the upward case.
- Which dynamic-puzzle tier does a joiner solve before it has read `SwarmSize`
  (audit silence, `2026-11-02-tick-walkthrough.md:311`)? The implementation's answer
  decides the join cost on a phone at spec difficulty.
- ISSUE-065 (receipt keys resolved from NodeIDs) and ISSUE-085 (ingress proposals carry
  no publisher signature) both become exploitable once bug 1 is public, since the
  attacker holds the default keys. They are filed; this audit does not re-file them.
