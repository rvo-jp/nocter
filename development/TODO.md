# Nocter Development Handoff

## Current State

Nocter v0.38.0 is published and externally audited. v0.39.0 is active as the synchronous network
I/O foundation. Phases 0 and 1 are complete: the cross-responsibility contract is fixed and
`std/net` provides checked, native-tested, target-independent numeric address values without
introducing network concepts into the compiler pipeline.

## Next Work

Implement v0.39.0 Phase 2 as one shared socket-substrate and TCP change. Extend the Darwin target
adapter with logical socket operations and opaque native records, then build descriptor ownership,
cleanup, readiness, and absolute-deadline policy once beneath `TcpStream` and `TcpListener`.
Preserve the immutable v0.38.0 tag and asset.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
