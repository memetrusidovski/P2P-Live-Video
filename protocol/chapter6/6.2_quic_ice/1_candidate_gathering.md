# 1. Candidate Gathering (STUN/ICE)

To achieve stable stream transport over unstable wireless links, our protocol integrates Interactive Connectivity Establishment (ICE) negotiation over a UDP-based **QUIC Transport Layer**.

The peer's transport layer implements the following handler to negotiate connections and handle dynamic IP transitions. The "signaling path" is concrete: two peers that have never spoken share a **referrer** — the DHT guardian whose `GET_PEERS` response introduced one to the other, or the gossip neighbour whose `SHUFFLE` did — and `PUNCH_REQUEST` (0x1C, Appendix D §D.4.14) rides through it. The referrer forwards the requester's *observed* address, never a self-declared candidate list, which is why no ICE candidate frame exists in this protocol: the only address that matters is the one the referrer saw.

```text
Algorithm: ICE Handshake and QUIC Session Migrator
Input: TargetPeerCoordinates, LocalQUICSession
Output: Stable Connection Established

1:  LocalCandidates <- GatherLocalInterfaceAddresses()
2:  STUNCandidates <- QuerySTUNServers(SharedSocket)
3:  AllCandidates <- LocalCandidates + STUNCandidates
4:  
5:  // Rendezvous via the referrer: PUNCH_REQUEST (App D §D.4.14) to the guardian
6:  // or gossip neighbour that named the target; it forwards our OBSERVED address
7:  SendPunchRequest(ReferrerOf(TargetPeerCoordinates), TargetNodeID)
8:  RemoteCandidates <- [TargetPeerCoordinates]   // the target's observed address from its Peer Record
8:  
9:  // Active Hole Punching Probing Loop
10: ConnectionEstablished <- False
11: For candidate in RemoteCandidates do:
12:     SendQUICProbePacket(SharedSocket, candidate)
13:     If ReceiveQUICAck(candidate) then:
14:         BindQUICActivePath(LocalQUICSession, candidate)
15:         ConnectionEstablished <- True
16:         Break
```
