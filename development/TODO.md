# Nocter Development Handoff

## Current State

Nocter v0.62.0 is published and externally audited. v0.63.0 Phases 0 through 2 are complete: one
variable-length target entropy role feeds transactional byte filling, full-width and unbiased
bounded integer generation, and allocation-free in-place shuffling through `std/random`.

## Next Work

Complete v0.63.0 Phase 3: add a runnable public example, review the complete compiler and standard
source for duplicate entropy or sampling authority, then run documentation, installation,
packaging, and release-qualification gates.

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
