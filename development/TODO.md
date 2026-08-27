# Nocter Development Handoff

## Current Task

Nocter v0.17.0 is published and externally audited. Exact source, artifact, and fresh-install
evidence belongs to [`development/releases/v0.17.0.md`](releases/v0.17.0.md); older release records
remain under `development/releases/` and are not repeated here.

The [v0.18.0 Phase 0, Phase 1, Phase 2, and twice-reopened Phase 3 work](milestones/v0.18.0.md) is
implemented. The [semantic-tooling reconstruction](docs/semantic-tooling-reconstruction.md) now
closes the checking-to-workspace authority chain; release qualification is the next gate. Phase 3 replaced body-semantic mutation plus rollback with
persistent immutable authorities and explicit type, copyability, and closure transactions. Its
reopened review then separated read-only type snapshots from construction ownership, kept type and
copyability in one semantic authority through every checking stage, bound checked member input to
its owning body generation, and removed repeated structural work. Phase 2 replaced
standalone `conform` declarations with instance-owned interface implementation, aggregate
associated-binding braces, nominal `impl` requirements, and statically witnessed callable
annotations. Phase 0 simplifies construction surfaces. Phase 1 adds exact source declarations for
every named builtin and adopts `primitive func`; named inherent ownership now derives from the
selected declaration, while only anonymous slices retain structural attachment authority. The
post-phase architecture audit then removed repeated target-directive parsing, source-index-based
primitive and standard-role selection, repeated package-target validation, source projection from
prepared semantic state, duplicated built-in type vocabularies, and standard-API spelling checks
in interface-implementation code actions. Executable architecture gates now protect these
boundaries. Phase 3 changes no language behavior. The completed reconstruction replaces optional
sparse editor recovery, lossy session diagnostic composition, and feature-local semantic joins
with explicit per-domain evidence and coverage contracts. One semantic pipeline now produces every
session outcome; source projection integrity cannot fail semantics; session provides one evidence
handoff to queries; workspace revisions are complete values; and ambiguous shared-source contexts
are rejected rather than ordered.

The subsequent authority-boundary review removed the remaining representative-source input from
package analysis. Each package generation now derives one canonical module-root set from every
current scope member, and speculative edits reuse that same complete demand. Visible-name
projection conflicts are retained as `SourceProjectionIssue` values rather than normalized by
entity order. Rename and code-action publication now require a generation-borrowed semantic-mutation
capability issued only after the complete query seal succeeds. Behavioral multi-module tests and
expanded production-path architecture gates protect these boundaries. Changed files invalidate
active demand but cannot join it, and an empty scope emits an invalidation-only generation without
running the compiler pipeline.

The Phase 3 body authority is now migrated. `TypeStore` and `CopyabilityTable` are immutable;
`TypeStore` has no mutation or branch-opening API, while `TypeAuthority` owns exact type lineage.
`SemanticAuthority` keeps type and copyability ownership inseparable through preparation, body
recovery, checked completion, member queries, and concrete specialization. Stable program facts
move through one `ProgramEnvironment`; checked semantic facts and closures can be paired only by
the body-finish boundary, and preparation owns the only production type/copyability seal. Closure
construction
uses an immutable internal authority and freezes into `ClosureTable` only after body checking.
`TypeTransaction`, `CopyabilityTransaction`, and `ClosureTransaction` share one body-level
capability and commit boundary through `BodySemanticTransaction`. Their persistent vectors and
indexes share unchanged roots and copy only changed paths. Exact-base lineage rejects sibling and
stale commits. Declaration lowering, preparation, body checking, member queries, concrete
specialization, type projection, and tests use these boundaries. All semantic checkpoints,
journals, body rollback paths, and full-store recovery clones are gone. Editor query sessions
verify their composite semantic base and reject cross-generation or cross-interruption reuse;
checked completion resolves the receiver type and source visibility through its own body rather
than accepting raw cross-generation input. Downstream architecture gates allow direct
persistent-storage dependencies only in model and checking. Type-resolved Clippy rejects
construction authorities, transactions, closure construction sequences, and persistent
collections outside their reviewed owner boundaries, including uses hidden behind aliases.
Declaration and lowering exemptions are limited to their program/type-construction modules. Persistent
iteration is linear, final copyability closure scans only appended types without traversing the
closed prefix, and structural storage and concreteness facts are computed once at interning.
Closure drafts and final definitions share one immutable core. The twice-reopened adversarial
review found no remaining persistent-authority issue, but a later cross-feature audit found that
editor recovery still lacked an explicit availability, completeness, and diagnostic-causality
contract. Complete workspace
tests, warnings-denied all-target Clippy, formatting, generated documentation, and repository
whitespace validation passed on the final tree.

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
