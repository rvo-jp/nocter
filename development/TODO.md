# Nocter Development Handoff

## Current State

Nocter v0.43.0 development is active on `develop-v0.43.0`. Phases 0-5 are complete.
Network.framework now supplies plain TCP and authenticated TLS
through one owner/event model. System trust, custom-root augmentation, hostname authentication,
synchronous TLS, asynchronous TLS, HTTP/1.1 ALPN, and custom-trust HTTPS have deterministic local
native coverage.

## Next Work

Perform Phase 6 failure, lifecycle, tooling, packaging, and security review. Qualify malformed,
unknown, and expired trust material, truncated handshakes, timeout and cancellation, then exercise
editor behavior and the packaged toolchain. Keep UDP on its descriptor substrate and keep native
objects and callbacks compiler-owned; do not add source-level FFI or a general callback escape
hatch.

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
