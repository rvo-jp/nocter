# Nocter Development Handoff

## Current Task

Begin v0.14.0 Phase 2 from the completed source/syntax boundary. The previous compiler is preserved
by commit `f6c08da3` and removed from the active working tree. No previous source, test, binary
behavior, or implementation document may be used as an implementation input.

## Immediate Work

1. Define semantic ID domains and crate dependency boundaries for packages, modules, declarations,
   callable headers, generic parameters, bodies, and structural types. Source ranges and rendered
   names must remain projections rather than identity.
2. Lower package and module surfaces into one deterministic compile-unit declaration graph without
   resolving body expressions or introducing checked semantics.
3. Intern structural types and resolve declaration headers, aliases, associated declarations, and
   generic requirements. Record semantic-boundary diagnostics independently from syntax recovery.

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
