# Nocter Development Handoff

## Current State

Nocter v0.58.0 is published and externally audited. v0.59.0 Phases 0 through 3 are complete.
`std/bytes` owns typed canonical integer decoding and `std/checksum` independently owns
allocation-free one-shot and incremental ISO-HDLC CRC-32. Its static table is verified from the
reflected polynomial, every two-chunk boundary produces the standard check value, and observing a
checksum does not consume its state. The active scope lives in
[`development/history/milestones/v0.59.0.md`](history/milestones/v0.59.0.md).

## Next Work

Build the append-only integrity-checked record application over the completed byte, checksum,
cursor, fixed-storage, and dynamic-storage contracts. One wire image must serve blocking and
asynchronous fragmented readers. Reject excessive or non-canonical lengths before allocation or
cursor mutation, preserve every preceding valid record after a damaged tail, and do not introduce
a generic framing framework before a second application demonstrates the same contract.

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
