# Nocter Development Handoff

## Current State

Nocter v0.65.0 Practical Stateful Services is active. Phases 0 through 3 completed page-backed
shared ownership, descriptor notification, mutexes, bounded channels, cooperative cancellation,
structured service ownership, process-wide termination observation, and bounded HTTP application
data. The previous v0.64.0 Application Encoding and Identity release is published and externally
audited.

## Next Work

Complete Phase 4 as one stateful-routing model. One router must carry explicit application state,
lend that state to selected handlers without a global or hidden clone, and retain the existing
single route-selection authority. Define cancellation, handler failure, and connection ownership
before adding convenience adapters.

Preserve the v0.64.0 tag, release asset, public notes, specification snapshot, and publication
audit without replacement. Any correction requires a new version and a newly qualified artifact.

Preserve every published tag and asset, including v0.49.0.

## Blockers

No external blocker is known. Phase 4 must preserve the current linear request/response lifecycle
while allowing concurrent handlers to reach explicitly shared application state.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
