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
[Phase 3 implementation review](reviews/v0.35.0-phase-3.md), and
[Phase 4 implementation review](reviews/v0.35.0-phase-4.md).

## Next Work

Implement v0.35.0 Phase 5 tooling and practical integration over the completed static-data and
Unicode-text product. Qualify formatter and structural output, semantic editor features, installed
standard-library behavior, examples, and application-level text use without adding editor-owned
Unicode semantics. Preserve the immutable v0.34.0 tag and release asset.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
