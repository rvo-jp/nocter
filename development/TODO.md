# Nocter Development Handoff

## Current State

Nocter v0.62.0 is published and externally audited. v0.63.0 Phases 0 and 1 are complete: one
variable-length target entropy role feeds `std/internal/entropy`, while checked `std/random`
provides transactional random-byte filling and full-width unsigned scalar generation.

## Next Work

Complete v0.63.0 Phase 2: derive unbiased bounded sampling and allocation-free Fisher-Yates
shuffling from the checked `std/random` scalar authority, with deterministic rejection and
permutation tests below the operating-system adapter.

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
