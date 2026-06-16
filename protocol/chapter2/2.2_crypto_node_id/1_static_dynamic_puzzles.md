# 1. Static and Dynamic Puzzles

To completely neutralize Sybil attacks (where an attacker spawns hundreds of thousands of fake nodes on a single machine), we bind every Node ID to a cryptographic keypair and require a significant expenditure of computational resources via Proof-of-Work (PoW).

### The Static Puzzle (Key Generation)
A peer must generate a 256-bit public key $PK_{\text{node}}$ (Ed25519) and find a static nonce $N_{\text{static}}$ (an 8-byte integer) such that the SHA-256 (or Blake3) hash of their concatenation has a prefix of $C_1$ zero bits:
$$\text{Blake3}(PK_{\text{node}} \parallel N_{\text{static}}) \le 2^{256 - C_1}$$

where $C_1$ is the static difficulty parameter (typically $C_1 = 16$). This establishes a permanent, expensive cryptographic identity.

### The Dynamic Puzzle (IP Binding)
To prevent an attacker from pre-generating millions of static IDs on a supercomputer and migrating them to a target network, the peer must physically bind their Node ID to their active public external IP address. 

The peer must find a dynamic nonce $N_{\text{dynamic}}$ such that:
$$\text{Blake3}(\text{Blake3}(PK_{\text{node}} \parallel N_{\text{static}}) \parallel IP_{\text{external}} \parallel N_{\text{dynamic}}) \le 2^{256 - C_2}$$

where $C_2$ is the dynamic difficulty parameter (typically $C_2 = 12$). This must be re-solved whenever the node's external IP address changes (e.g., cell tower handover), binding the cryptographic identity to a specific physical network location in real time.
