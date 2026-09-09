# Nocter Development Handoff

## Current State

Nocter v0.43.0 development is active on `develop-v0.43.0`. Phase 0 is complete.
Network.framework is the selected Darwin TLS provider, and v0.43.0 includes replacement of the TCP
stream/listener substrate so HTTPS does not leave two connection engines. Phase 1 has begun the
closed native adapter foundation.

## Next Work

Complete Phase 1's cross-thread callback mailbox, dispatch ownership, cancellation lifetime, and
reactor wake bridge. The fixed one-pointer Block ABI and synchronous native invocation are already
qualified. Keep native objects and callbacks compiler-owned; do not add source-level FFI or a
general callback escape hatch.

Preserve every published tag and asset.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
