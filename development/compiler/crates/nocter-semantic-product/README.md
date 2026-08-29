# nocter-semantic-product

## Responsibility

Expose the immutable result vocabulary produced by semantic computation without exposing its query
entry, database, stage functions, or input-publication machinery.

## Contract

Post-computation consumers depend on this crate when interpreting complete or recoverable semantic
outcomes. The crate intentionally exports only result types. `nocter-compiler-computation` remains
the sole production owner permitted to depend on and execute `nocter-semantic-computation`.

## Invariants

- Session and editor consumers cannot demand, reorder, or publish semantic queries.
- Re-exported values preserve their original identity; the contract performs no conversion or
  recomputation.
- This crate owns no source, query, session, target, backend, or protocol policy.
