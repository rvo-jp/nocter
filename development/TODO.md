# Nocter Development Handoff

## Current Task

Nocter v0.14.0 is published and frozen. v0.15.0 Phase 0 through Phase 3 are complete. Phase 0 removed
semantic authority from `SourceIndex`, closed concrete dispatch over checked evidence, replaced the
backend-visible semantic `TypeStore` with a runtime type contract, and gave shared ABI facts one
owner. Phase 1 separated direct-only physical-source `include` from directory-module `use`, made
`index.nct` the sole public contract, and centralized exact-source private access. Phase 2 added
explicit, separable interface defaults and migrated the standard library to contract-first module
roots. Phase 3 generalized the compiler-owned sequence pack into typed callable argument packs,
preserving one ownership, provenance, cleanup, and hidden-ABI pipeline. The exact scope and
completion records are in `development/milestones/v0.15.0.md`.

The active specification-first compiler is under `development/compiler/`. The previous
compiler was preserved by commit `f6c08da3` and removed from the active tree. Do not inspect or use
the archived implementation, its tests, released binaries, or historical output as implementation
input.

## Current Baseline

- The source, syntax, declaration, checked-program, MIR, machine, ARM64, Mach-O, package, command,
  formatter, standard-library, and editor phases are implemented through the production boundaries
  described in `development/milestones/v0.14.0.md`.
- Source discovery, declaration namespaces, private semantic access, and editor recovery now keep
  exact-source includes separate from module imports. Public contracts and private definitions join
  under reciprocal direct includes without widening implementation visibility.
- Editor queries consume one immutable generation. Hover and semantic tokens share one deterministic
  source-binding authority; semantic ranges, cursor containment, containment, and overlap belong to
  `nocter-source`.
- Compiler-owned quick fixes cover imports, required conformance methods, and optional/fallible
  callable result contracts. Every edit is applied to an isolated overlay and must pass ordinary
  full-package compilation before publication.
- Every public single-file example executes as a native process with exact status, stdout, and
  stderr checks. The public `file-summary` package executes with a real file argument.
- ARM64 string-to-pointer copy now applies the authored destination offset. A native primitive
  conformance case and `custom-format.nct` output test protect the fix.
- Every completed v0.15.0 phase passed its focused tests and complete workspace, Clippy, formatting,
  generated-documentation, and repository-integrity gates. Exact completion evidence belongs to
  `development/milestones/v0.15.0.md`; the v0.14.0 clean-build record remains in its qualification
  document.
- Warnings-denied workspace Clippy, Rust formatting, documentation regeneration, and repository
  whitespace checks passed. The clean build used a temporary external target directory that was
  removed afterward.
- The completion-criterion trace and named executable evidence are recorded in
  `development/milestones/v0.14.0-qualification.md`.

## Completed Review Work

1. Versionless compiler source overrides are separate from versioned editor documents.
2. Runtime contracts have one source-independent owner; declaration lowering alone projects syntax
   primitive bindings to semantic identities.
3. MIR publishes a closed backend environment, and machine cannot reach target or checked storage.
4. Executable semantic queries replace MIR navigation through provider storage.
5. Convenience re-exports no longer obscure contract ownership.
6. The adversarial second review found no remaining boundary violation, and incremental plus clean
   external-target qualification passed.

The post-release v0.15.0 audit found that items 2 and 3 were weaker than their wording: checking
still used `SourceIndex` as input authority, and the MIR environment still exposed semantic
`TypeStore`. Phase 0 replaced those boundaries rather than preserving the inaccurate claim. Its
final review also removed an implicit presentation-to-semantics projection, lexical visibility
re-evaluation during specialization, and an unused opaque declaration identity from the runtime
contract.

## Guardrails

- `spec/` is the sole source of public language behavior.
- Ask the user only when an observable behavior remains ambiguous after reading the specification.
- Do not add compatibility fallbacks, source-text semantic inference, duplicate indexes, or reverse
  lookup from presentation strings.
- A later phase cannot import an earlier phase's private representation to repeat its decisions.
- Source order, declaration order, filesystem enumeration, and arena insertion order must not select
  between otherwise equal semantic candidates.
- Keep public documentation in English. Edit source Markdown and regenerate the website.

## Verification

```sh
cargo fmt --manifest-path development/compiler/Cargo.toml --all --check
cargo clippy --manifest-path development/compiler/Cargo.toml --workspace --all-targets -- -D warnings
cargo test --manifest-path development/compiler/Cargo.toml --workspace
node docs/build-docs.js
git diff --check
```
