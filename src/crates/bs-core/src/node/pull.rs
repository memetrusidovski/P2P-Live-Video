//! The PULL zone (Ch4 §4.3): bitfield gossip, the deadline-aware rarest-first
//! scheduler, pull responses, and late-join backfill.
//!
//! Until HyParView lands, the "Active Set" a node pulls from is every peer it
//! holds a session with for tree duty: parents, children and a draining parent.

use rand::Rng;
use std::collections::{BTreeMap, BTreeSet};

use bs_media::fec::{repair_esi_base, FecEncoder};
use bs_wire::consts::K_BLOCK;
use bs_wire::frames::{
    BlockTransmission, BufferStateBitfield, ManifestRequest, ManifestSelector, PullRequest,
};
use bs_wire::{Frame, SegmentSeq};

use super::Node;
use crate::io::{Channel, Event, PeerAddr};
use crate::time::{Duration, Instant};

/// Per-node pull state.
#[derive(Debug, Default)]
pub struct PullState {
    /// Latest bitfield per neighbour per (segment, chunk).
    pub bitfields: BTreeMap<PeerAddr, BTreeMap<(u32, u8), BufferStateBitfield>>,
    /// Outstanding requests by (segment, block).
    pub pending: BTreeMap<(u32, u16), PendingPull>,
    /// Last broadcast want per block.
    pub broadcast_at: BTreeMap<(u32, u16), Instant>,
    /// Next bitfield gossip.
    pub next_gossip: Instant,
    /// Next scheduler pass.
    pub next_sched: Instant,
    /// PULL service budget: bytes available now, refilled at `r_pull · upload`.
    pub budget_bytes: f64,
    /// Last refill.
    pub budget_at: Instant,
    /// Counters for KPIs.
    pub pulls_sent: u64,
    /// Requests answered.
    pub pulls_served: u64,
    /// Blocks completed through the pull path.
    pub blocks_pulled: u64,
    /// Requests refused for budget.
    pub pulls_refused: u64,
    /// Requests awaiting service, served each τ_sched in urgency order within the budget.
    pub serve_queue: Vec<QueuedPull>,
}

/// An outstanding PULL_REQUEST of ours.
#[derive(Debug, Clone)]
pub struct PendingPull {
    /// Last request time.
    pub at: Instant,
    /// Requests sent for this block so far.
    pub attempts: u8,
    /// Providers already asked (each is asked once per block).
    pub tried: Vec<PeerAddr>,
}

/// A PULL_REQUEST waiting for service.
#[derive(Debug, Clone)]
pub struct QueuedPull {
    /// Requester.
    pub from: PeerAddr,
    /// Its playout deadline for the block (our clock).
    pub deadline: Instant,
    /// The request.
    pub req: PullRequest,
}

impl Node {
    /// Peers we exchange bitfields and pulls with: tree peers plus the HyParView Active Set.
    pub(super) fn active_peers(&self) -> BTreeSet<PeerAddr> {
        let mut s = BTreeSet::new();
        for t in &self.trees {
            if let Some(p) = &t.parent {
                s.insert(p.addr);
            }
            if let Some(p) = &t.old_parent {
                s.insert(p.addr);
            }
            for c in t.children.values() {
                if !c.pending {
                    s.insert(c.addr);
                }
            }
        }
        for a in self.membership.active.keys() {
            s.insert(*a);
        }
        s
    }

    /// Earliest pull timer.
    pub(super) fn pull_next_timer(&self) -> Option<Instant> {
        self.matrix.as_ref()?;
        if self.is_source() && self.pull.serve_queue.is_empty() {
            return Some(self.pull.next_gossip);
        }
        Some(self.pull.next_gossip.min(self.pull.next_sched))
    }

    /// Run due pull timers.
    pub(super) fn pull_timers(&mut self) {
        let now = self.now;
        if self.matrix.is_none() {
            return;
        }
        if now >= self.pull.next_gossip {
            self.pull.next_gossip = now + self.params.tau_gossip;
            self.gossip_bitfields();
        }
        if now >= self.pull.next_sched {
            self.pull.next_sched = now + self.params.tau_sched;
            self.serve_pull_queue();
            if !self.is_source() {
                self.schedule_pulls();
            }
        }
    }

    /// Advertise availability of the last three segments to every active peer.
    fn gossip_bitfields(&mut self) {
        let peers = self.active_peers();
        if peers.is_empty() {
            return;
        }
        let chunks = self.swarm.recent_chunks(3);
        let mut frames = Vec::with_capacity(chunks.len());
        for (seg, c) in chunks {
            if let Some(bf) = self.swarm.bitfield(seg, c) {
                frames.push(Frame::BufferStateBitfield(bf));
            }
        }
        for p in peers {
            for f in &frames {
                self.send(p, Channel::Control, f.clone(), None);
            }
        }
    }

    /// A peer that served us a retained manifest holds that chunk's blocks: record a
    /// full bitfield so backfill pulls go to it directly instead of by broadcast.
    pub(super) fn note_implicit_provider(
        &mut self,
        from: PeerAddr,
        m: &bs_wire::frames::ManifestBody,
    ) {
        let n = m.block_count() as usize;
        let Ok(start) = bs_wire::BlockIndex::new(m.chunk_index, 0) else {
            return;
        };
        let bf = BufferStateBitfield::from_flags(m.segment, start, &vec![true; n]);
        self.pull
            .bitfields
            .entry(from)
            .or_default()
            .insert((m.segment.0, m.chunk_index), bf);
    }

    /// A neighbour's bitfield.
    pub(super) fn on_bitfield(&mut self, from: PeerAddr, bf: BufferStateBitfield) {
        let key = (bf.segment.0, bf.start.chunk());
        let per = self.pull.bitfields.entry(from).or_default();
        per.insert(key, bf);
        // Forget old segments.
        let floor = self.swarm.live_edge.0.saturating_sub(12);
        per.retain(|(s, _), _| *s >= floor);
    }

    /// PULL window `W_pull` for the current parent RTTs (Ch4 §4.3.1).
    fn w_pull(&self) -> Duration {
        let mut srtt_max = Duration::ZERO;
        let mut evict_max = self.params.tau_evict_floor;
        for t in &self.trees {
            if let Some(p) = &t.parent {
                srtt_max = srtt_max.max(p.srtt);
                evict_max = evict_max.max(self.params.tau_evict(p.srtt, p.rttvar));
            }
        }
        self.params.w_pull(evict_max, srtt_max)
    }

    /// The scheduler of Ch4 §4.3.3: one pass over the missing blocks in the PULL zone.
    fn schedule_pulls(&mut self) {
        let now = self.now;
        let window = self.w_pull();
        let missing = self.swarm.missing_in_window(now, window);
        if missing.is_empty() {
            return;
        }
        let peers = self.active_peers();
        const RETRY_MS: u64 = 300;
        let mut sent_this_pass = 0usize;
        for m in missing {
            if sent_this_pass >= 32 {
                break;
            }
            // Only blocks of trees this node is attached to (or repairing, or serving
            // children in). A shed layer's blocks are missing by design, and a tree
            // still in its initial join is not the PULL zone's job: pulling a whole
            // stripe while waiting for a parent turns repair into a second
            // distribution path and saturates the provider (Ch4 §4.3.3).
            let wanted = self
                .trees
                .get(m.tree.index())
                .map(|t| {
                    t.parent.is_some()
                        || t.old_parent.is_some()
                        || t.lost_at.is_some()
                        || (t.assigned && !t.children.is_empty())
                })
                .unwrap_or(false);
            if !wanted {
                continue;
            }
            let key = (m.segment.0, m.index.0);
            // Retry with backoff (300, 600, then 1200 ms): a provider that has not
            // answered is budget-bound, and re-asking every 300 ms only fills its queue
            // with requests that expire (Ch4 §4.3.3 registers one pending request per
            // block, not a stream of them). Retries never stop while the block is in
            // the zone: providers serve in urgency order, so the request nearest its
            // deadline is the one they answer.
            let attempts = self.pull.pending.get(&key).map(|x| x.attempts).unwrap_or(0);
            if let Some(pp) = self.pull.pending.get(&key) {
                let wait = Duration::from_millis(RETRY_MS << pp.attempts.saturating_sub(1).min(2));
                if now.duration_since(pp.at) < wait {
                    continue;
                }
            }
            let urgency_ms = m
                .deadline
                .duration_since(now)
                .as_millis()
                .min(u16::MAX as u64) as u16;
            let missing_symbols = if m.symbols_held >= K_BLOCK / 2 {
                K_BLOCK.saturating_sub(m.symbols_held).max(1) as u8
            } else {
                0
            };
            // Providers whose bitfield shows the block, best RTT first, excluding every
            // provider already asked for this block (it is budget-bound or wrong).
            let mut tried: Vec<PeerAddr> = self
                .pull
                .pending
                .get(&key)
                .map(|x| x.tried.clone())
                .unwrap_or_default();
            let holders: Vec<PeerAddr> = peers
                .iter()
                .filter(|p| {
                    self.pull
                        .bitfields
                        .get(p)
                        .and_then(|b| b.get(&(m.segment.0, m.index.chunk())))
                        .map(|bf| bf.has(m.index))
                        .unwrap_or(false)
                })
                .copied()
                .collect();
            if !holders.is_empty() && holders.iter().all(|p| tried.contains(p)) {
                // Every holder has been asked once: start another round.
                tried.clear();
            }
            let mut providers: Vec<(Duration, PeerAddr)> = holders
                .iter()
                .filter(|p| !tried.contains(p))
                .map(|p| (self.peer_srtt(*p), *p))
                .collect();
            providers.sort();
            // Spread load: any provider within 20 ms of the best RTT is as good.
            let chosen = providers.first().map(|(best, _)| {
                let near: Vec<PeerAddr> = providers
                    .iter()
                    .filter(|(rtt, _)| *rtt <= *best + Duration::from_millis(20))
                    .map(|(_, p)| *p)
                    .collect();
                near[self.rng.random_range(0..near.len())]
            });
            match chosen {
                Some(provider) => {
                    let req = PullRequest {
                        segment: m.segment,
                        block: m.index,
                        missing_symbols,
                        urgency_ms,
                        broadcast_want: false,
                    };
                    self.send(provider, Channel::Control, Frame::PullRequest(req), None);
                    tried.push(provider);
                    self.pull.pending.insert(
                        key,
                        PendingPull {
                            at: now,
                            attempts: attempts + 1,
                            tried,
                        },
                    );
                    self.pull.pulls_sent += 1;
                    sent_this_pass += 1;
                    self.emit(Event::PullSent {
                        segment: m.segment.0,
                        block: m.index.0,
                        urgency_ms,
                        broadcast: false,
                    });
                }
                None => {
                    // Rarity signal: the spec bounds it to one broadcast per block per τ_sched;
                    // until HyParView supplies bitfields from more neighbours we use 5·τ_sched.
                    if self
                        .pull
                        .broadcast_at
                        .get(&key)
                        .map(|t| now.duration_since(*t) < self.params.tau_sched * 5)
                        .unwrap_or(false)
                    {
                        continue;
                    }
                    if peers.is_empty() {
                        continue;
                    }
                    let req = PullRequest {
                        segment: m.segment,
                        block: m.index,
                        missing_symbols: 0,
                        urgency_ms,
                        broadcast_want: true,
                    };
                    for p in &peers {
                        self.send(*p, Channel::Control, Frame::PullRequest(req.clone()), None);
                    }
                    self.pull.broadcast_at.insert(key, now);
                    // A broadcast asks everyone once; a later directed retry may ask any of
                    // them again once their bitfields show the block.
                    self.pull.pending.insert(
                        key,
                        PendingPull {
                            at: now,
                            attempts: attempts + 1,
                            tried,
                        },
                    );
                    self.pull.pulls_sent += 1;
                    sent_this_pass += 1;
                    self.emit(Event::PullSent {
                        segment: m.segment.0,
                        block: m.index.0,
                        urgency_ms,
                        broadcast: true,
                    });
                }
            }
        }
        // Drop bookkeeping for blocks that are verified or past.
        let floor = self.swarm.live_edge.0.saturating_sub(12);
        self.pull.pending.retain(|(s, _), _| *s >= floor);
        self.pull.broadcast_at.retain(|(s, _), _| *s >= floor);
    }

    /// Smoothed RTT toward a peer (parents measure it; others get a default).
    fn peer_srtt(&self, p: PeerAddr) -> Duration {
        for t in &self.trees {
            if let Some(pl) = &t.parent {
                if pl.addr == p {
                    return pl.srtt;
                }
            }
        }
        Duration::from_millis(60)
    }

    /// Refill and spend the PULL service budget (`r_pull` of the uplink).
    fn pull_budget_take(&mut self, bytes: usize) -> bool {
        let now = self.now;
        let rate = self.params.r_pull * self.cfg.upload_kbps as f64 * 1000.0 / 8.0; // bytes/s
        let elapsed = now.duration_since(self.pull.budget_at).as_secs_f64();
        self.pull.budget_at = now;
        // Pace, do not burst: pull responses share the uplink with the push path, and
        // a burst of blocks delays every child's symbols behind it. Allow at most
        // 100 ms of reserve (never less than one block) to accumulate.
        let burst = (0.1 * rate).max(bs_wire::consts::BLOCK_SIZE as f64 + 256.0);
        self.pull.budget_bytes = (self.pull.budget_bytes + elapsed * rate).min(burst);
        if self.pull.budget_bytes >= bytes as f64 {
            self.pull.budget_bytes -= bytes as f64;
            true
        } else {
            false
        }
    }

    /// Queue a `PULL_REQUEST`; the scheduler serves the most urgent first (Ch4 §4.3.3).
    pub(super) fn on_pull_request(&mut self, from: PeerAddr, r: PullRequest) {
        if self.swarm.block_with_proof(r.segment, r.block).is_none() {
            // A broadcast want we cannot serve stays silent; a directed request we
            // cannot serve means the requester's view of our buffer was wrong.
            if !r.broadcast_want {
                self.emit(Event::PullUnheld {
                    segment: r.segment.0,
                    block: r.block.0,
                });
            }
            return;
        }
        let deadline = self.now + Duration::from_millis(r.urgency_ms as u64);
        if let Some(q) = self
            .pull
            .serve_queue
            .iter_mut()
            .find(|q| q.from == from && q.req.segment == r.segment && q.req.block == r.block)
        {
            q.deadline = deadline;
            q.req = r;
            return;
        }
        if self.pull.serve_queue.len() >= 512 {
            self.pull.pulls_refused += 1;
            self.emit(Event::PullRefused {
                segment: r.segment.0,
                block: r.block.0,
            });
            return;
        }
        self.pull.serve_queue.push(QueuedPull {
            from,
            deadline,
            req: r,
        });
        // Serve immediately if the budget allows; otherwise the tick will.
        self.serve_pull_queue();
    }

    /// Serve queued requests in deadline order while the PULL budget lasts; drop
    /// requests whose deadline has passed (the requester has moved on).
    fn serve_pull_queue(&mut self) {
        if self.pull.serve_queue.is_empty() {
            return;
        }
        let now = self.now;
        let mut expired = Vec::new();
        self.pull.serve_queue.retain(|q| {
            let keep = q.deadline > now;
            if !keep {
                expired.push((q.req.segment.0, q.req.block.0));
            }
            keep
        });
        self.pull.pulls_refused += expired.len() as u64;
        for (segment, block) in expired {
            self.emit(Event::PullRefused { segment, block });
        }
        self.pull.serve_queue.sort_by_key(|q| q.deadline);
        let i = 0;
        while i < self.pull.serve_queue.len() {
            let q = self.pull.serve_queue[i].clone();
            let Some((bytes, siblings)) = self.swarm.block_with_proof(q.req.segment, q.req.block)
            else {
                self.pull.serve_queue.remove(i);
                continue;
            };
            let cost = if q.req.missing_symbols == 0 {
                bytes.len() + 32 * siblings.len() + 16
            } else {
                (q.req.missing_symbols as usize + 1) * 1100
            };
            if !self.pull_budget_take(cost) {
                break; // out of budget until the next tick
            }
            self.pull.serve_queue.remove(i);
            if q.req.missing_symbols == 0 {
                let f = BlockTransmission {
                    segment: q.req.segment,
                    block: q.req.block,
                    siblings,
                    data: bytes,
                };
                self.send(q.from, Channel::Pull, Frame::BlockTransmission(f), None);
            } else {
                let n = q.req.missing_symbols as usize + 1;
                let enc = FecEncoder::new(q.req.segment, q.req.block, bytes);
                let base = repair_esi_base(&self.node_id());
                let proof = bs_wire::frames::BlockProof {
                    segment: q.req.segment,
                    block: q.req.block,
                    sender_hop_depth: u8::MAX,
                    siblings,
                };
                self.send(q.from, Channel::Control, Frame::BlockProof(proof), None);
                for sym in enc.repair_symbols(base, n as u16) {
                    self.send(q.from, Channel::Datagram, Frame::RaptorQSymbol(sym), None);
                }
            }
            self.pull.pulls_served += 1;
            self.emit(Event::PullServed {
                segment: q.req.segment.0,
                block: q.req.block.0,
                to: q.from.to_string(),
            });
        }
        // Whatever is left past this point waited; count refusals only when dropped by age.
        let mut expired = Vec::new();
        self.pull.serve_queue.retain(|q| {
            let keep = q.deadline > now;
            if !keep {
                expired.push((q.req.segment.0, q.req.block.0));
            }
            keep
        });
        self.pull.pulls_refused += expired.len() as u64;
        for (segment, block) in expired {
            self.emit(Event::PullRefused { segment, block });
        }
    }

    /// A pulled block arrived.
    pub(super) fn on_block_transmission(&mut self, from: PeerAddr, bt: BlockTransmission) {
        let from_id = self
            .sessions
            .get(&from)
            .and_then(|s| s.node_id)
            .unwrap_or(bs_wire::NodeId::ZERO);
        let key = (bt.segment.0, bt.block.0);
        match self
            .swarm
            .add_full_block(bt.segment, bt.block, bt.data, bt.siblings, from_id)
        {
            Ok(Some(vb)) => {
                self.pull.pending.remove(&key);
                self.pull.blocks_pulled += 1;
                self.emit(Event::BlockPulled {
                    segment: bt.segment.0,
                    block: bt.block.0,
                });
                self.on_verified(vb, Some(from));
            }
            Ok(None) => {}
            Err(_) => {
                self.emit(Event::BlockRejected {
                    tree: 0,
                    segment: bt.segment.0,
                    block: bt.block.0,
                    from: from_id,
                });
            }
        }
    }

    /// After the first attach: fetch the manifests of the retained segments before
    /// the live edge so their blocks can be pulled ahead of the playout deadline
    /// (Ch4 §4.3.1 steps 3 and 5).
    pub(super) fn backfill_from(&mut self, parent: PeerAddr) {
        let edge = self.swarm.live_edge;
        if !edge.is_some() {
            return;
        }
        for back in 1..=1u32 {
            if edge.0 > back {
                let req = ManifestRequest {
                    segment: SegmentSeq(edge.0 - back),
                    selector: ManifestSelector::AllChunks,
                };
                self.send(parent, Channel::Control, Frame::ManifestRequest(req), None);
            }
        }
    }
}
