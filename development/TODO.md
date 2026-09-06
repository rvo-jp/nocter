# Nocter Development Handoff

## Current State

Nocter v0.38.0 implementation and source-tree qualification are complete. Released behavior
remains v0.37.0 until the candidate receives release identity, passes reproducible packaging and
fresh installed-home qualification, and is published.

## Next Work

Enter v0.38.0 release preparation. Assign the version through the repository's existing single
release-identity workflow, prepare English public release notes and version references, run two
independent complete compiler gates, and qualify byte-identical archives plus a fresh installed
home. Reuse the retained qualified archive for publication; do not rebuild after qualification.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
