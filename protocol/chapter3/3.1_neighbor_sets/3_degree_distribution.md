# 3. Degree Distribution

This double-walk algorithm mathematically guarantees that the resulting global overlay graph has a degree distribution strongly centered around the target active size $c_a$. 

Unlike pure random meshes where some nodes might randomly accrue 100 connections (creating hotspots) while others accrue 0, the HyParView eviction rules ensure a symmetric graph. 

Because any node forced to accept a random walk `FORWARD_JOIN` must drop an existing connection if it is full (pushing the dropped connection into the passive set), the maximum out-degree of any node is strictly hard-capped at $c_a$, providing a mathematically provable defense against accidental hotspot bottlenecks and ensuring uniform resource distribution across the entire swarm.
