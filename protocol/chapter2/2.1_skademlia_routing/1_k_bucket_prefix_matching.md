# 1. k-Bucket Prefix Matching (The Secure Routing Table)

To operate without a centralized tracker, every peer maintains a local routing table structured as a set of **k-buckets** based on prefix matching in a 256-bit space. 

Let $NodeID_{\text{local}}$ be the 256-bit identifier of the local node. The routing table is divided into 256 distinct k-buckets:
$$\mathcal{B} = \{B_0, B_1, \dots, B_{255}\}$$

A k-bucket $B_j$ contains contact information (IP address, port, and S/Kademlia Node ID) for peers whose IDs share a common prefix with $NodeID_{\text{local}}$ of length exactly $j$ bits, followed by a different $(j+1)$-th bit. The parameter $k$ represents the bucket capacity (typically $k = 20$).

```text
Local Node ID: 110100...
Distance Categories (XOR Metric):
  B_0: Peers starting with 0...           (Distance range: [2^255, 2^256 - 1])
  B_1: Peers starting with 10...          (Distance range: [2^254, 2^255 - 1])
  B_2: Peers starting with 111...         (Distance range: [2^253, 2^254 - 1])
```

By using the XOR metric $d(x, y) = x \oplus y$, the distance between nodes is strictly symmetric ($d(x, y) = d(y, x)$), which guarantees that if node A is close to node B, node B is also close to node A, ensuring stable bidirectional routing maps.
