# Nocter Development Handoff

## Current State

Nocter v0.43.0 development is active on `develop-v0.43.0`. Phases 0 and 1 are complete.
Network.framework is the selected Darwin TLS provider, and v0.43.0 includes replacement of the TCP
stream/listener substrate so HTTPS does not leave two connection engines. Phase 2 is migrating the
plain TCP surface onto the closed native adapter foundation.

## Next Work

Migrate the public plain TCP stream and listener onto compiler-owned Network.framework primitives
while retaining the existing logical timeout, synchronous, asynchronous, stable-error, and unique
ownership contracts. Introduce the closed primitive operation surface before changing public
owners; then remove descriptor-backed TCP only after equivalent loopback and cancellation evidence
passes. Keep UDP on its descriptor substrate. Keep native objects and callbacks compiler-owned; do
not add source-level FFI or a general callback escape hatch.

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
