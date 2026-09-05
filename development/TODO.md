# Nocter Development Handoff

## Current State

Nocter v0.34.0 is [published and externally audited](releases/v0.34.0.md). Its tag and release asset
are immutable.

The current implementation is the published v0.34.0 Unicode-scalar foundation plus the complete
v0.35.0 immutable-static, generated Unicode-data, character-property, borrowed Unicode-trim,
default casing, and owned suffix-mutation products. The release identity is assigned to v0.35.0;
publication status remains v0.34.0 until a qualified archive is separately authorized. Active
scope belongs to the
[v0.35.0 milestone](milestones/v0.35.0.md),
[Phase 0 design review](reviews/v0.35.0-phase-0.md),
[Phase 1 implementation review](reviews/v0.35.0-phase-1.md),
[Phase 2 implementation review](reviews/v0.35.0-phase-2.md),
[Phase 3 implementation review](reviews/v0.35.0-phase-3.md),
[Phase 4 implementation review](reviews/v0.35.0-phase-4.md),
[Phase 5 integration review](reviews/v0.35.0-phase-5.md), and
[release-preparation record](milestones/v0.35.0-release-preparation.md).

## Next Work

Qualify the exact clean v0.35.0 candidate in Phase 6. Run two independent full compiler gates,
generated-data reproducibility, explicit public-HTTPS acquisition, clean installed-home
qualification, duplicate archive comparison, dead-static-data inspection, and final architecture
review. Record the retained archive identity and stop before publication. Preserve the immutable
v0.34.0 tag and release asset.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
