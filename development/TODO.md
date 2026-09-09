# Nocter Development Handoff

## Current State

Nocter v0.42.0 implementation and source-tree qualification are complete on the development
branch, with no open practical finding. Its candidate identity is fixed at `0.42.0`; public
latest-release references remain at v0.41.0.

## Next Work

Commit the release-content identity, run two independent complete compiler gates, run the explicit
public-HTTPS acquisition test, build the archive twice from isolated targets, qualify a fresh
installed home from the retained candidate, and record exact source and artifact identities. Stop
before publication, which remains a separate explicitly authorized operation.

Preserve every published tag and asset, including v0.41.0.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
