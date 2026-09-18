# Nocter Development Handoff

## Current State

Nocter v0.58.0 is published and externally audited. v0.59.0 development is active with an
integrity-checked binary-record milestone. Phase 0 owns one explicit prefix-decode outcome for
complete, incomplete, overflowed, and non-canonical input before variable-width codecs, checksums,
or stream composition are admitted. The active scope lives in
[`development/history/milestones/v0.59.0.md`](history/milestones/v0.59.0.md).

## Next Work

Complete the Phase 0 semantic result and its compiler, formatter, editor, and native fixtures. Then
build canonical base-128 integer codecs and CRC-32 as separate responsibilities, compose them with
existing cursors and storage, and prove the complete design through an integrity-checked append-only
record application.

Preserve the v0.58.0 tag, release asset, public notes, specification snapshot, and publication audit
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
