# Development Maintenance

This document contains long-lived maintenance policy. Keep short-term handoff state in
[TODO](../TODO.md) and public language rules in the [specification](../../spec/README.md).

## Design Rules

- Prefer coherent responsibilities and easy future changes over small diffs.
- Split by responsibility and abstraction layer, not by line count.
- Prefer narrow APIs that return purpose-specific owned results over callers exploring internal maps
  or mutable state.
- Do not mix compiler phases, protocol transport, and presentation in one file.
- Extract a shared responsibility when a change would duplicate AST traversal, lookup, type
  formatting, or drop logic.
- Do not add compatibility shims for removed repository locations or unpublished behavior.

Put a new responsibility in a new module or file. Add it to an existing file only when the existing
responsibility naturally explains it.

## Sources of Truth

The single ownership map lives in the [implementation documentation index](README.md#information-ownership).
Do not reproduce that table or candidate status in maintenance policy.

## Update Triggers

- Candidate scope, qualification, non-goal, or publication state changed: update the matching file
  under `../milestones/`.
- Region, provenance, allocation-effect, or callable-summary design changed: update
  `region-provenance.md`.
- Compiler module ownership or phase data flow changed: update `architecture.md`.
- Allocation, drop, or collection invariant changed: update `allocator-ownership.md`.
- Iterator ownership, element access, or transient shifting changed: update `iteration.md`.
- Capability-set, conditional-conformance, iterator-adapter, or collection-builder design changed:
  update `iterator-composition.md`.
- Method-generic, default-method, closure, callable specialization, or chain API design changed:
  update `callable-default-methods.md`.
- Construction lowering, default-selection model, or editor projection changed: update
  `construction-surfaces.md`.
- Interpolation runtime binding, evaluation, formatting, or cleanup changed: update
  `interpolation.md`.
- Runtime representation, primitive binding, or ownership invariant in tracked `development/std`
  changed: update `standard-library.md`.
- Editor-facing capability or analysis API changed: update `lsp.md`.
- Next concrete task, blocker, or uncommitted state changed: update `TODO.md`.

If source behavior or a public API changes, update the owning specification chapter. Development
documents may explain how the compiler implements that rule but must not restate the rule as a
second contract.

Do not append command logs, commit lists, or a chronology of completed items. Replace only the facts
needed for current decisions.

## Public Documentation

Public-facing documentation is written in English. The repository-level `AGENTS.md` defines the
scope and the exceptions for internal files. Edit source Markdown and regenerate the website with
`node docs/build-docs.js`.

## Documentation Placement

- Keep the repository-root `README.md` focused on released-product overview, installation, use, and
  links to deeper documentation.
- Keep language and public standard-library semantics in `spec/`.
- Keep contributor setup, repository-local commands, compiler architecture, implementation design,
  milestone plans, maintenance policy, and handoff state under `development/`.
- Use `development/README.md` as the single root-to-development entry point. Do not duplicate
  developer instructions or active milestone status in root documentation.

## Verification

Run the standard verification for commits that change shared compiler behavior from the repository
root:

```sh
./development/compiler/scripts/verify.sh
cargo fmt --manifest-path development/compiler/Cargo.toml --check
git diff --check
```

Run a narrow test first, then the complete verification. The
[test ownership policy](testing.md) selects the narrowest authoritative layer and limits
distributed-home and framed-LSP coverage to representative integration boundaries.

For documentation-only changes, at minimum check links and paths, inspect Markdown structure, run
`node docs/build-docs.js`, and run `git diff --check`.

## Commit Checkpoints

- Keep a behavior change, its tests, and its documentation in one coherent commit.
- Keep pure refactors separate from behavior promotion.
- Do not stage, revert, or format unrelated user changes.
- Commit each coherent verified chunk without waiting for a long session to end.
- If verification cannot run, record why in the final response and, when relevant, in `TODO.md`.

Commit messages describe the result. Do not use chronological notes such as “continue.”
