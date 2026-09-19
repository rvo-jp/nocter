# Nocter Development Handoff

## Current State

Nocter v0.62.0 is published and externally audited. v0.63.0 implementation is complete and
reviewed: one variable-length target entropy role feeds transactional byte filling, full-width and
unbiased bounded integer generation, and allocation-free in-place shuffling through `std/random`.

## Next Work

Prepare v0.63.0 for release. Update the version authorities and release-facing records, build and
verify a disposable archive, then stop before publication unless publication is explicitly
requested.

Preserve the v0.62.0 tag, release asset, public notes, specification snapshot, and publication
audit without replacement. Any correction requires a new version and a newly qualified artifact.

Preserve every published tag and asset, including v0.49.0.

## Blockers

No blocker remains for v0.62.0.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
