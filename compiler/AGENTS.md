# Nocter Compiler Agent Rules

These rules apply to work under `compiler/`.
They are written for long-running development across multiple AI sessions.

## Session Start

Before making compiler changes, read:

- `README.md`
- `TODO.md`
- `docs/README.md`
- `docs/architecture.md`
- `docs/implementation-status.md`
- `docs/v0-closure.md`
- `docs/roadmap.md`
- `docs/maintenance.md`

Run `git status --short` and identify unrelated user changes before editing.
Do not stage, revert, or rewrite unrelated local changes.

## Engineering Priority

Prefer long-term maintainability over short diffs.
Do not keep adding logic to a busy file when a clearer module boundary exists.

Use responsibility and abstraction layer as the split criteria:

- `ast/` owns syntax tree data.
- `resolve/` owns imports, scopes, symbols, and name lookup.
- `typecheck/` owns type rules and type diagnostics.
- `ir/` owns lower-level compiler representation.
- `abi/` owns data layout and call/return classification.
- `target/` owns target-specific code generation and binary output.
- `diagnostics/` owns structured diagnostics and rendering.
- `driver/` owns CLI and protocol entry points.
- `driver/lsp/` owns editor protocol behavior, but must reuse compiler analysis instead of reimplementing language semantics.

When a new concept does not fit one of the existing responsibilities, create or propose a focused module before adding broad logic to an existing file.

## Refactoring Policy

Refactoring is allowed work, not cleanup to postpone indefinitely.

Use these triggers to refactor before feature work continues:

- one file mixes protocol handling, semantic analysis, and feature-specific presentation
- one function is forced to know details from multiple compiler phases
- adding a feature requires copying AST traversal or symbol lookup logic
- tests need excessive setup because production code has no narrow API
- a module name no longer describes most of its contents

Keep behavior changes and pure structure changes in separate commits when practical.
If they must be combined, document why in the final response.

## Documentation Updates

At the end of a session, update the smallest relevant document set:

- `TODO.md`: short-term handoff state, known unrelated local changes, next concrete task
- `docs/implementation-status.md`: user-visible implementation capability changed
- `docs/v0-closure.md`: v0 completion gate or ship/reject/defer decision changed
- `docs/std-runtime-status.md`: distributed standard-library runtime behavior changed
- `docs/roadmap.md`: current priority or recommended next task changed
- `docs/architecture.md`: module responsibility or pipeline design changed
- `docs/maintenance.md`: long-lived development policy changed

Do not append chronological logs to long-lived design documents.
Record facts that help the next session make better decisions.

## Verification

For Rust compiler changes, run the narrowest sufficient checks.
Prefer `./compiler/scripts/verify.sh` before commits that touch shared compiler behavior.

When verification cannot be run, record the reason in the final response and in `TODO.md` if it affects the next session.

Always report:

- what changed
- what was verified
- what remains uncommitted
- which unrelated files were intentionally left alone

## Commit Checkpoints

When a coherent chunk of work is complete, verified, and no user-owned unrelated changes are mixed into the staged set, create a git commit before continuing to the next chunk.
Do this especially after changes that update multiple compiler phases, add a new module, or change user-visible behavior.
Keep the commit scoped to the completed chunk; leave unrelated local changes unstaged.
