# Nocter Development Handoff

## Current State

Nocter v0.40.0 is published and externally audited. v0.41.0 implementation and source-tree
qualification are complete on `develop-v0.41.0`, with no open practical finding. No v0.41.0
release identity has been assigned.

## Next Work

Prepare and qualify one reproducible v0.41.0 release candidate from the completed implementation.
Update versioned release metadata, build the archive twice from isolated targets, qualify a fresh
installed home from the retained candidate, and record exact source and artifact identities.
Publication remains a separate explicitly authorized operation.

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
