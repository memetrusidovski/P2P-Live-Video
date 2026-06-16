# 3.3 Dynamic Churn Recovery

This subchapter details the sequence of events and algorithmic logic used to heal broken connections before a video buffer runs dry.

## Deep Dive Topics:
*   [1. Recovery Timeline](1_recovery_timeline.md): The 0-250ms active connection drop and repair timeline.
*   [2. Active Probing Mechanics](2_active_probing.md): Keepalive heartbeats and immediate eviction rules.
