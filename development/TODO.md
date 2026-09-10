# Nocter Development Handoff

## Current State

Nocter v0.43.0 development is active on `develop-v0.43.0`. Phases 0-4 are complete and the Phase 5
HTTPS implementation is complete. Network.framework now supplies plain TCP and authenticated TLS
through one owner/event model. System trust, custom-root augmentation, hostname authentication,
synchronous TLS, asynchronous TLS, and HTTP/1.1 ALPN have deterministic local native coverage.

## Next Work

Complete Phase 5 with a public HTTP client configuration that can carry an owned custom trust root
through the existing `ClientTransport` selection without teaching the HTTP codec about TLS. Use it
to qualify a deterministic local HTTPS response through both synchronous and asynchronous clients.
Then perform Phase 6 failure, lifecycle, tooling, packaging, and security review. Keep UDP on its
descriptor substrate and keep native objects and callbacks compiler-owned; do not add source-level
FFI or a general callback escape hatch.

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
