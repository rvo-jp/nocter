# Nocter Development Handoff

## Current State

Nocter v0.58.0 is published and externally audited. v0.59.0 Phases 0 through 4 are complete.
`std/bytes` owns typed canonical integer decoding and `std/checksum` independently owns
allocation-free one-shot and incremental ISO-HDLC CRC-32. The `binary-record` application now
composes those authorities into a deterministic append-only record log and proves identical
framing through blocking and asynchronous fragmented readers. The active scope lives in
[`development/history/milestones/v0.59.0.md`](history/milestones/v0.59.0.md).

## Next Work

Complete editor, whole-area review, and release qualification for the byte-codec, checksum,
cursor/storage composition, and binary-record sources. Verify public and implementation sources
through the complete editor surface, run the repository and distribution gates, and audit for a
second endian, checksum, cursor, framing, or stream authority. Keep generic framing, schema, and
authentication APIs outside the release unless an independent application establishes their
contract.

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
