# Nocter Development Handoff

## Current State

Nocter v0.43.0 is published and externally audited. v0.44.0 implementation and source-tree
qualification are complete on `develop-v0.44.0`. Its release identity is fixed at `0.44.0`; public
latest-release references remain at v0.43.0 until the publication commit. The old result-type-driven
asynchronous producer model has been replaced by an explicit `async` declaration modifier and a
separate `future T` structural type throughout the compiler and public sources.

## Next Work

Commit the release-content identity, run independent complete compiler gates, build the archive
twice from isolated targets, qualify a fresh installed home from the retained candidate, and record
exact source and artifact identities. Publication is authorized, but must reuse only that retained
qualified archive.

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
