# Nocter v0.3.0 Phase 2 Handoff

The completion criteria and implementation boundaries are owned by the
[v0.3.0 Development Contract](docs/v0.3.0.md) and
[Explicit Iteration and Collection Access](docs/iteration.md). Git owns chronological implementation
history.

## Current Baseline

- branch: `develop`
- released baseline: `v0.2.0`
- completed milestone gates: `v0.3.0 Phase 0` and `v0.3.0 Phase 1 Typed Literal Core`
- active milestone gate: `v0.3.0 Phase 2 Explicit Iteration and Collection Access`
- target: `arm64-darwin`
- required Phase 0 items: none
- required Phase 1 items: none

Phase 1 connects user-defined sequence and string literal declarations, declaration-identity
resolution, literal-only ephemeral packs, generic specialization, ownership cleanup, Phase 0
allocation contexts and provenance, distributed `Vec<T>` and `String`, and compiler-backed LSP
queries. Packaged-home tests observe native content, reverse drop order, stable allocation-abort
status, explicit context selection, indirect region escape rejection, and `Vec` storage release.

## Current Objective

Implement allocation-free readonly iteration, owned vector iteration with exact remaining-element
cleanup, provenance-safe optional element access, ownership-safe insertion/removal, and their
compiler-backed LSP facts. The active checklist and acceptance matrix live only in
`docs/v0.3.0.md`.

Do not include collection `for`, sequence spread, general iterator interfaces, Unicode character
semantics, interpolation, or broad LSP refactoring in Phase 2.
