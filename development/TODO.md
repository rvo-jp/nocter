# Nocter Development Handoff

## Current Task

Continue v0.14.0 Phase 1 from the closed public grammar and the new source/syntax foundation. The
previous compiler is preserved by commit `f6c08da3` and removed from the active working tree. No
previous source, test, binary behavior, or implementation document may be used as an implementation
input.

## Immediate Work

1. Complete nominal declarations and member containers in G007-G012, reusing the existing
   declaration/type/requirement parser rather than introducing parallel member parsers.
2. Complete the type and requirement boundaries in G013-G018, including accepted, rejected, and
   semantic-boundary coverage not already exercised by callable declarations.
3. Expand G001 onward from the
   [grammar conformance plan](docs/grammar-conformance.md) into accepted and rejected parser
   fixtures and stable tree-shape snapshots; keep semantic-boundary cases parseable.
4. Add expression and block parsing only after the declaration/type productions they consume are
   structurally stable. Do not introduce declaration or checked semantic crates during Phase 1.

## Guardrails

- Do not restore or inspect the archived compiler.
- Do not migrate archived tests or diagnostics.
- Do not run a released compiler to discover unspecified behavior.
- Do not treat the existing standard-library implementation as language semantics.
- Do not create the new compiler workspace before the grammar closure gate.
- Do not mark specification closure complete while an observable choice remains implicit.

## Verification

```sh
cargo fmt --manifest-path development/compiler/Cargo.toml --all --check
cargo clippy --manifest-path development/compiler/Cargo.toml --workspace --all-targets -- -D warnings
cargo test --manifest-path development/compiler/Cargo.toml --workspace
node docs/build-docs.js
git diff --check
```
