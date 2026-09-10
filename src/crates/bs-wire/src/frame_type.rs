//! The canonical frame-type registry (Appendix D §D.3) and transport mapping (§D.2).

/// Which transport a frame travels on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Transport {
    /// Plain UDP before any QUIC session exists; carries a validation block.
    PreSessionUdp,
    /// QUIC bidirectional control stream.
    QuicStream,
    /// QUIC unreliable datagram (RFC 9221).
    QuicDatagram,
}

macro_rules! frame_types {
    ($( $(#[$doc:meta])* $name:ident = $code:literal => $transport:ident ),* $(,)?) => {
        /// Every frame code in the registry.
        #[allow(non_camel_case_types)]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        #[repr(u8)]
        pub enum FrameType {
            $( $(#[$doc])* $name = $code, )*
        }
        impl FrameType {
            /// All registered frame types, in code order.
            pub const ALL: &'static [FrameType] = &[ $( FrameType::$name, )* ];
            /// Look up a code.
            pub fn from_code(code: u8) -> Option<Self> {
                match code { $( $code => Some(FrameType::$name), )* _ => None }
            }
            /// The primary transport for this frame type.
            pub fn transport(self) -> Transport {
                match self { $( FrameType::$name => Transport::$transport, )* }
            }
            /// Registry name, as written in Appendix D.
            pub fn name(self) -> &'static str {
                match self { $( FrameType::$name => stringify!($name), )* }
            }
        }
    };
}

frame_types! {
    /// Carries validation block; solicits PONG.
    PING = 0x01 => PreSessionUdp,
    /// Reflects observed source address (STUN-lite).
    PONG = 0x02 => PreSessionUdp,
    /// HyParView join, upgraded to QUIC on acceptance.
    JOIN = 0x03 => PreSessionUdp,
    /// Signed broadcaster shutdown.
    STREAM_END = 0x04 => QuicStream,
    /// Membership promotion or tree-slot request.
    NEIGHBOR = 0x05 => QuicStream,
    /// Passive-set shuffle; TTL = 0 over a session is a direct request.
    SHUFFLE = 0x06 => QuicStream,
    /// Graceful disconnect with reason (also carries join rejections, ISSUE-059).
    DISCONNECT = 0x07 => QuicStream,
    /// Accepts JOIN / NEIGHBOR / tree-join.
    ACCEPTED = 0x08 => QuicStream,
    /// HyParView random-walk propagation.
    FORWARD_JOIN = 0x09 => QuicStream,
    /// DHT stream registration.
    REGISTER_PEER = 0x0A => PreSessionUdp,
    /// DHT peer discovery.
    GET_PEERS = 0x0B => PreSessionUdp,
    /// S/Kademlia node lookup.
    FIND_NODE = 0x0C => PreSessionUdp,
    /// S/Kademlia value lookup.
    FIND_VALUE = 0x0D => PreSessionUdp,
    /// Parent-selection capacity probe.
    PROBE = 0x0E => PreSessionUdp,
    /// Probe answer with per-tree state.
    PROBE_RESPONSE = 0x0F => PreSessionUdp,
    /// Pull-path block with inline Merkle proof.
    BLOCK_TRANSMISSION = 0x10 => QuicStream,
    /// Per-chunk signed manifest.
    MANIFEST = 0x11 => QuicStream,
    /// Media symbol.
    RAPTORQ_SYMBOL = 0x12 => QuicDatagram,
    /// Push-path Merkle proof preceding a block's symbols.
    BLOCK_PROOF = 0x13 => QuicStream,
    /// HMAC-authenticated neighbour gossip.
    GOSSIP_EXCHANGE = 0x14 => QuicStream,
    /// Buffer availability bitfield.
    BUFFER_STATE_BITFIELD = 0x15 => QuicStream,
    /// Reactive pull.
    PULL_REQUEST = 0x16 => QuicStream,
    /// Manifest with slicing matrix (forest resize / fold).
    MANIFEST_UPDATE = 0x17 => QuicStream,
    /// Per-tree child roster for Deputy election.
    ROSTER = 0x18 => QuicStream,
    /// Commit-then-sample receipt presentation.
    RANK_PROOF = 0x19 => QuicStream,
    /// Publisher → guardian Stream Record write.
    STORE_RECORD = 0x1A => PreSessionUdp,
    /// Guardian → publisher counts.
    STORE_RECORD_ACK = 0x1B => PreSessionUdp,
    /// Hole-punch rendezvous.
    PUNCH_REQUEST = 0x1C => PreSessionUdp,
    /// Fetch past or pending manifests.
    MANIFEST_REQUEST = 0x1D => QuicStream,
    /// Parent releases a child but keeps serving until a deadline.
    DRAIN_NOTICE = 0x1E => QuicStream,
    /// Source-signed codec / container / initialisation data per layer (SOLUTION-060).
    STREAM_DESCRIPTOR = 0x1F => QuicStream,
    /// Signed per-segment per-tree upload receipt.
    PROOF_OF_UPLOAD = 0x20 => QuicStream,
    /// Tit-for-Tat choke state.
    CHOKE_STATE = 0x21 => QuicStream,
    /// Evidence-carrying accusations.
    REPUTATION_AUDIT_GOSSIP = 0x22 => QuicStream,
    /// Emergent relay proposal.
    RELAY_PROPOSAL = 0x30 => QuicStream,
    /// Emergent relay bind.
    RELAY_BIND = 0x31 => QuicStream,
}

impl FrameType {
    /// Whether frames of this type carry the 152-byte validation block after the header.
    pub fn carries_validation_block(self) -> bool {
        self.transport() == Transport::PreSessionUdp
    }
}

impl core::fmt::Display for FrameType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.name())
    }
}
