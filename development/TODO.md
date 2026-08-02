# Nocter v0.3.0 Phase 1 Completion Handoff

The completion criteria and implementation boundaries are owned by the
[v0.3.0 Development Contract](docs/v0.3.0.md) and
[Typed Literal Core](docs/typed-literals.md). Git owns chronological implementation history.

## Current Baseline

- branch: `develop`
- released baseline: `v0.2.0`
- completed milestone gates: `v0.3.0 Phase 0` and `v0.3.0 Phase 1 Typed Literal Core`
- target: `arm64-darwin`
- required Phase 0 items: none
- required Phase 1 items: none

Phase 1 connects user-defined sequence and string literal declarations, declaration-identity
resolution, literal-only ephemeral packs, generic specialization, ownership cleanup, Phase 0
allocation contexts and provenance, distributed `Vec<T>` and `String`, and compiler-backed LSP
queries. Packaged-home tests observe native content, reverse drop order, stable allocation-abort
status, explicit context selection, indirect region escape rejection, and `Vec` storage release.

## Next Milestone

No later v0.3.0 phase is currently defined. Before further feature implementation, update
`docs/v0.3.0.md` with a cohesive scope, explicit non-goals, an acceptance matrix, dependency order,
and a verification gate. Do not silently treat spread, interpolation, general iteration, normal
variadics, or broader LSP refactoring as Phase 1 cleanup.
