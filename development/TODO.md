# Nocter Development Handoff

## Current State

Nocter v0.57.0 is published and externally audited. v0.58.0 implementation is complete as one
binary-data and protocol-foundation milestone. Its completion definition, responsibility
boundaries, and closed phase record are the authority in
[`development/history/milestones/v0.58.0.md`](history/milestones/v0.58.0.md).

## Next Work

Prepare v0.58.0 for release without expanding its completed scope. `std/bytes` owns public scalar
conversion, `std/internal/bytes` owns its one package implementation, `scan.ByteCursor` owns exact
input progress, `fixed.ByteBuffer<N>` owns transactional bounded commit, `Vec<T>` owns batch growth,
and existing writer contracts transport the same encoded slice. The `binary-record` package passes
valid, truncated, invalid-magic, trailing-input, installed-home, and exact wire-image execution.

The Phase 5 review is recorded in
[`development/history/reviews/v0.58.0-phase-5.md`](history/reviews/v0.58.0-phase-5.md). Release
preparation must requalify the versioned archive; Phase 5 deliberately did not change release
metadata, create a tag, push commits, or publish an asset.

Preserve the v0.57.0 tag, release asset, public notes, specification snapshot, and publication audit
without replacement. Any correction requires a new version and a newly qualified artifact.

Preserve every published tag and asset, including v0.49.0.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
