# 2. 64-bit ConnID Migration

Unlike standard TCP connections (which are bound to the IP/Port 4-tuple and drop instantly during cell handovers), QUIC identifies connections using a persistent **64-bit Connection ID**. This allows the session to migrate seamlessly between network interfaces (e.g., WiFi to 5G cellular) without resetting the streaming buffer.

```text
  [ Client (WiFi) ] ------------------- QUIC ConnID: 0xFA9E4B ------------------> [ Swarm Parent ]
          |
  (Switches to 5G Cell)
          |
          v
  [ Client (5G IP) ] --- Sends non-probing packet with ConnID: 0xFA9E4B --------> [ Swarm Parent ]
                                                                                           |
                                                                                (Seamlessly migrates
                                                                                 socket endpoint!)
```

```text
// Appended to ICE execution thread:
// Dynamic IP Migration Handler (Threaded Event Listener)

On PhysicalInterfaceChangeDetected() do:
    NewLocalIP <- GetNewActiveInterfaceIP()
    ActiveConnID <- LocalQUICSession.GetActiveConnectionID()
    
    // Execute connection migration without closing session
    SendQUICPathValidationProbe(SharedSocket, NewLocalIP, ActiveConnID)
    AwaitPathResponse()
    LocalQUICSession.UpdateLocalEndpoint(NewLocalIP)
```
