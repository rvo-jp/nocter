# Nocter Development Handoff

## Current State

Nocter v0.48.0 is published and externally audited. v0.49.0 Phase 5 is complete on
`develop-v0.49.0`. Closed status and output operations now compose the same launch, endpoint, and
exact-child observation authorities as streaming spawn. Their canonical forms are executor-safe;
the synchronous twins use `_blocking`. The former terminal command-I/O session and its duplicate
pipe construction and byte classifiers are gone.

## Next Work

Begin v0.49.0 Phase 6 with one complete pipeline application and editor qualification over ordinary
checked declarations. Cover output larger than pipe capacity, simultaneous stdout and stderr,
early stdin closure, exec rejection, nonzero and signal termination, timeout, cancellation, and
exact cleanup without adding another process or descriptor authority.

Preserve every published tag and asset, including v0.48.0.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
