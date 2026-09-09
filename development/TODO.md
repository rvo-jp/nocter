# Nocter Development Handoff

## Current State

Nocter v0.40.0 is published and externally audited. v0.41.0 implementation and source-tree
qualification are complete on `develop-v0.41.0`, with no open practical finding. Its candidate
identity is fixed at `0.41.0`; public latest-release references remain at v0.40.0.

## Next Work

Commit the release-content identity, run two independent complete compiler gates, run the explicit
public-HTTPS acquisition test, build the archive twice from isolated targets, qualify a fresh
installed home from the retained candidate, and record exact source and artifact identities. Stop
before publication, which remains a separate explicitly authorized operation.

Preserve the immutable v0.39.0 and v0.40.0 tags and assets.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
