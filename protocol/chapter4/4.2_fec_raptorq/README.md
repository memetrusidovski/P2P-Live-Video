# 4.2 Forward Error Correction (FEC) Layer: RaptorQ Packetization

This subchapter details the mathematical mechanics of the fountain codes used to eliminate UDP packet-loss retransmission delays.

## Deep Dive Topics:
*   [1. Generator Matrix Math](1_generator_matrix_math.md): RFC 6330 Fountain code probabilities and encoding symbols.
*   [2. Systematic Dispersion](2_systematic_dispersion.md): Distributing parity across multi-forest parents.
*   [3. Symbol Packet Layout](3_symbol_packet_layout.md): `RAPTORQ_SYMBOL` byte alignments (SBN and ESI).
