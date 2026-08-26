# Nocter Development Handoff

## Current Task

Nocter v0.17.0 is published and externally audited. Exact source, artifact, and fresh-install
evidence belongs to [`development/releases/v0.17.0.md`](releases/v0.17.0.md); older release records
remain under `development/releases/` and are not repeated here.

The [v0.18.0 Phase 0, Phase 1, and Phase 2 work](milestones/v0.18.0.md) and their adversarial reviews
are complete. Phase 3 is active and replaces body-semantic mutation plus rollback with persistent
immutable authorities and explicit type, copyability, and closure overlays. Phase 2 replaced
standalone `conform` declarations with instance-owned interface implementation, aggregate
associated-binding braces, nominal `impl` requirements, and statically witnessed callable
annotations. Phase 0 simplifies construction surfaces. Phase 1 adds exact source declarations for
every named builtin and adopts `primitive func`; named inherent ownership now derives from the
selected declaration, while only anonymous slices retain structural attachment authority. The
post-phase architecture audit then removed repeated target-directive parsing, source-index-based
primitive and standard-role selection, repeated package-target validation, source projection from
prepared semantic state, duplicated built-in type vocabularies, and standard-API spelling checks
in interface-implementation code actions. Executable architecture gates now protect these
boundaries. Phase 3 changes no language behavior; release qualification resumes only after its
persistent-authority completion gate and final review pass.

The Phase 3 body authority is now migrated. `TypeStore` and `CopyabilityTable` are immutable;
closure construction uses an immutable internal authority and freezes into `ClosureTable` only
after body checking. `TypeTransaction`, `CopyabilityTransaction`, and `ClosureTransaction` share
one body-level capability and commit boundary through `BodySemanticTransaction`. Their persistent
vector, arena, and indexes share unchanged roots and copy only changed paths. Exact-base lineage
rejects sibling and stale commits. Declaration lowering, preparation, body checking, member
queries, concrete specialization, type projection, and tests use these boundaries. All semantic
checkpoints, journals, body rollback paths, and full-store recovery clones are gone. Editor query
sessions now verify their exact type and copyability bases, reject cross-generation or
cross-interruption reuse, and have checked plus recovery tests proving repeatable completion without
mutating accepted types. The next step is to audit downstream dependency and API boundaries before
Phase 3 qualification.

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
- Compiler-owned quick fixes cover imports, required interface methods, and optional/fallible
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
- Named builtin types enter discovery, namespaces, checking, documentation, hover, highlighting,
  and navigation through one exact `primitive type` declaration identity. The compiler retains
  canonical representation, but no consumer reconstructs source authority from a spelling or path.
- Target directives are decoded into one typed `TargetSelection` during discovery and carried into
  declaration lowering. Standard declaration roles are resolved once into `StandardLibrary`, while
  primitive roles are resolved once from `FrontendBindings`; neither path reads `SourceIndex`.
- Prepared and checked semantic programs exclude source projection. Their output or recovery
  boundaries retain `SourceIndex` beside the semantic program, never inside it.
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
