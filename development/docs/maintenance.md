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

| Information | Owner |
|---|---|
| Language and public standard-library semantics | `spec/` |
| Active milestone completion and priorities | `docs/v0.3.0.md` |
| Released v0.2.0 completion record | `docs/v0.2.0.md` |
| Compiler phase boundaries | `docs/architecture.md` |
| Region, provenance, and allocation-context design | `docs/region-provenance.md` |
| Allocator, ownership, and drop invariants | `docs/allocator-ownership.md` |
| Distributed standard-library implementation | `docs/standard-library.md` |
| LSP capabilities and analysis design | `docs/lsp.md` |
| Next task and handoff facts | `TODO.md` |
| Historical sequence | Git history |

Do not keep the same status table in multiple documents. Keep the active checklist only in
`v0.3.0.md`; focused documents own design and concrete acceptance behavior. `v0.2.0.md` is a
released record and does not receive new work items.

## Update Triggers

- Active gate, non-goal, or work order changed: update `v0.3.0.md`.
- Region, provenance, allocation-effect, or callable-summary design changed: update
  `region-provenance.md`.
- Compiler module ownership or phase data flow changed: update `architecture.md`.
- Allocation, drop, or collection invariant changed: update `allocator-ownership.md`.
- Runtime behavior in tracked `development/std` changed: update `standard-library.md`.
- Editor-facing capability or analysis API changed: update `lsp.md`.
- Next concrete task, blocker, or uncommitted state changed: update `TODO.md`.

Do not append command logs, commit lists, or a chronology of completed items. Replace only the facts
needed for current decisions.

## Public Documentation

Public-facing documentation is written in English. The repository-level `AGENTS.md` defines the
scope and the exceptions for internal files. Edit source Markdown and regenerate the website with
`node docs/build-docs.js`.

## Verification

Run the standard verification for commits that change shared compiler behavior from the repository
root:

```sh
./development/compiler/scripts/verify.sh
cargo fmt --manifest-path development/compiler/Cargo.toml --check
git diff --check
```

Run a narrow test first, then the complete verification. A standard-library runtime promotion needs
a distributed-home or CLI run test. LSP behavior needs an analysis unit test and a JSON-RPC
integration test.

For documentation-only changes, at minimum check links and paths, inspect Markdown structure, run
`node docs/build-docs.js`, and run `git diff --check`.

## Commit Checkpoints

- Keep a behavior change, its tests, and its documentation in one coherent commit.
- Keep pure refactors separate from behavior promotion.
- Do not stage, revert, or format unrelated user changes.
- Commit each coherent verified chunk without waiting for a long session to end.
- If verification cannot run, record why in the final response and, when relevant, in `TODO.md`.

Commit messages describe the result. Do not use chronological notes such as “continue.”
