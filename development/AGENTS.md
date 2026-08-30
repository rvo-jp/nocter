# Nocter Development Agent Rules

These rules apply to work under `development/` on the active specification-first compiler.

## Session Start

Before compiler work, read:

- `../README.md`
- `../spec/README.md`
- `README.md`
- `TODO.md`
- `compiler/README.md`
- `milestones/README.md`
- `milestones/v0.21.0.md`
- `docs/README.md`
- `docs/architecture.md`
- `docs/maintenance.md`

Before editing a compiler crate, also read that crate's colocated `README.md` completely.

Run `git status --short` before editing. Preserve unrelated user changes and never stage, revert, or
rewrite them.

## Historical Isolation

The compiler preserved by commit `f6c08da3` is historical evidence, not a design input. In current
development:

- do not read, restore, copy, port, execute, or depend on the archived compiler source
- do not use archived compiler tests, diagnostics, generated structures, or runtime behavior as a
  language oracle
- do not infer missing behavior from published binaries or historical implementation documents
- do not copy the previous standard-library implementation to bootstrap compiler behavior
- derive public behavior only from `spec/` and external platform standards explicitly cited by it

Historical milestone and release records may be consulted only for release-history work explicitly
requested by the user. They are never evidence for language semantics or new compiler structure.

## Specification Closure

Ask the user only when the specification still permits at least two materially different
user-visible behaviors after the owning chapter, cross-references, and design principles have been
audited. Do not ask about internal representation, implementation order, unfixed diagnostic
wording, naming cleanup, or a consequence mechanically implied by an adopted rule. Do not ask the
user merely to confirm a recommendation.

When a genuine observable ambiguity remains:

1. identify the exact conflicting or incomplete normative text
2. provide a minimal Nocter program that distinguishes the alternatives
3. compare the alternatives against the design principles
4. recommend one alternative with concrete consequences
5. ask the user to decide
6. record the decision in English in the owning `spec/` chapter
7. derive valid, boundary, and invalid conformance cases from the adopted rule

Internal choices that cannot affect accepted programs, diagnostics required by the specification,
observable execution, ABI, CLI behavior, or editor contracts do not require a language decision.

## Compiler Boundaries

The compiler is built as an acyclic sequence of authorities:

```text
source -> syntax -> declarations and types -> checked program
       -> executable program -> MIR -> machine program -> code generation
```

Source locations are diagnostic and editor projections, never semantic identity. Semantic types do
not contain source syntax or rendered names. Dispatch is selected once, monomorphized items form one
program graph, and later stages cannot repeat earlier decisions.

Create a focused file or crate for every new responsibility. Do not introduce compatibility
adapters to archived concepts, fallback lookup, name-based semantic equality, or parallel indexes.

## Documentation Ownership

- `spec/`: sole normative source for language, standard-library API, CLI, diagnostics, and editor
  behavior
- `development/milestones/v0.21.0.md`: active design scope and completed phase records
- `development/releases/v0.19.0.md`: latest published release evidence
- `development/docs/architecture.md`: compiler-wide pipeline, dependency direction, and cross-crate
  authority boundaries only
- `development/compiler/crates/<crate>/README.md`: that crate's responsibility, input/output
  contract, internal responsibility split, and local invariants
- `development/docs/*.md`: cross-crate contracts and completed design records; never a duplicate
  owner of crate internals
- `development/TODO.md`: next concrete work and current blockers only
- `development/milestones/` and `development/reviews/`: plans, historical rationale, findings, and
  remediation evidence; never current crate-internal authority
- `development/releases/`: immutable published release evidence only

Every workspace member must have one colocated `README.md`. `development/compiler/Cargo.toml` owns
workspace membership, crate manifests own exact dependencies, and Rust source/rustdoc owns exact
APIs. Do not duplicate those lists in prose. When a crate's internal responsibility changes, update
its README in the same commit. When only a cross-crate edge changes, update architecture or the
owning boundary document instead.

Write public documentation in English. Edit source Markdown and regenerate the website with
`node docs/build-docs.js`.

## Verification

Documentation checkpoints must run:

```sh
node docs/build-docs.js
git diff --check
```

Compiler verification commands live in `compiler/README.md`. Run the narrowest authoritative test
before the complete gate.

## Commit Checkpoints

Commit each coherent verified boundary. A phase is not complete while an older authority or a
temporary compatibility path remains. Passing tests do not replace an adversarial authority audit.
