# Nocter Development Handoff

## Current Task

Nocter v0.17.0 is published and externally audited. Exact source, artifact, and fresh-install
evidence belongs to [`development/releases/v0.17.0.md`](releases/v0.17.0.md); older release records
remain under `development/releases/` and are not repeated here.

The [v0.18.0 Phase 0 construction surface simplification](milestones/v0.18.0.md) and its
[adversarial review](reviews/v0.18.0-phase-0.md) are complete. The adopted source contract removes
construction `default`, makes structural construction depend only on nominal representation and
field visibility, and removes construction-surface aggregation from nominal type hover. No
compatibility syntax or retained default-member state remains. No subsequent v0.18.0 phase is
currently selected.

The active specification-first compiler is under `development/compiler/`. The implementation
removed before the v0.14.0 rewrite remains available only in Git history and must not be used as a
behavioral oracle or implementation reference.

## Current Baseline

- The source, syntax, declaration, checked-program, MIR, machine, ARM64, Mach-O, package, command,
  formatter, standard-library, and editor phases are implemented through the production boundaries
  described in `development/milestones/v0.14.0.md`.
- The built-in `error` is a one-word move-only handle to immutable owned or static runtime nodes.
  Every `T!` is move-only; construction snapshots source text, context consumes its cause, access
  borrows the handle, cleanup is iterative, and process reporting uses the same runtime schema.
- `nocter-runtime-contract` is the sole numeric error ABI authority. Standard error members are
  ordinary source-backed methods, and the editor has no synthetic error-field lookup.
- Source discovery, declaration namespaces, private semantic access, and editor recovery keep
  exact same-module source-visibility edges separate from module imports. Public contracts and
  private definitions join under reciprocal direct `see` edges without widening implementation
  visibility.
- Editor queries consume one immutable generation. Hover and semantic tokens share one deterministic
  source-binding authority; semantic ranges, cursor containment, containment, and overlap belong to
  `nocter-source`.
- Compiler-owned quick fixes cover imports, required conformance methods, and optional/fallible
  callable result contracts. Every edit is applied to an isolated overlay and must reach checked
  full-package semantics before publication; target-specific completion is not required for a
  source-semantic edit.
- Every public single-file example executes as a native process with exact status, stdout, and
  stderr checks. One shared acceptance catalog covers every public package example; `file-summary`
  and `text-report` compile through the target session and execute successful and argument-error
  scenarios as native processes.
- `std/fs` provides whole-file reads and writes, portable metadata, existence, removal, and rename.
  Stream algorithms stay in `std/io`; target paths, public I/O error policy, and Darwin ABI facts
  have separate package-internal owners. Native acceptance covers the complete Phase 0 behavior.
- Module-qualified values, body types, and constant subexpressions use one name-resolution
  authority. Checking consumes the selected identity without repeating namespace lookup, and
  callable re-exports retain their declaration identity through native lowering and editor
  projection.
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
