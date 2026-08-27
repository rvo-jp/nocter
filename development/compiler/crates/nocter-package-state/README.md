# nocter-package-state

## Responsibility

Own atomic, recoverable mutation of package declarations, locks, and installed exact-package state.

## Contract

The crate stages one intended package-state transition, validates its original root source and
destination set, then commits or discards the complete transaction. Package resolution supplies
domain values; acquisition supplies staged content. Editor overlays cannot enter this boundary.

## Internal Responsibilities

- root-source compare-before-write authority
- staging directories and destination validation
- lock/source transaction assembly
- atomic commit and cleanup

## Invariants

- A failed or interrupted transaction exposes no partial persistent state.
- Concurrent root-source changes are rejected instead of overwritten.
- Every destination is canonical and inside the authorized package state root.
- Transaction authority is consumed once.
