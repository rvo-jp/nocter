# Nocter Development Handoff

## Current State

Nocter v0.48.0 is published and externally audited. v0.49.0 Phase 0 is complete on
`develop-v0.49.0`. The adopted structured-process design makes spawn, child ownership, standard
stream endpoints, observation, and abandonment one model. It identifies the current generated
runtime's descriptor/timer-only `poll` mapping as the first implementation boundary: process exit
must become a target-independent wait interest and a native event, never a blocking or periodic
standard-library probe.

## Next Work

Implement v0.49.0 Phase 1 as one runtime change: extend the sole semantic and ABI interest
authority, migrate the host Darwin reactor and generated ARM64 Darwin root to a unified native
event mapping, and qualify mixed descriptor, timer, and process completion including cancellation
and stale events. Do not add a process wait primitive to standard source until that lower contract
is complete.

Preserve every published tag and asset, including v0.48.0.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
