# Nocter Development Handoff

## Current State

Nocter v0.46.0 is published and externally audited. The v0.47.0 candidate at release-content
commit `e191c63a881899e9e1df833184111d0ef4116026` is qualified, and its retained archive is ready for
authorized publication. Public latest-release references remain at v0.46.0 until that publication
commit. The candidate makes
unqualified `Reader` and `Writer` canonical asynchronous byte-stream contracts, gives synchronous
contracts explicit `BlockingReader` and `BlockingWriter` names, removes duplicated concrete
collection logic through generic defaults, and closes compiler convention and source-snapshot
authorities.

## Next Work

Wait for explicit publication authorization. Publication must reuse the retained qualified archive
without rebuilding it, update public latest-release references in a separate commit, create the
annotated `v0.47.0` tag, upload exactly one asset, and record a byte-for-byte public audit. The
[v0.47.0 milestone](history/milestones/v0.47.0.md),
[Phase 5 final review](history/reviews/v0.47.0-phase-5.md), and
[release-preparation record](history/milestones/v0.47.0-release-preparation.md) own the frozen
implementation and qualification evidence. Do not tag, push, upload, or change public
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
