# Nocter Development Handoff

## Current State

Nocter v0.43.0 is published and externally audited. The annotated tag, remote `main`, GitHub latest
release, retained local candidate, public asset bytes, and extracted installation all resolve to
the recorded v0.43.0 identities.
Network.framework now supplies plain TCP and authenticated TLS
through one owner/event model. System trust, custom-root augmentation, hostname authentication,
synchronous TLS, asynchronous TLS, HTTP/1.1 ALPN, and custom-trust HTTPS have deterministic local
native coverage.

## Next Work

Plan v0.44.0 as the explicit asynchronous-producer and `future T` type-model change before adding
more asynchronous APIs or a `noblock` guarantee. Freeze its grammar, callable execution authority,
ownership semantics, migration boundary, and deletion of the old `async T` inference model before
implementation begins.

Preserve every published tag and asset, including v0.43.0.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
