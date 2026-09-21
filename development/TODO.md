# Nocter Development Handoff

## Current State

Nocter v0.65.0 Practical Stateful Services is published and externally audited. v0.66.0 Durable
Local Application State has completed Phases 0–5 implementation and review. Durable filesystem
replacement, bounded recoverable storage, persistent HTTP service behavior, editor coverage, and
complete compiler qualification are closed.

## Next Work

Prepare v0.66.0 for release: assign release identity, write public release notes, qualify two
independently built archives and installed homes, inspect the exact candidate diff, and publish
only after those release gates pass. The implementation review is
[`development/history/reviews/v0.66.0-phase-5.md`](history/reviews/v0.66.0-phase-5.md).

Preserve the v0.64.0 tag, release asset, public notes, specification snapshot, and publication
audit without replacement. Any correction requires a new version and a newly qualified artifact.

Preserve every published tag and asset, including v0.49.0.

## Blockers

No blocker remains from v0.65.0.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
