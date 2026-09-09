# Nocter Development Handoff

## Current State

Nocter v0.43.0 development is active on `develop-v0.43.0`. Phases 0 and 1 are complete.
Network.framework is the selected Darwin TLS provider, and v0.43.0 includes replacement of the TCP
stream/listener substrate so HTTPS does not leave two connection engines. Phase 2 is migrating the
plain TCP surface onto the closed native adapter foundation.

## Next Work

Connect the production plain-connection constructor and frozen owner lifecycle to the Machine
primitive boundary, then implement receive, send, local/remote address, cancellation, and release
without exposing native handles in source declarations. Migrate the public plain TCP stream only
after equivalent loopback, timeout, transfer, cancellation, and stable-error evidence passes; then
adopt accepted connections and migrate listeners. Remove descriptor-backed TCP only after the
replacement is qualified. Keep UDP on its descriptor substrate. Keep native objects and callbacks
compiler-owned; do not add source-level FFI or a general callback escape hatch.

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
