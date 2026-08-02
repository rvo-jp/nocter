# Nocter v0.3.0 Phase 2 Completion Handoff

The completion criteria and implementation boundaries are owned by the
[v0.3.0 Development Contract](docs/v0.3.0.md) and
[Explicit Iteration and Collection Access](docs/iteration.md). Git owns chronological implementation
history.

## Current Baseline

- branch: `develop`
- released baseline: `v0.2.0`
- completed milestone gates: `v0.3.0 Phase 0`, `v0.3.0 Phase 1 Typed Literal Core`, and
  `v0.3.0 Phase 2 Explicit Iteration and Collection Access`
- active milestone gate: none
- target: `arm64-darwin`
- required Phase 0 items: none
- required Phase 1 items: none
- required Phase 2 items: none

Phase 2 adds allocation-free `ViewIter<T>`, consuming `VecIntoIter<T>`, provenance-safe `get` and
`get_mut`, failure-atomic insertion, ownership-safe removal, exact remaining-range cleanup, and
specialized LSP queries. Packaged-home tests observe native scalar, byte, and move-only order,
source-loan retention, mutation visibility, deterministic failed-growth state, lexical-region
escape rejection, and cleanup across exhaustion, `break`, and `?`.

## Current Objective

No implementation objective is active. Define the next v0.3.0 phase in `docs/v0.3.0.md` before
starting work that expands the language or standard-library contract.

Collection `for`, sequence spread, general iterator interfaces, Unicode character semantics,
interpolation, and broad LSP refactoring remain outside the completed Phase 2 contract.
