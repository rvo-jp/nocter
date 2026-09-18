# Nocter Development Handoff

## Current State

Nocter v0.57.0 is published and externally audited. v0.58.0 is active as one binary-data and
protocol-foundation milestone. Its completion definition and responsibility boundaries are the
authority in [`development/history/milestones/v0.58.0.md`](history/milestones/v0.58.0.md).

## Next Work

Complete v0.58.0 Phase 3 by connecting the canonical scalar codecs to dynamic storage and external
writers without introducing another cursor, buffer, or byte-order authority. Phases 0 through 2
are complete: `std/bytes` owns scalar conversion, `scan.ByteCursor` owns exact input progress, and
`fixed.ByteBuffer<N>` owns transactional bounded commit. Continue through the practical application
unless an observable language-design choice requires user direction.

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
