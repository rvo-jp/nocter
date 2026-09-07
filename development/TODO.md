# Nocter Development Handoff

## Current State

Nocter v0.38.0 is published and externally audited. v0.39.0 is active as the synchronous network
I/O foundation. Phases 0 through 4 are complete: `std/net` provides checked, native-tested numeric
addresses, synchronous TCP, boundary-preserving UDP, and finite monotonic operation timeouts over
one private descriptor/deadline policy and one Darwin ABI adapter, without introducing network
concepts into the compiler pipeline.

## Next Work

Implement v0.39.0 Phase 5 integration, tooling, and full qualification. Add an ordinary loopback
example, exercise the complete semantic-tooling capability matrix over the public network surface,
audit installed-home behavior, and run the release-candidate repository gate. Preserve the
immutable v0.38.0 tag and asset.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
