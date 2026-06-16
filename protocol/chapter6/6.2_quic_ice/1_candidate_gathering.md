# 1. Candidate Gathering (STUN/ICE)

To achieve stable stream transport over unstable wireless links, our protocol integrates Interactive Connectivity Establishment (ICE) negotiation over a UDP-based **QUIC Transport Layer**.

The peer's transport layer implements the following handler to negotiate connections and handle dynamic IP transitions:

```text
Algorithm: ICE Handshake and QUIC Session Migrator
Input: TargetPeerCoordinates, LocalQUICSession
Output: Stable Connection Established

1:  LocalCandidates <- GatherLocalInterfaceAddresses()
2:  STUNCandidates <- QuerySTUNServers(SharedSocket)
3:  AllCandidates <- LocalCandidates + STUNCandidates
4:  
5:  // Transmit ICE candidates via gossip signaling path
6:  TransmitCandidatesToPeer(TargetPeerCoordinates, AllCandidates)
7:  RemoteCandidates <- AwaitRemoteCandidates(TargetPeerCoordinates)
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
