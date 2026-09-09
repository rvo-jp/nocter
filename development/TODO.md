# Nocter Development Handoff

## Current State

Nocter v0.42.0 Phase 1 is complete on the development branch. HTTP request normalization,
final-response-head selection, and response-body progression each have one transport-independent
authority consumed by synchronous and asynchronous orchestration. `Client.send_async` and
`Response.read_async` cross the native reactor, including fragmented chunked input after an
informational response.

## Next Work

Implement v0.42.0 Phase 2: qualify timeout and cancellation behavior at every async HTTP ownership
transition. Keep connection timeout, write backpressure, idle head/body reads, and any future
whole-request deadline as distinct contracts.

Preserve every published tag and asset, including v0.41.0.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
