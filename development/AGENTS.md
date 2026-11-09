# Nocter Development Agent Rules

These rules supplement the [repository rules](../AGENTS.md) for work under `development/`.
Repository-wide principles, document placement and commit policy are owned there.

## Task Context

For development work, start from `README.md` and `TODO.md`. Before compiler changes, read
`compiler/README.md`, `architecture/overview.md` and the current contracts relevant to the task.
Use `architecture/README.md` to locate affected cross-component contracts. For unreleased
language work, read `spec/README.md` and the owning chapters; for published behavior, read the
corresponding root `../spec/` chapters. Read each affected crate's colocated README completely
before editing it. Do not require unrelated documents merely as a startup ritual.

## Behavioral Inputs

Derive language behavior from the owning version's current specification and explicitly selected
external platform standards. Derive published library behavior from its checked `std/**/index.nct`
contract and assigned behavior guide; adopted unreleased library changes come from
`development/spec/library/`. Do not claim unmigrated APIs or implementations are supported.

Do not use an archived compiler, its tests, diagnostics, generated structures, runtime behavior
or published binaries as a language oracle. Do not restore, copy, port, execute or depend on an
archived implementation, or copy a previous standard-library implementation to bootstrap the
new compiler. Git history can explain a change but cannot fill a missing current contract.
An unclosed v2.0.0 subject has no implicit fallback to the published language.

## Delegated v2.0.0 Work

The agent is authorized to decide v2.0.0 language design, source alternatives, semantics,
analysis, optimization, implementation, verification and ordinary work order. Resolve those
decisions autonomously within the user's documented goals and explicit preferences. Explain
consequential choices and tradeoffs with concrete examples, subject to the syntax approval
boundary below. Ordinary work and next tasks do not require approval.

Obtain the user's approval before a major change of syntax-design direction. This includes
replacing the basic binding/input/result/transfer notation or its user-facing meaning, changing
a pervasive default, or introducing a central source concept that substantially changes how
ordinary programs are read or written. Apply this boundary when revisiting adopted decisions too.
Clarifying or completing syntax within the adopted direction, internal models, analyses,
optimization, refactoring, tests and documentation maintenance remain delegated. Assess the
impact on the programmer's model, not merely the number of edited productions or files.

Before requesting approval, prepare a concrete proposal with representative before/after code,
the problem it solves, alternatives, consequences, migration impact and a recommendation. Keep
it clearly pending in the current design work; do not replace the adopted specification or
implement it as accepted behavior before approval. Continue independent authorized work while
that decision is pending. An explicit user adoption already provides approval for that change.

Ask for genuinely missing requirements that cannot be established from current documents or
the user's instructions, or for actions outside authorization. Release publication, replacement
of qualified artifacts, external messages and unrelated product decisions are not delegated.
Follow the root goal-confirmation policy without turning it into another approval procedure.

## Specification Closure

Audit the owning chapter and its references before deciding that behavior is unspecified or
contradictory. Reduce an unresolved semantic question to a minimal program, compare alternatives
against the documented goals, and resolve it within the delegated scope. If the user's intent
is genuinely missing, present the distinguishing example, consequences and a recommendation
when requesting clarification. Internal representation and ordinary implementation sequencing
do not require a user decision.

Write the adopted rule directly into its owning specification before implementation or
conformance admission. Keep the rationale, relevant counterexamples, proof obligations and
valid/conflicting/failure/cancellation cases with the current subject, without a parallel
decision log. Unresolved proposals must remain clearly separate from adopted contracts.
Use primary research as evidence, not authority over the project's goals.

Distinguish selected design, implemented compiler scope and verified support. Record actual
implementation limits with the responsible component and remaining work in `TODO.md`; link
instead of copying an earlier grammar or preserving a superseded specification. Planned source
fixtures are not passed production conformance. Do not silently amend the released contract.

## Compiler Boundaries

The compiler is built as an acyclic sequence of authorities:

```text
source -> syntax -> declarations and types -> checked program
       -> executable program -> MIR -> machine program -> code generation
```

Source locations are diagnostic and editor projections, never semantic identity. Semantic types
do not contain source syntax or rendered names. Dispatch is selected once, monomorphized items
form one program graph, and later stages cannot repeat earlier decisions.

Give each responsibility one owning component. Choose file, module and crate boundaries for
their actual contracts and dependency direction. Do not introduce adapters to archived concepts,
hidden fallback lookup, name-based semantic equality or independently maintained parallel indexes.

## Development Document Responsibilities

The root rules own language/library documentation placement. Within development:

- `architecture/overview.md` owns the compiler-wide pipeline and cross-crate authority boundaries.
- Other `architecture/` documents own current cross-responsibility contracts, not completed-work
  reports or duplicated crate internals.
- Each compiler crate's colocated README owns its responsibility, input/output contract,
  internal responsibility split and local invariants. Update it with responsibility changes.
- `compiler/Cargo.toml` owns workspace membership, crate manifests own exact dependencies,
  and Rust source/rustdoc owns exact APIs. Documentation links to these instead of copying lists.
- `TODO.md` owns remaining work, blockers and unresolved findings. Completed work is removed.
- `site/` owns the documentation build mechanism and website-only static inputs.

Every workspace member must have one colocated README. Update the owning architecture contract
when a cross-crate edge changes; do not repeat crate-internal details there. Remove obsolete
documents and repair referring links when an authority moves.

## Verification

Compiler verification commands live in `compiler/README.md`; evidence selection is owned by
`verification/README.md`. Run the narrowest relevant checks, then the required affected gates.
Review the changed path for competing authorities and compatibility fallbacks; tests alone do
not establish a coherent design. Do not count unconsumed editor/native paths as feature evidence.

For public Markdown changes, follow the root documentation policy with:

```sh
node development/site/test-generation.js
node development/site/build-docs.js --output /tmp/nocter-site
git diff --check
```

Internal instruction/work-plan edits require a consistency and diff review. Run link validation
when references change, and additional checks only where the change affects their subject.
