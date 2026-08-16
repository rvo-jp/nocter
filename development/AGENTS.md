# Nocter Development Agent Rules

These rules apply to work under `development/` during the specification-first compiler rewrite.

## Session Start

Before rewrite work, read:

- `../README.md`
- `../spec/README.md`
- `README.md`
- `TODO.md`
- `compiler/README.md`
- `milestones/README.md`
- `milestones/v0.14.0.md`
- `docs/architecture.md`
- `docs/maintenance.md`

Run `git status --short` before editing. Preserve unrelated user changes and never stage, revert, or
rewrite them.

## Specification-First Isolation

The compiler preserved by commit `f6c08da3` is historical evidence, not a design input. During the
rewrite:

- do not read, restore, copy, port, execute, or depend on the archived compiler source
- do not use archived compiler tests, diagnostics, generated structures, or runtime behavior as a
  language oracle
- do not infer missing behavior from published binaries or historical implementation documents
- do not copy the previous standard-library implementation to bootstrap compiler behavior
- derive public behavior only from `spec/` and external platform standards explicitly cited by it

Historical milestone and release records may be consulted only for release-history work explicitly
requested by the user. They are never evidence for language semantics or new compiler structure.

## Specification Closure

Do not guess when the specification permits materially different user-visible behavior. Instead:

1. identify the exact conflicting or incomplete normative text
2. provide a minimal Nocter program that distinguishes the alternatives
3. compare the alternatives against the design principles
4. recommend one alternative with concrete consequences
5. ask the user to decide
6. record the decision in English in the owning `spec/` chapter
7. derive valid, boundary, and invalid conformance cases from the adopted rule

Internal choices that cannot affect accepted programs, diagnostics required by the specification,
observable execution, ABI, CLI behavior, or editor contracts do not require a language decision.

## New Compiler Boundaries

The new compiler is built as an acyclic sequence of authorities:

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
- `development/milestones/v0.14.0.md`: rewrite scope and completion gates
- `development/docs/architecture.md`: new compiler dependency and authority boundaries
- `development/TODO.md`: short-lived handoff and unresolved work
- `development/releases/`: immutable published release evidence only

Write public documentation in English. Edit source Markdown and regenerate the website with
`node docs/build-docs.js`.

## Verification

Until the new Cargo workspace exists, documentation checkpoints must run:

```sh
node docs/build-docs.js
git diff --check
```

Once a compiler verification entry point is introduced, document it in `compiler/README.md` and
run the narrowest authoritative test before the complete gate.

## Commit Checkpoints

Commit each coherent verified boundary. A phase is not complete while an older authority or a
temporary compatibility path remains. Passing tests do not replace an adversarial authority audit.
