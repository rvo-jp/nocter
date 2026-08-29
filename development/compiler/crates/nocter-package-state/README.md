# nocter-package-state

## Responsibility

Own atomic, recoverable mutation of package declarations, locks, and installed exact-package state.

## Contract

The crate stages one intended package-state transition, validates its original root source and
destination set, then commits or discards the complete transaction. Package resolution supplies
domain values; acquisition supplies staged content. A caller may inject the read-only package
resolver so command parsing uses its compiler-computation source authority. Editor overlays cannot
enter this mutation boundary.

## Internal Responsibilities

- root-source compare-before-write authority
- staging directories and destination validation
- lock/source transaction assembly
- atomic commit and cleanup
- post-commit package-graph revalidation through the injected resolver

## Invariants

- A failed or interrupted transaction exposes no partial persistent state.
- Concurrent root-source changes are rejected instead of overwritten.
- Every destination is canonical and inside the authorized package state root.
- Transaction authority is consumed once.
- A transaction never returns a package snapshot captured before its own source commit.
