# Nocter Development Handoff

## Current State

Nocter v0.34.0 is [published and externally audited](releases/v0.34.0.md). Its tag and release asset
are immutable.

The current implementation is the published v0.34.0 Unicode-scalar foundation plus the complete
and qualified v0.35.0 immutable-static, generated Unicode-data, character-property, borrowed
Unicode-trim, default casing, and owned suffix-mutation products. Release-content commit
`97147821a0b8ae78f525768bf705074e75fd0254` produced the retained qualified archive; publication
status remains v0.34.0 until publication is separately authorized. Active scope belongs to the
[v0.35.0 milestone](milestones/v0.35.0.md),
[Phase 0 design review](reviews/v0.35.0-phase-0.md),
[Phase 1 implementation review](reviews/v0.35.0-phase-1.md),
[Phase 2 implementation review](reviews/v0.35.0-phase-2.md),
[Phase 3 implementation review](reviews/v0.35.0-phase-3.md),
[Phase 4 implementation review](reviews/v0.35.0-phase-4.md),
[Phase 5 integration review](reviews/v0.35.0-phase-5.md),
[Phase 6 qualification and final review](reviews/v0.35.0-phase-6.md), and the
[release-preparation record](milestones/v0.35.0-release-preparation.md).

## Next Work

Await separate publication authorization. Publication must reuse
`dist/nocter-v0.35.0-arm64-darwin.tar.gz` with SHA-256
`62b30603177fc05ed9e2e50d6051ec59eeae5fbf634c05cea9467661f3dd820e`, update public latest-release
links in a separate commit, create one annotated `v0.35.0` tag, upload exactly one asset, and verify
the public download byte for byte. Do not rebuild the qualified candidate. Preserve the immutable
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
