# Nocter Development Handoff

## Current Task

Begin v0.14.0 Phase 2 from the completed source/syntax boundary. The previous compiler is preserved
by commit `f6c08da3` and removed from the active working tree. No previous source, test, binary
behavior, or implementation document may be used as an implementation input.

## Immediate Work

1. Normalize the completed bound header-type arena into the structural type store. Resolve alias
   applications and associated selections together with their requirement environment; do not add
   either as a temporary canonical `TypeKind`. Package/module/source topology, surface inventory,
   cross-file callable joining, recursive identity reservation, names, visibility, generic binders,
   authored imports/re-exports, prelude fallback, lexical type-name binding, and declaration target
   pattern binding are complete.
2. Define declaration headers, associated declarations, callable provenance, and generic
   requirements, then freeze the declaration program and separate `SourceIndex`.
3. Record semantic-boundary diagnostics independently from syntax recovery and add ordering-
   permutation tests for the complete lowering result.

## Guardrails

- Do not restore or inspect the archived compiler.
- Do not migrate archived tests or diagnostics.
- Do not run a released compiler to discover unspecified behavior.
- Do not treat the existing standard-library implementation as language semantics.
- Do not mark specification closure complete while an observable choice remains implicit.
- Do not let Phase 2 revisit parser ambiguity, infer syntax from resolved names, or place source
  ranges and rendered names in semantic identity.

## Verification

```sh
cargo fmt --manifest-path development/compiler/Cargo.toml --all --check
cargo clippy --manifest-path development/compiler/Cargo.toml --workspace --all-targets -- -D warnings
cargo test --manifest-path development/compiler/Cargo.toml --workspace
node docs/build-docs.js
git diff --check
```
