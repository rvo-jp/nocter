# Nocter Development Handoff

## Current State

Nocter v0.38.0 is published and externally audited. v0.39.0 is active as the synchronous network
I/O foundation. Phase 0 is complete: the public, descriptor, deadline, target-adapter, and error
authorities are fixed in `development/design/network-io-design.md` without introducing network
concepts into the compiler pipeline.

## Next Work

Implement v0.39.0 Phase 1 numeric IPv4, IPv6, IP, and socket-address values. Establish the checked
`std/net` contract before implementation, keep parsing and canonical generation single-sourced,
and keep native socket records out of public values. Preserve the immutable v0.38.0 tag and asset.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
