# 1.3 State-Machine Representation of Peer Life Cycle

This subchapter maps out the exact state machine a node executes from initial boot to stream termination.

## Deep Dive Topics:
*   [1. Peer State Transition Model](1_transition_model.md): Definition of the 7 discrete connection states.
*   [2. Algorithmic Core Main Loop](2_algorithmic_core_loop.md): The microsecond-level event-driven polling loop.
*   [3. Connection Handover and Mobile Tolerance](3_connection_handover.md): QUIC connection migration during IP changes.
