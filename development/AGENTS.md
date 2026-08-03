# Nocter Development Agent Rules

These rules apply to work under `development/` and long-running compiler sessions.

## Session Start

Before compiler changes, read:

- `../README.md`
- `../spec/README.md`
- `README.md`
- `TODO.md`
- `docs/README.md`
- `docs/v0.3.0.md`
- `docs/architecture.md`
- `docs/region-provenance.md`
- the focused design document for the area being changed
- `docs/maintenance.md`

Run `git status --short` before editing. Preserve unrelated user changes and never stage, revert, or
rewrite them.

## Recorded Milestone

v0.2.0 is released and complete. v0.3.0 Phase 0, Phase 1 Typed Literal Core, Phase 2 Explicit
Iteration and Collection Access, Phase 3 Owned String Interpolation and Formatting, and Phase 4
Public Provenance Contracts and Generic Interface Bounds are complete on `develop`, as are Phase 5
through Phase 9. Phase 10 Callable Values and Interface Default Methods is active. Do not use `v0` as
shorthand for a release scope. Use
[docs/v0.3.0.md](docs/v0.3.0.md) for current milestone status and
[docs/v0.2.0.md](docs/v0.2.0.md) only for the released baseline.

## Engineering Priority

Prefer long-term maintainability over small diffs. Do not add logic to a busy file when a focused
module gives the responsibility a stable name and API.

Inside `compiler/src/`:

- `ast/` owns syntax tree data.
- `resolve/` owns imports, scopes, symbols, visibility, and name lookup.
- `typecheck/` owns type, generic, ownership, borrow, and drop semantics.
- `analysis/` exposes compiler-backed query results for tooling.
- `ir/` owns explicit lower-level compiler representation.
- `abi/` owns data layout and call/return classification.
- `backend/` and `target/` own code generation and binary output.
- `diagnostics/` owns structured diagnostics and rendering.
- `driver/` owns CLI and protocol orchestration.
- `driver/lsp/` owns editor protocol behavior and must reuse `analysis` facts.

When a new concept does not fit an existing responsibility, create or propose a focused module before
adding broad logic to an existing file.

## Refactoring Policy

Refactor before feature work when:

- one file mixes transport, semantic analysis, and presentation
- one function must know details from several compiler phases
- a feature would copy AST traversal, lookup, type formatting, or drop logic
- tests require full-pipeline setup because no narrow production API exists
- a module name no longer describes most of its contents

Keep structural changes and behavior changes in separate commits when practical.

## Documentation Updates

Update only the owner of the changed fact:

- `TODO.md`: next concrete task, blockers, uncommitted handoff state
- `docs/v0.3.0.md`: current milestone status, completion gate, scope, non-goal, or work order
- `docs/v0.2.0.md`: released v0.2.0 completion record
- `docs/architecture.md`: phase/module responsibility or data flow
- `docs/region-provenance.md`: region, storage-origin, allocation-effect, or callable-summary design
- `docs/typed-literals.md`: literal shape, definition, element-pack, or per-literal context design
- `docs/iteration.md`: readonly/owned iteration, element access, transient shift, or iterator LSP
  design
- `docs/iterator-composition.md`: capability sets, conditional conformance, adapters, or collection
  builder design
- `docs/callable-default-methods.md`: method generics, interface default methods, closure ownership,
  callable specialization, or chainable iterator design
- `docs/interpolation.md`: interpolation runtime binding, formatting, evaluation, cleanup, or LSP
  design
- `docs/provenance-contracts.md`: explicit result-origin contracts, interface bounds, static bound
  dispatch, or their editor integration
- `docs/allocator-ownership.md`: allocator, ownership, drop, String/Vec invariants
- `docs/standard-library.md`: distributed std runtime behavior
- `docs/lsp.md`: editor capability or compiler-analysis contract
- `docs/maintenance.md`: long-lived engineering policy

Do not append chronological logs or commit lists to design documents. Git owns history.

## Verification

Use the narrowest test that proves the change, then prefer
`./development/compiler/scripts/verify.sh` before commits that touch shared compiler behavior.
Always run `git diff --check`.

Report what changed, what was verified, what remains uncommitted, and which unrelated files were left
alone. If required verification cannot run, record the reason in the final response and in `TODO.md`
when it affects the next session.

## Commit Checkpoints

Commit each coherent verified chunk before continuing. Keep unrelated local changes unstaged. Prefer
one behavior change plus tests and docs, or one behavior-preserving structural refactor.
