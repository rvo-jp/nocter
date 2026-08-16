# Nocter Development Handoff

## Current Task

Complete the source-backed semantic diagnostic boundary for v0.14.0 Phase 2. The previous compiler
is preserved by commit `f6c08da3` and removed from the active working tree. No previous source,
test, binary behavior, or implementation document may be used as an implementation input.

## Immediate Work

1. Extend the common source-backed diagnostic envelope from the completed contract and freeze-time
   declaration rules to surface, header, generic, import, and type-binding passes. Internal
   consistency faults remain distinct and must not receive user-facing error codes.
2. Derive diagnostic cases from G001-G018 semantic-boundary fixtures and verify that input ordering
   cannot change the selected code, semantic subject, primary range, or related range.

Complete foundations: the lowering boundary now defines every reserved declaration, member,
parameter, requirement, body, and opaque-result identity from the normalized type graph; separates
authored and inferred callable provenance; projects contract and implementation sources onto shared
semantic identities; records exact standard-package and built-in attachment authority; validates
the frozen graph; and returns only an immutable `DeclarationProgram` plus `SourceIndex`.
Freeze-time authored-rule failures now carry stable `E0200`-`E0212` codes, exact primary and related
declaration-site identities, correction guidance, and source projections. Malformed compiler graph
errors remain a separate internal integrity category. Callable contract failures now project
`E0250`-`E0253` through the same diagnostic envelope before semantic reservation. The production
`lower_compile_unit_declarations` facade owns the only complete pass ordering and returns typed
stage failures rather than allowing callers to assemble partial pipelines.

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
