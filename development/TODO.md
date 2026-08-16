# Nocter Development Handoff

## Current Task

Complete the source-backed semantic diagnostic boundary for v0.14.0 Phase 2. The previous compiler
is preserved by commit `f6c08da3` and removed from the active working tree. No previous source,
test, binary behavior, or implementation document may be used as an implementation input.

## Immediate Work

1. Audit prelude composition failures. Compiler-selected authority and inconsistent namespace state
   remain internal; any authored prelude-import rule must retain its exact syntax subject.
2. Extend the source-backed boundary to type normalization without turning malformed bound graphs
   into user-facing errors. Alias cycles and authored associated selections must retain the syntax
   subjects that selected them.
3. Derive diagnostic cases from G001-G018 semantic-boundary fixtures and verify that input ordering
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
stage failures rather than allowing callers to assemble partial pipelines. Authored module-surface
violations project `E0230`-`E0232` directly from their exact syntax subjects; invalid syntax trees
and inconsistent discovery inputs remain internal pipeline failures. Header preparation records
exact name tokens and visibility nodes in shared namespace violations before consuming temporary
surface identities, then projects `E0240`-`E0242` without reverse lookup. Authored imports reuse
those namespace rules and add `E0260`, `E0261`, and `E0412` for missing, widening, and inaccessible
selected names. Namespace bindings retain exact local-name origins, so collision and access notes
never expand to a whole `use` declaration. Source-composition violations and module cycles project
`E0270`-`E0271`; module edges retain their authored `use` nodes, and cycle selection is canonical
under compile-unit input reordering. Missing, duplicate, stale, and unreachable discovery inputs
remain internal contract errors. Generic binder declarations project `E0280`-`E0282`; lexical
scopes retain exact declaration tokens, so duplicate declarations and inherited shadowing produce
different rules and related spans without reverse syntax lookup. Repeated binders in a declaration
target pattern remain references to the first binding.
Header type binding now projects `E0290`-`E0302`. Unknown names, invalid entities and arguments,
`Self`, fixed-array lengths, callable parameter/provenance duplicates, opaque bindings, and generic
requirements retain their exact syntax subjects when their rules are selected. Invalid parser
snapshots, missing discovery state, and duplicate source-index insertion remain internal errors.

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
