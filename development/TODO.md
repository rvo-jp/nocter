# Nocter Development Handoff

## Current State

Nocter v0.35.0 is [published and externally audited](releases/v0.35.0.md). Its tag and release asset
are immutable.

The current implementation is the published v0.35.0 immutable-static and Unicode-text product. Its
implementation, qualification, publication, and public evidence belong to the
[v0.35.0 milestone](milestones/v0.35.0.md),
[Phase 0 design review](reviews/v0.35.0-phase-0.md),
[Phase 1 implementation review](reviews/v0.35.0-phase-1.md),
[Phase 2 implementation review](reviews/v0.35.0-phase-2.md),
[Phase 3 implementation review](reviews/v0.35.0-phase-3.md),
[Phase 4 implementation review](reviews/v0.35.0-phase-4.md),
[Phase 5 integration review](reviews/v0.35.0-phase-5.md),
[Phase 6 qualification and final review](reviews/v0.35.0-phase-6.md), the
[release-preparation record](milestones/v0.35.0-release-preparation.md), and the
[release audit](releases/v0.35.0.md).

## Next Work

Define the next milestone as one coherent practical application capability before changing the
language or standard library. Preserve the immutable v0.35.0 tag and release asset.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
