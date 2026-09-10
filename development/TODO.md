# Nocter Development Handoff

## Current State

Nocter v0.43.0 implementation and source-tree qualification are complete on the development
branch, with no open practical finding. Its candidate identity is fixed at `0.43.0`; public
latest-release references remain at v0.42.0.
Network.framework now supplies plain TCP and authenticated TLS
through one owner/event model. System trust, custom-root augmentation, hostname authentication,
synchronous TLS, asynchronous TLS, HTTP/1.1 ALPN, and custom-trust HTTPS have deterministic local
native coverage.

## Next Work

Commit the release-content identity, run two independent complete compiler gates, run the explicit
public-HTTPS acquisition test, build the archive twice from isolated targets, qualify a fresh
installed home from the retained candidate, and record exact source and artifact identities.
Publication is already authorized, but must reuse only that retained qualified archive.

Preserve every published tag and asset, including v0.42.0.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
