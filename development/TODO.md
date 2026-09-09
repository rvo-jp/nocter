# Nocter Development Handoff

## Current State

Nocter v0.43.0 development is active on `develop-v0.43.0`. Phases 0 and 1 are complete.
Network.framework is the selected Darwin TLS provider, and v0.43.0 includes replacement of the TCP
stream/listener substrate so HTTPS does not leave two connection engines. Phase 2 is migrating the
plain TCP surface onto the closed native adapter foundation.

## Next Work

Materialize the frozen Network.framework operation/lifecycle contract through the canonical ARM64
adapter imports and migrate the public plain TCP stream and listener while retaining the existing
logical timeout, synchronous, asynchronous, stable-error, and unique-ownership contracts. Define
the fixed connection owner plus receive/send callback targets, then implement and qualify
connection creation, start, transfer, address, cancellation, and release before listener adoption.
Remove descriptor-backed TCP only after equivalent loopback and cancellation evidence passes. Keep
UDP on its descriptor substrate. Keep native objects and callbacks compiler-owned; do not add
source-level FFI or a general callback escape hatch.

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
