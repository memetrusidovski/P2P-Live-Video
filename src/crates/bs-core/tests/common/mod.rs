//! In-test harness: several `Node`s, a virtual clock, fixed-latency frame
//! delivery, an in-test discovery oracle and a synthetic publisher feed. No
//! network model; every output is traced per node for ordering assertions.

#![allow(dead_code)]

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use bs_core::{
    Channel, Command, Duration, Event, Input, Instant, Node, NodeConfig, Output, Params, Role,
};
use bs_crypto::pow::Difficulty;
use bs_crypto::Identity;
use bs_media::source::Source;
use bs_media::{Ladder, LayeredChunk, SyntheticSource};
use bs_wire::{
    BlockIndex, Frame, NodeClass, NodeId, PeerRecord, SegmentSeq, StreamId, StreamRecord,
};

/// Fixed one-way latency between nodes.
pub const LATENCY: Duration = Duration::from_millis(10);
/// Session handshake time.
pub const SESSION_OPEN: Duration = Duration::from_millis(20);
/// Discovery round trip.
pub const DISCOVERY_RTT: Duration = Duration::from_millis(10);
/// Chunk period.
pub const CHUNK: Duration = Duration::from_millis(250);

/// Address for a node index.
pub fn addr(i: u16) -> SocketAddr {
    SocketAddr::new(
        IpAddr::V4(Ipv4Addr::new(10, 0, 0, (i as u8).max(1))),
        5000 + i,
    )
}

/// Reproduce `Node::new`'s identity derivation so tests can sign as a node.
pub fn identity_for(seed: u64, a: SocketAddr, params: &Params) -> Identity {
    let mut s = [0u8; 32];
    s[..8].copy_from_slice(&seed.to_le_bytes());
    s[8..16].copy_from_slice(&a.port().to_le_bytes()[..].repeat(4)[..8]);
    Identity::from_seed(
        s,
        Difficulty {
            c1: params.c1,
            c2_min: params.c2_min,
        },
    )
}

/// Find a seed whose rendezvous assignment over `m` trees is `tree`.
pub fn seed_for_tree(a: SocketAddr, params: &Params, m: u8, tree: u8) -> u64 {
    for seed in 1..10_000u64 {
        let id = identity_for(seed, a, params).node_id();
        if bs_crypto::rendezvous::assigned_tree(&id, m).0 == tree {
            return seed;
        }
    }
    panic!("no seed found for tree {tree}");
}

/// One traced output.
#[derive(Debug, Clone)]
pub enum TraceItem {
    Event(Event),
    Send {
        to: SocketAddr,
        channel: Channel,
        frame: Frame,
        not_before: Option<Instant>,
    },
    Deliver {
        chunk: LayeredChunk,
        layers_complete: u8,
        layers_subscribed: u8,
    },
    OpenSession(SocketAddr),
    CloseSession(SocketAddr),
    Discover,
    Register(PeerRecord),
    StoreRecord,
}

#[derive(Debug)]
enum Pending {
    Frame {
        to: SocketAddr,
        from: SocketAddr,
        channel: Channel,
        frame: Frame,
    },
    SessionOpen {
        a: SocketAddr,
        b: SocketAddr,
    },
    SessionClosed {
        to: SocketAddr,
        peer: SocketAddr,
    },
    Discovered {
        to: SocketAddr,
        cmd: Command,
    },
}

struct Ev {
    at: Instant,
    seq: u64,
    kind: Pending,
}
impl PartialEq for Ev {
    fn eq(&self, o: &Self) -> bool {
        (self.at, self.seq) == (o.at, o.seq)
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

/// The harness.
pub struct Harness {
    pub now: Instant,
    pub params: Params,
    pub ladder: Ladder,
    nodes: HashMap<SocketAddr, Node>,
    order: Vec<SocketAddr>,
    heap: BinaryHeap<Reverse<Ev>>,
    seq: u64,
    fifo: HashMap<(SocketAddr, SocketAddr, u8), Instant>,
    records: HashMap<NodeId, PeerRecord>,
    stream_record: Option<StreamRecord>,
    pub traces: HashMap<SocketAddr, Vec<(Instant, TraceItem)>>,
    /// Frames whose sender is in this set are silently dropped.
    pub drop_from: HashSet<SocketAddr>,
    /// Corrupt source symbol 0 of this block in flight.
    pub poison: Option<(SegmentSeq, BlockIndex)>,
    /// Corrupt source symbol 0 of every block sent by this node (a poisoning relay).
    pub poison_all_from: Option<SocketAddr>,
    source: Option<SyntheticSource>,
    pub published: Vec<LayeredChunk>,
    publisher: Option<SocketAddr>,
    next_publish: Option<Instant>,
}

impl Default for Harness {
    fn default() -> Self {
        Self::new()
    }
}

impl Harness {
    pub fn new() -> Self {
        Self {
            now: Instant::ZERO,
            params: Params::for_simulation(),
            ladder: Ladder::reference(),
            nodes: HashMap::new(),
            order: Vec::new(),
            heap: BinaryHeap::new(),
            seq: 0,
            fifo: HashMap::new(),
            records: HashMap::new(),
            stream_record: None,
            traces: HashMap::new(),
            drop_from: HashSet::new(),
            poison: None,
            poison_all_from: None,
            source: None,
            published: Vec::new(),
            publisher: None,
            next_publish: None,
        }
    }

    fn schedule(&mut self, at: Instant, kind: Pending) {
        self.seq += 1;
        self.heap.push(Reverse(Ev {
            at,
            seq: self.seq,
            kind,
        }));
    }

    /// Add a publisher (forest size 3) with `upload_kbps`.
    pub fn add_publisher(&mut self, a: SocketAddr, upload_kbps: u32, seed: u64) -> StreamId {
        let node = Node::new(NodeConfig {
            role: Role::Publisher {
                ladder: self.ladder.clone(),
                forest_size: 3,
            },
            node_class: NodeClass::Relay,
            upload_kbps,
            addr: a,
            stream_id: StreamId::ZERO,
            params: self.params.clone(),
            seed,
        });
        let sid = node.stream_id();
        self.nodes.insert(a, node);
        self.order.push(a);
        self.publisher = Some(a);
        self.source = Some(SyntheticSource::new(self.ladder.clone(), seed, None));
        sid
    }

    /// Add a viewer.
    pub fn add_viewer(
        &mut self,
        a: SocketAddr,
        class: NodeClass,
        top_layer: u8,
        upload_kbps: u32,
        seed: u64,
    ) {
        let sid = self
            .node(self.publisher.expect("publisher first"))
            .stream_id();
        let node = Node::new(NodeConfig {
            role: Role::Viewer { top_layer },
            node_class: class,
            upload_kbps,
            addr: a,
            stream_id: sid,
            params: self.params.clone(),
            seed,
        });
        self.nodes.insert(a, node);
        self.order.push(a);
    }

    pub fn node(&self, a: SocketAddr) -> &Node {
        self.nodes.get(&a).expect("unknown node")
    }

    /// Start a node now.
    pub fn start(&mut self, a: SocketAddr) {
        self.feed(a, Input::Cmd(Command::Start));
    }

    /// Begin feeding the publisher one chunk every 250 ms, starting now.
    pub fn start_publishing(&mut self) {
        self.next_publish = Some(self.now);
    }

    /// Inject a frame into `to` as if sent by `from`, immediately.
    pub fn inject(&mut self, to: SocketAddr, from: SocketAddr, channel: Channel, frame: Frame) {
        self.feed(
            to,
            Input::Frame {
                from,
                channel,
                frame,
            },
        );
    }

    /// Feed one input and drain outputs.
    pub fn feed(&mut self, a: SocketAddr, input: Input) {
        let now = self.now;
        let Some(node) = self.nodes.get_mut(&a) else {
            return;
        };
        node.handle(input, now);
        let mut outs = Vec::new();
        while let Some(o) = node.poll_output() {
            outs.push(o);
        }
        for o in outs {
            self.handle_output(a, o);
        }
    }

    fn trace(&mut self, a: SocketAddr, item: TraceItem) {
        let now = self.now;
        self.traces.entry(a).or_default().push((now, item));
    }

    fn handle_output(&mut self, from: SocketAddr, out: Output) {
        match out {
            Output::Send {
                to,
                channel,
                frame,
                not_before,
            } => {
                self.trace(
                    from,
                    TraceItem::Send {
                        to,
                        channel,
                        frame: frame.clone(),
                        not_before,
                    },
                );
                if self.drop_from.contains(&from) || !self.nodes.contains_key(&to) {
                    return;
                }
                let mut frame = frame;
                if let Frame::RaptorQSymbol(s) = &mut frame {
                    let targeted = self
                        .poison
                        .is_some_and(|(seg, blk)| s.segment == seg && s.block == blk);
                    let all = self.poison_all_from == Some(from);
                    if (targeted || all) && s.esi == 0 {
                        let mut v = s.payload.to_vec();
                        v[0] ^= 0xFF;
                        s.payload = v.into();
                    }
                }
                let base = not_before.map(|t| t.max(self.now)).unwrap_or(self.now);
                let mut at = base + LATENCY;
                let code = match channel {
                    Channel::Udp => 0,
                    Channel::Control => 1,
                    Channel::Datagram => 2,
                    Channel::Tree(t) => 16 + t.0,
                };
                let tail = self.fifo.entry((from, to, code)).or_insert(Instant::ZERO);
                if *tail > at {
                    at = *tail;
                }
                *tail = at;
                self.schedule(
                    at,
                    Pending::Frame {
                        to,
                        from,
                        channel,
                        frame,
                    },
                );
            }
            Output::OpenSession(peer) => {
                self.trace(from, TraceItem::OpenSession(peer));
                self.schedule(
                    self.now + SESSION_OPEN,
                    Pending::SessionOpen { a: from, b: peer },
                );
            }
            Output::CloseSession(peer) => {
                self.trace(from, TraceItem::CloseSession(peer));
                self.schedule(
                    self.now + LATENCY,
                    Pending::SessionClosed {
                        to: peer,
                        peer: from,
                    },
                );
            }
            Output::Discover { wanted_trees, .. } => {
                self.trace(from, TraceItem::Discover);
                let who = self.node(from).node_id();
                let mut records: Vec<PeerRecord> = self
                    .records
                    .values()
                    .filter(|r| r.node_id != who)
                    .filter(|r| {
                        wanted_trees.is_empty()
                            || (r.node_class == NodeClass::Relay
                                && !r.assigned_trees.intersection(wanted_trees).is_empty())
                    })
                    .copied()
                    .collect();
                records.sort_by_key(|r| r.node_id);
                let cmd = Command::Discovered {
                    records,
                    stream_record: self.stream_record.clone(),
                };
                self.schedule(
                    self.now + DISCOVERY_RTT,
                    Pending::Discovered { to: from, cmd },
                );
            }
            Output::Register(r) => {
                self.trace(from, TraceItem::Register(r));
                self.records.insert(r.node_id, r);
            }
            Output::StoreRecord(sr) => {
                self.trace(from, TraceItem::StoreRecord);
                self.stream_record = Some(sr);
            }
            Output::Deliver {
                chunk,
                layers_complete,
                layers_subscribed,
            } => {
                self.trace(
                    from,
                    TraceItem::Deliver {
                        chunk,
                        layers_complete,
                        layers_subscribed,
                    },
                );
            }
            Output::Event(e) => self.trace(from, TraceItem::Event(e)),
        }
    }

    fn publish_tick(&mut self) {
        let Some(p) = self.publisher else { return };
        let chunk = self
            .source
            .as_mut()
            .and_then(|s| s.next_chunk())
            .expect("synthetic source is infinite");
        self.published.push(chunk.clone());
        self.feed(p, Input::Cmd(Command::PublishChunk(chunk)));
        self.next_publish = Some(self.now + CHUNK);
    }

    /// Advance virtual time to `t`, firing deliveries, timers and publish ticks in order.
    pub fn run_until(&mut self, t: Instant) {
        loop {
            let mut next: Option<(Instant, u8)> = None; // (time, kind: 0 heap, 1 publish, 2 timer)
            if let Some(Reverse(e)) = self.heap.peek() {
                next = Some((e.at, 0));
            }
            if let Some(p) = self.next_publish {
                if next.map(|(x, _)| p < x).unwrap_or(true) {
                    next = Some((p, 1));
                }
            }
            let mut timer_node = None;
            for a in &self.order {
                if let Some(tt) = self.nodes[a].next_timer() {
                    if next.map(|(x, _)| tt < x).unwrap_or(true) {
                        next = Some((tt, 2));
                        timer_node = Some(*a);
                    }
                }
            }
            let Some((at, kind)) = next else {
                self.now = t;
                return;
            };
            if at > t {
                self.now = t;
                return;
            }
            self.now = at.max(self.now);
            match kind {
                0 => {
                    let Reverse(ev) = self.heap.pop().unwrap();
                    match ev.kind {
                        Pending::Frame {
                            to,
                            from,
                            channel,
                            frame,
                        } => self.feed(
                            to,
                            Input::Frame {
                                from,
                                channel,
                                frame,
                            },
                        ),
                        Pending::SessionOpen { a, b } => {
                            if self.nodes.contains_key(&a) && self.nodes.contains_key(&b) {
                                let ida = self.node(a).node_id();
                                let idb = self.node(b).node_id();
                                self.feed(
                                    b,
                                    Input::SessionOpened {
                                        peer: a,
                                        node_id: ida,
                                    },
                                );
                                self.feed(
                                    a,
                                    Input::SessionOpened {
                                        peer: b,
                                        node_id: idb,
                                    },
                                );
                            }
                        }
                        Pending::SessionClosed { to, peer } => {
                            self.feed(to, Input::SessionClosed { peer })
                        }
                        Pending::Discovered { to, cmd } => self.feed(to, Input::Cmd(cmd)),
                    }
                }
                1 => self.publish_tick(),
                _ => {
                    let a = timer_node.unwrap();
                    self.feed(a, Input::Tick);
                }
            }
        }
    }

    /// Run for `d` more virtual time.
    pub fn run_for(&mut self, d: Duration) {
        let t = self.now + d;
        self.run_until(t);
    }

    /// Events of a node in order.
    pub fn events(&self, a: SocketAddr) -> Vec<(Instant, Event)> {
        self.traces
            .get(&a)
            .map(|v| {
                v.iter()
                    .filter_map(|(t, i)| {
                        if let TraceItem::Event(e) = i {
                            Some((*t, e.clone()))
                        } else {
                            None
                        }
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
    /// Sends of a node in order.
    pub fn sends(
        &self,
        a: SocketAddr,
    ) -> Vec<(Instant, SocketAddr, Channel, Frame, Option<Instant>)> {
        self.traces
            .get(&a)
            .map(|v| {
                v.iter()
                    .filter_map(|(t, i)| match i {
                        TraceItem::Send {
                            to,
                            channel,
                            frame,
                            not_before,
                        } => Some((*t, *to, *channel, frame.clone(), *not_before)),
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
    /// Deliveries of a node in order.
    pub fn delivers(&self, a: SocketAddr) -> Vec<(Instant, LayeredChunk, u8, u8)> {
        self.traces
            .get(&a)
            .map(|v| {
                v.iter()
                    .filter_map(|(t, i)| match i {
                        TraceItem::Deliver {
                            chunk,
                            layers_complete,
                            layers_subscribed,
                        } => Some((*t, chunk.clone(), *layers_complete, *layers_subscribed)),
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
    /// Full trace of a node.
    pub fn items(&self, a: SocketAddr) -> &[(Instant, TraceItem)] {
        self.traces.get(&a).map(|v| v.as_slice()).unwrap_or(&[])
    }
    /// Index in the trace of the first item matching `f`, searching from `from`.
    pub fn find_from(
        &self,
        a: SocketAddr,
        from: usize,
        f: impl Fn(&TraceItem) -> bool,
    ) -> Option<usize> {
        self.items(a)
            .iter()
            .enumerate()
            .skip(from)
            .find(|(_, (_, i))| f(i))
            .map(|(k, _)| k)
    }
    /// Count of events matching a predicate.
    pub fn count_events(&self, a: SocketAddr, f: impl Fn(&Event) -> bool) -> usize {
        self.events(a).iter().filter(|(_, e)| f(e)).count()
    }
}

/// Publisher + one relay (assigned to tree 1) + one leaf under the relay, all
/// subscribing to layer 0 only. The publisher has exactly one slot in tree 1, so
/// the leaf can only attach under the relay. Returns (pub, relay, leaf).
pub fn chain_topology(h: &mut Harness) -> (SocketAddr, SocketAddr, SocketAddr) {
    let p = addr(1);
    let r = addr(2);
    let l = addr(3);
    h.add_publisher(p, 8000, 1); // K_v = (1, 1, 0)
    let seed = seed_for_tree(r, &h.params.clone(), 3, 1);
    h.add_viewer(r, NodeClass::Relay, 0, 10_000, seed);
    h.add_viewer(l, NodeClass::Leaf, 0, 1000, 99);
    h.start(p);
    h.start_publishing();
    h.run_for(Duration::from_millis(500));
    h.start(r);
    h.run_for(Duration::from_secs(3)); // relay attaches and warms up
    h.start(l);
    h.run_for(Duration::from_secs(2));
    (p, r, l)
}
