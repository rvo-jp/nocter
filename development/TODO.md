# Nocter Development Handoff

## Current State

Nocter v0.43.0 development is active on `develop-v0.43.0`. Phases 0-6 are complete and the
implementation and source-tree qualification are closed.
Network.framework now supplies plain TCP and authenticated TLS
through one owner/event model. System trust, custom-root augmentation, hostname authentication,
synchronous TLS, asynchronous TLS, HTTP/1.1 ALPN, and custom-trust HTTPS have deterministic local
native coverage.

## Next Work

Prepare the v0.43.0 release: update release identity and public release documentation, build and
inspect the final archive from a clean revision, requalify the extracted adjacent toolchain home,
and record reproducibility evidence before tagging or publication. Phase 6 already qualifies trust
failures, hostname and certificate validity, truncated records, handshake deadlines, negotiated
HTTP/1.1 ALPN, cancellation ownership, editor projection, examples, pre-release packaging, and the
complete repository gate. Keep UDP on its descriptor substrate and keep native objects and
callbacks compiler-owned; do not add source-level FFI or a general callback escape hatch.

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
