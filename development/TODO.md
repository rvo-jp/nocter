# Nocter Development Handoff

## Current Task

Begin v0.14.0 Phase 2 from the completed source/syntax boundary. The previous compiler is preserved
by commit `f6c08da3` and removed from the active working tree. No previous source, test, binary
behavior, or implementation document may be used as an implementation input.

## Immediate Work

1. Extend the immutable `DeclarationProgram` spine with complete declaration, member, generic,
   callable, requirement, and body arenas derived from the specification.
2. Add a syntax-to-declaration lowering crate that canonicalizes package/module/declaration input,
   then freezes one declaration program and its separate `SourceIndex` without checking bodies.
3. Resolve declaration headers, aliases, associated declarations, and generic requirements into
   the existing structural type store. Record semantic-boundary diagnostics independently from
   syntax recovery.

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
