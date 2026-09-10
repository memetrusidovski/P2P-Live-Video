//! The discrete-event loop: drives every `Node` with a virtual clock through the
//! network model, the discovery oracle and the media source.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use bs_core::{
    Channel, Command, Duration, Event, Input, Instant, Lifecycle, Node, NodeConfig, Output, Params,
    Role,
};
use bs_media::{FileSource, Source, SyntheticSource};
use bs_wire::{Frame, FrameType, NodeClass};
use bytes::Bytes;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

use crate::invariants::Invariants;
use crate::kpi::Kpis;
use crate::network::{Delivery, Network};
use crate::oracle::Oracle;
use crate::report::EventLog;
use crate::scenario::{Scenario, ScriptedEvent, ViewerGroup};

/// Node index in the simulation.
pub type NodeIdx = usize;

#[derive(Debug)]
enum Kind {
    Start(NodeIdx),
    Kill(NodeIdx),
    Deliver {
        to: NodeIdx,
        from: SocketAddr,
        channel: Channel,
        frame: Frame,
    },
    SessionOpen {
        a: NodeIdx,
        b: NodeIdx,
    },
    SessionOpenFailed {
        a: NodeIdx,
        peer: SocketAddr,
    },
    SessionClosed {
        to: NodeIdx,
        peer: SocketAddr,
    },
    Discovered {
        to: NodeIdx,
        cmd: Command,
    },
    Timer(NodeIdx),
    PublishTick,
    Scripted(usize),
    Snapshot,
    End,
}

struct Ev {
    at: Instant,
    seq: u64,
    kind: Kind,
}
impl PartialEq for Ev {
    fn eq(&self, o: &Self) -> bool {
        self.at == o.at && self.seq == o.seq
    }
}
impl Eq for Ev {}
impl PartialOrd for Ev {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Ev {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        (self.at, self.seq).cmp(&(o.at, o.seq))
    }
}

/// A simulated peer.
pub struct SimNode {
    /// The protocol state machine.
    pub node: Node,
    /// Address.
    pub addr: SocketAddr,
    /// Label from the scenario.
    pub label: String,
    /// Whether alive.
    pub alive: bool,
    /// Scheduled timer, if any.
    timer_at: Option<Instant>,
    /// Whether this node poisons blocks it forwards (adversarial scenarios).
    pub poison: bool,
    /// Start time.
    pub started_at: Option<Instant>,
}

/// The simulation.
pub struct Sim {
    /// Scenario.
    pub scenario: Scenario,
    /// Nodes (index 0 is the publisher).
    pub nodes: Vec<SimNode>,
    addr_index: HashMap<SocketAddr, NodeIdx>,
    /// Network.
    pub network: Network,
    /// Oracle.
    pub oracle: Oracle,
    queue: BinaryHeap<Reverse<Ev>>,
    seq: u64,
    /// Virtual now.
    pub now: Instant,
    rng: ChaCha8Rng,
    source: Box<dyn Source>,
    source_repeat: Option<(Vec<u8>, bs_media::Ladder)>,
    /// KPIs.
    pub kpis: Kpis,
    /// Invariant checker.
    pub invariants: Invariants,
    /// Event log.
    pub log: EventLog,
    /// Node count that ever started.
    pub started: usize,
    end: Instant,
    scripted_done: Vec<bool>,
}

fn addr_for(i: usize) -> SocketAddr {
    let i = i as u32 + 1;
    SocketAddr::new(
        IpAddr::V4(Ipv4Addr::new(
            10,
            ((i >> 16) & 0xFF) as u8,
            ((i >> 8) & 0xFF) as u8,
            (i & 0xFF) as u8,
        )),
        4000 + (i % 1000) as u16,
    )
}

impl Sim {
    /// Build from a scenario. `base` resolves relative file paths.
    pub fn new(scenario: Scenario, base: &std::path::Path, log: EventLog) -> anyhow::Result<Self> {
        let mut rng = ChaCha8Rng::seed_from_u64(scenario.seed);
        let ladder = scenario.ladder();
        type Repeat = Option<(Vec<u8>, bs_media::Ladder)>;
        let (source, repeat): (Box<dyn Source>, Repeat) = match &scenario.source {
            crate::scenario::SourceSpec::Synthetic { seed } => (
                Box::new(SyntheticSource::new(
                    ladder.clone(),
                    seed.unwrap_or(scenario.seed),
                    None,
                )),
                None,
            ),
            crate::scenario::SourceSpec::File { path, repeat } => {
                let p = if std::path::Path::new(path).is_absolute() {
                    std::path::PathBuf::from(path)
                } else {
                    base.join(path)
                };
                let data = std::fs::read(&p)
                    .map_err(|e| anyhow::anyhow!("reading source file {}: {e}", p.display()))?;
                let rep = if *repeat {
                    Some((data.clone(), ladder.clone()))
                } else {
                    None
                };
                (
                    Box::new(FileSource::new(ladder.clone(), Bytes::from(data))),
                    rep,
                )
            }
        };
        let end = Instant::ZERO + Duration::from_secs_f64(scenario.duration_s);
        let mut sim = Self {
            network: Network::new(scenario.network.clone(), scenario.seed),
            oracle: Oracle::default(),
            queue: BinaryHeap::new(),
            seq: 0,
            now: Instant::ZERO,
            rng: ChaCha8Rng::seed_from_u64(scenario.seed ^ 0xABCD),
            source,
            source_repeat: repeat,
            kpis: Kpis::new(&scenario),
            invariants: Invariants::new(scenario.params.d_max),
            log,
            started: 0,
            end,
            scripted_done: vec![false; scenario.events.len()],
            nodes: Vec::new(),
            addr_index: HashMap::new(),
            scenario,
        };
        // Publisher.
        let pub_addr = addr_for(0);
        let params = sim.scenario.params.clone();
        let publisher = Node::new(NodeConfig {
            role: Role::Publisher {
                ladder: ladder.clone(),
                forest_size: sim.scenario.forest_size,
            },
            node_class: NodeClass::Relay,
            upload_kbps: sim.scenario.publisher.upload_kbps,
            addr: pub_addr,
            stream_id: bs_wire::StreamId::ZERO,
            params: params.clone(),
            seed: sim.scenario.seed,
        });
        let stream_id = publisher.stream_id();
        sim.network.add(
            pub_addr,
            sim.scenario.publisher.region,
            sim.scenario.publisher.upload_kbps,
        );
        sim.push_node(publisher, pub_addr, "publisher".into());
        let start = Instant::ZERO + Duration::from_secs_f64(sim.scenario.publisher.start_s);
        sim.schedule(start, Kind::Start(0));
        sim.schedule(start, Kind::PublishTick);
        // Viewers.
        let groups = sim.scenario.viewers.clone();
        for g in &groups {
            sim.add_group(g, &params, stream_id, &mut rng, 0.0);
        }
        for (i, e) in sim.scenario.events.clone().iter().enumerate() {
            let at = match e {
                ScriptedEvent::KillFraction { at_s, .. }
                | ScriptedEvent::KillCount { at_s, .. }
                | ScriptedEvent::JoinBurst { at_s, .. }
                | ScriptedEvent::SetLoss { at_s, .. }
                | ScriptedEvent::Poison { at_s, .. } => *at_s,
            };
            sim.schedule(
                Instant::ZERO + Duration::from_secs_f64(at),
                Kind::Scripted(i),
            );
        }
        sim.schedule(Instant::ZERO + Duration::from_secs(1), Kind::Snapshot);
        sim.schedule(end, Kind::End);
        Ok(sim)
    }

    fn push_node(&mut self, node: Node, addr: SocketAddr, label: String) -> NodeIdx {
        let idx = self.nodes.len();
        self.addr_index.insert(addr, idx);
        self.nodes.push(SimNode {
            node,
            addr,
            label,
            alive: true,
            timer_at: None,
            poison: false,
            started_at: None,
        });
        idx
    }

    fn add_group(
        &mut self,
        g: &ViewerGroup,
        params: &Params,
        stream_id: bs_wire::StreamId,
        rng: &mut ChaCha8Rng,
        offset_s: f64,
    ) {
        for _ in 0..g.count {
            let idx = self.nodes.len();
            let addr = addr_for(idx);
            let upload = rng
                .random_range(g.upload_kbps.min..=g.upload_kbps.max)
                .round() as u32;
            let region = g
                .region
                .unwrap_or_else(|| rng.random_range(0..self.network.regions()));
            let class = if g.class.eq_ignore_ascii_case("leaf") {
                NodeClass::Leaf
            } else {
                NodeClass::Relay
            };
            let node = Node::new(NodeConfig {
                role: Role::Viewer {
                    top_layer: g.top_layer,
                },
                node_class: class,
                upload_kbps: upload,
                addr,
                stream_id,
                params: params.clone(),
                seed: self
                    .scenario
                    .seed
                    .wrapping_mul(1_000_003)
                    .wrapping_add(idx as u64),
            });
            self.network.add(addr, region, upload);
            let i = self.push_node(node, addr, g.label.clone());
            let join = offset_s + rng.random_range(g.join_s.min..=g.join_s.max);
            self.schedule(
                Instant::ZERO + Duration::from_secs_f64(join),
                Kind::Start(i),
            );
            if let Some(l) = g.leave_s {
                let leave = offset_s + rng.random_range(l.min..=l.max);
                self.schedule(
                    Instant::ZERO + Duration::from_secs_f64(leave.max(join + 0.1)),
                    Kind::Kill(i),
                );
            }
        }
    }

    fn schedule(&mut self, at: Instant, kind: Kind) {
        self.seq += 1;
        self.queue.push(Reverse(Ev {
            at,
            seq: self.seq,
            kind,
        }));
    }

    /// Run to completion. Returns `false` if an invariant halted the run.
    pub fn run(&mut self) -> bool {
        while let Some(Reverse(ev)) = self.queue.pop() {
            self.now = ev.at;
            match ev.kind {
                Kind::End => break,
                Kind::Start(i) => self.start_node(i),
                Kind::Kill(i) => self.kill_node(i, "scripted leave"),
                Kind::Deliver {
                    to,
                    from,
                    channel,
                    frame,
                } => {
                    if self.nodes[to].alive {
                        self.feed(
                            to,
                            Input::Frame {
                                from,
                                channel,
                                frame,
                            },
                        );
                    }
                }
                Kind::SessionOpen { a, b } => {
                    if self.nodes[a].alive && self.nodes[b].alive {
                        let (aa, ab) = (self.nodes[a].addr, self.nodes[b].addr);
                        self.network.open_session(aa, ab);
                        let ida = self.nodes[a].node.node_id();
                        let idb = self.nodes[b].node.node_id();
                        self.feed(
                            b,
                            Input::SessionOpened {
                                peer: aa,
                                node_id: ida,
                            },
                        );
                        self.feed(
                            a,
                            Input::SessionOpened {
                                peer: ab,
                                node_id: idb,
                            },
                        );
                    } else if self.nodes[a].alive {
                        let ab = self.nodes[b].addr;
                        self.feed(a, Input::SessionClosed { peer: ab });
                    }
                }
                Kind::SessionOpenFailed { a, peer } => {
                    if self.nodes[a].alive {
                        self.feed(a, Input::SessionClosed { peer });
                    }
                }
                Kind::SessionClosed { to, peer } => {
                    if self.nodes[to].alive {
                        self.feed(to, Input::SessionClosed { peer });
                    }
                }
                Kind::Discovered { to, cmd } => {
                    if self.nodes[to].alive {
                        self.feed(to, Input::Cmd(cmd));
                    }
                }
                Kind::Timer(i) => {
                    if self.nodes[i].alive && self.nodes[i].timer_at == Some(ev.at) {
                        self.nodes[i].timer_at = None;
                        self.feed(i, Input::Tick);
                    }
                }
                Kind::PublishTick => self.publish_tick(),
                Kind::Scripted(i) => self.scripted(i),
                Kind::Snapshot => {
                    self.snapshot();
                    let next = self.now + Duration::from_secs(1);
                    if next < self.end {
                        self.schedule(next, Kind::Snapshot);
                    }
                }
            }
            if self.invariants.halted() {
                return false;
            }
        }
        self.snapshot();
        true
    }

    fn start_node(&mut self, i: usize) {
        if !self.nodes[i].alive {
            return;
        }
        self.nodes[i].started_at = Some(self.now);
        self.started += 1;
        self.kpis
            .node_started(i, self.now, self.nodes[i].node.node_class(), i == 0);
        self.feed(i, Input::Cmd(Command::Start));
    }

    fn kill_node(&mut self, i: usize, why: &str) {
        if !self.nodes[i].alive {
            return;
        }
        self.nodes[i].alive = false;
        self.kpis.node_died(i, self.now);
        self.oracle.remove(&self.nodes[i].node.node_id());
        let addr = self.nodes[i].addr;
        let peers = self.network.kill(addr);
        let idle = self.network.idle_timeout();
        for p in peers {
            if let Some(&j) = self.addr_index.get(&p) {
                self.schedule(self.now + idle, Kind::SessionClosed { to: j, peer: addr });
            }
        }
        self.log.note(self.now, i, &format!("killed: {why}"));
    }

    fn publish_tick(&mut self) {
        let period = self.scenario.params.chunk_period;
        if !self.nodes[0].alive {
            return;
        }
        let chunk = match self.source.next_chunk() {
            Some(c) => Some(c),
            None => match &self.source_repeat {
                Some((data, ladder)) => {
                    // Loop the file, continuing segment numbering and timestamps.
                    let emitted = self.kpis.chunks_published;
                    let mut looped = FileSourceOffset {
                        inner: FileSource::new(ladder.clone(), Bytes::from(data.clone())),
                        seg_off: (emitted / 4) as u32,
                        ch_off: (emitted % 4) as u8,
                        ts_off: emitted * period.as_micros(),
                    };
                    let c = looped.next_chunk();
                    self.source = Box::new(looped);
                    c
                }
                None => None,
            },
        };
        match chunk {
            Some(c) => {
                self.kpis.publish_chunk(&c);
                self.feed(0, Input::Cmd(Command::PublishChunk(c)));
                self.schedule(self.now + period, Kind::PublishTick);
            }
            None => {
                self.feed(0, Input::Cmd(Command::EndStream));
                self.log.note(self.now, 0, "source exhausted; STREAM_END");
            }
        }
    }

    fn scripted(&mut self, i: usize) {
        if self.scripted_done[i] {
            return;
        }
        self.scripted_done[i] = true;
        let ev = self.scenario.events[i].clone();
        match ev {
            ScriptedEvent::KillFraction {
                fraction, class, ..
            } => {
                let mut cands = self.alive_viewers(&class);
                let n = ((cands.len() as f64) * fraction).round() as usize;
                rand::seq::SliceRandom::shuffle(&mut cands[..], &mut self.rng);
                for i in cands.into_iter().take(n) {
                    self.kill_node(i, "churn storm");
                }
                self.log
                    .note(self.now, 0, &format!("scripted: killed {n} ({class})"));
            }
            ScriptedEvent::KillCount { count, class, .. } => {
                let mut cands = self.alive_viewers(&class);
                rand::seq::SliceRandom::shuffle(&mut cands[..], &mut self.rng);
                for i in cands.into_iter().take(count) {
                    self.kill_node(i, "scripted kill");
                }
            }
            ScriptedEvent::JoinBurst { group, at_s } => {
                let params = self.scenario.params.clone();
                let sid = self.nodes[0].node.stream_id();
                let mut rng = ChaCha8Rng::seed_from_u64(self.scenario.seed ^ (i as u64 + 77));
                let g = ViewerGroup {
                    join_s: crate::scenario::Range {
                        min: 0.0,
                        max: group.join_s.max - group.join_s.min,
                    },
                    ..group
                };
                self.add_group(&g, &params, sid, &mut rng, at_s);
                self.log
                    .note(self.now, 0, &format!("scripted: join burst of {}", g.count));
            }
            ScriptedEvent::SetLoss { loss, .. } => {
                self.network.loss = loss;
                self.log
                    .note(self.now, 0, &format!("scripted: loss = {loss}"));
            }
            ScriptedEvent::Poison { fraction, .. } => {
                let mut cands = self.alive_viewers("relay");
                let n = ((cands.len() as f64) * fraction).round() as usize;
                rand::seq::SliceRandom::shuffle(&mut cands[..], &mut self.rng);
                for i in cands.into_iter().take(n) {
                    self.nodes[i].poison = true;
                }
                self.log.note(
                    self.now,
                    0,
                    &format!("scripted: {n} relays now poison blocks"),
                );
            }
        }
    }

    fn alive_viewers(&self, class: &str) -> Vec<usize> {
        (1..self.nodes.len())
            .filter(|&i| self.nodes[i].alive)
            .filter(|&i| match class {
                "relay" => self.nodes[i].node.node_class() == NodeClass::Relay,
                "leaf" => self.nodes[i].node.node_class() != NodeClass::Relay,
                _ => true,
            })
            .collect()
    }

    /// Feed one input and drain all outputs.
    fn feed(&mut self, i: NodeIdx, input: Input) {
        let now = self.now;
        self.nodes[i].node.handle(input, now);
        while let Some(out) = self.nodes[i].node.poll_output() {
            self.handle_output(i, out);
        }
        // Reschedule the timer.
        let next = self.nodes[i].node.next_timer();
        if let Some(t) = next {
            let t = if t < now { now } else { t };
            if self.nodes[i].timer_at.map(|cur| t < cur).unwrap_or(true) {
                self.nodes[i].timer_at = Some(t);
                self.schedule(t, Kind::Timer(i));
            }
        }
        if self.nodes[i].node.state() == Lifecycle::Terminated && self.nodes[i].alive {
            self.kpis.node_terminated(i, now);
        }
    }

    fn handle_output(&mut self, i: NodeIdx, out: Output) {
        let from = self.nodes[i].addr;
        match out {
            Output::Send {
                to,
                channel,
                frame,
                not_before,
            } => {
                let Some(&j) = self.addr_index.get(&to) else {
                    return;
                };
                let at = not_before.unwrap_or(self.now).max(self.now);
                // Invariants on what leaves a node.
                self.invariants.on_send(
                    self.now,
                    i,
                    &self.nodes[i].node,
                    &frame,
                    channel,
                    to,
                    &mut self.log,
                );
                let mut frame = frame;
                if self.nodes[i].poison {
                    if let Frame::RaptorQSymbol(s) = &mut frame {
                        let mut v = s.payload.to_vec();
                        v[0] ^= 0xFF;
                        s.payload = Bytes::from(v);
                    }
                }
                let ty = frame.frame_type();
                let is_media = matches!(
                    ty,
                    FrameType::RAPTORQ_SYMBOL | FrameType::BLOCK_TRANSMISSION
                );
                let size = frame.payload_len() + 4;
                self.kpis.bytes(ty, size);
                // Session-bound channels need a session.
                if !matches!(channel, Channel::Udp) && !self.network.has_session(from, to) {
                    self.kpis.no_session_drops += 1;
                    return;
                }
                match self
                    .network
                    .send(&mut self.rng, at, from, to, channel, size, is_media)
                {
                    Delivery::At(t) => self.schedule(
                        t,
                        Kind::Deliver {
                            to: j,
                            from,
                            channel,
                            frame,
                        },
                    ),
                    Delivery::Dropped => {}
                }
            }
            Output::OpenSession(peer) => {
                let Some(&j) = self.addr_index.get(&peer) else {
                    return;
                };
                if self.network.alive(&peer) {
                    let d = self.network.session_open_delay(&from, &peer);
                    self.schedule(self.now + d, Kind::SessionOpen { a: i, b: j });
                } else {
                    let d = self.network.idle_timeout();
                    self.schedule(self.now + d, Kind::SessionOpenFailed { a: i, peer });
                }
            }
            Output::CloseSession(peer) => {
                let pj = self.addr_index.get(&peer).copied();
                self.log
                    .note(self.now, i, &format!("close session to node {:?}", pj));
                if self.network.close_session(from, peer) {
                    if let Some(&j) = self.addr_index.get(&peer) {
                        let d = self.network.one_way_latency(&from, &peer);
                        self.schedule(self.now + d, Kind::SessionClosed { to: j, peer: from });
                    }
                }
            }
            Output::Discover { wanted_trees, .. } => {
                let who = self.nodes[i].node.node_id();
                let records = self.oracle.get_peers(&mut self.rng, &who, wanted_trees);
                let sr = self.oracle.stream_record.clone();
                let d = self.network.discovery_rtt();
                self.schedule(
                    self.now + d,
                    Kind::Discovered {
                        to: i,
                        cmd: Command::Discovered {
                            records,
                            stream_record: sr,
                        },
                    },
                );
            }
            Output::Register(rec) => self.oracle.register(rec),
            Output::StoreRecord(sr) => self.oracle.store(sr),
            Output::Deliver {
                chunk,
                layers_complete,
                layers_subscribed,
            } => {
                self.kpis.delivered(
                    i,
                    self.now,
                    &chunk,
                    layers_complete,
                    layers_subscribed,
                    &mut self.invariants,
                    &mut self.log,
                );
            }
            Output::Event(e) => {
                self.kpis.event(i, self.now, &e);
                self.invariants.on_event(self.now, i, &e, &mut self.log);
                if self.scenario.log_all_events
                    || !matches!(e, Event::BlockVerified { .. } | Event::Heartbeat { .. })
                {
                    self.log
                        .event(self.now, i, &self.nodes[i].node.node_id(), &e);
                }
            }
        }
    }

    fn snapshot(&mut self) {
        let alive: Vec<&SimNode> = self.nodes.iter().filter(|n| n.alive).collect();
        self.kpis.snapshot(self.now, &alive);
        self.invariants
            .snapshot(self.now, &self.nodes, &self.addr_index, &mut self.log);
    }
}

/// A file source whose numbering continues after a loop.
struct FileSourceOffset {
    inner: FileSource,
    seg_off: u32,
    ch_off: u8,
    ts_off: u64,
}
impl Source for FileSourceOffset {
    fn ladder(&self) -> &bs_media::Ladder {
        self.inner.ladder()
    }
    fn next_chunk(&mut self) -> Option<bs_media::LayeredChunk> {
        let mut c = self.inner.next_chunk()?;
        let idx = c.chunk_index + self.ch_off;
        c.segment = bs_wire::SegmentSeq(c.segment.0 + self.seg_off + (idx / 4) as u32);
        c.chunk_index = idx % 4;
        c.timestamp_us += self.ts_off;
        Some(c)
    }
    fn layer_hashes(&self) -> Vec<bs_wire::Hash> {
        self.inner.layer_hashes()
    }
}
