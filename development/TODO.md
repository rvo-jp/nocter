# Nocter Development Handoff

## Current State

Nocter v0.34.0 is [published and externally audited](releases/v0.34.0.md). Its tag and release asset
are immutable.

The current implementation is the published v0.34.0 Unicode-scalar foundation plus the complete
v0.35.0 immutable-static, generated Unicode-data, character-property, borrowed Unicode-trim,
default casing, and owned suffix-mutation products. Its active scope belongs to the
[v0.35.0 milestone](milestones/v0.35.0.md),
[Phase 0 design review](reviews/v0.35.0-phase-0.md),
[Phase 1 implementation review](reviews/v0.35.0-phase-1.md),
[Phase 2 implementation review](reviews/v0.35.0-phase-2.md),
[Phase 3 implementation review](reviews/v0.35.0-phase-3.md),
[Phase 4 implementation review](reviews/v0.35.0-phase-4.md), and
[Phase 5 integration review](reviews/v0.35.0-phase-5.md).

## Next Work

Prepare and stabilize v0.35.0 in Phase 6. Run release-version assignment, full compiler and
documentation gates, generated-data reproducibility, clean installed-home qualification, duplicate
archive comparison, dead-static-data inspection, and final architecture review before publication.
Preserve the immutable v0.34.0 tag and release asset.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
