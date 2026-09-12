# Nocter Development Handoff

## Current State

Nocter v0.46.0 is published and externally audited. v0.47.0 implementation Phases 0–7 are complete,
and the release identity is fixed at `0.47.0` on `develop-v0.47.0`. Public latest-release references
remain at v0.46.0 until an explicitly authorized publication commit. The candidate makes
unqualified `Reader` and `Writer` canonical asynchronous byte-stream contracts, gives synchronous
contracts explicit `BlockingReader` and `BlockingWriter` names, removes duplicated concrete
collection logic through generic defaults, and closes compiler convention and source-snapshot
authorities.

## Next Work

Commit the release-content identity, rerun the exact-content compiler and documentation gates,
produce two independent byte-identical archives, and qualify a fresh installed home before
publication. The [v0.47.0 milestone](history/milestones/v0.47.0.md),
[Phase 5 final review](history/reviews/v0.47.0-phase-5.md), and
[release-preparation record](history/milestones/v0.47.0-release-preparation.md) own the frozen
implementation and qualification contract. Do not tag, push, upload, or change public
latest-release references without explicit authorization.

Preserve every published tag and asset, including v0.46.0.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
