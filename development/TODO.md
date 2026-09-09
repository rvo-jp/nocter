# Nocter Development Handoff

## Current State

Nocter v0.42.0 Phase 2 is complete on the development branch. Async HTTP timeout operations
delegate connection, write, readiness, and idle-input timing to the existing async TCP authority.
Native fixtures cover lazy cancellation, exclusive response-read ownership, head/body idle
timeouts, write backpressure, and premature peer closure.

## Next Work

Implement v0.42.0 Phase 3: add the practical request and response operations needed by ordinary
applications. Every convenience must derive from the existing message, exchange, and unique
response-owner contracts rather than duplicate policy or imply replay.

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
