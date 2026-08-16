# Nocter Development Handoff

## Current Task

Continue v0.14.0 Phase 1 from the closed public grammar and the new source/syntax foundation. The
previous compiler is preserved by commit `f6c08da3` and removed from the active working tree. No
previous source, test, binary behavior, or implementation document may be used as an implementation
input.

## Immediate Work

1. Implement block imports, executable sequences, bindings, and terminal statements in G019-G021
   on the shared line-sequence and block-delimiter boundary.
2. Extend accepted, rejected, and semantic-boundary fixtures plus tree-shape snapshots with each
   newly implemented conformance row.
3. Continue through control flow and expressions in grammar dependency order. Do not introduce
   declaration or checked semantic crates during Phase 1.

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
