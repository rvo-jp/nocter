# Compiler Design and Maintenance Documentation

This directory contains current cross-crate compiler contracts and repository maintenance policy.
Public language, platform, CLI, diagnostic, and editor behavior belongs exclusively in
[`spec/`](../../spec/README.md). Standard-library declarations and declaration documentation belong
to each [`development/std/**/index.nct`](../std/README.md); longer observable module behavior belongs
to the colocated README. A workspace crate's private mechanism belongs in that crate's colocated
`README.md`. Implementation history and qualification evidence belong under `development/history/`,
not in current design documents.

## Reading Order

1. [Compiler Architecture](architecture.md) — pipeline, side authorities, and dependency rules.
2. The relevant [workspace crate README](../compiler/README.md#crate-documentation) — one crate's
   inputs, outputs, internal responsibilities, and invariants.
3. A cross-crate contract below when a decision spans more than one crate.
4. The [milestone index](../history/milestones/README.md) or [review index](../history/reviews/README.md) only when
   scope, implementation history, or qualification evidence is needed.

## Cross-Crate Contracts

- [Checked Program Boundary](checked-program-design.md)
- [Target, Executable, and MIR Boundary](target-program-design.md)
- [Machine Program and Native Target Boundary](machine-program-design.md)
- [Declaration Diagnostic Boundary](declaration-diagnostic-boundary.md)
- [Semantic Presentation Boundary](semantic-presentation-design.md)
- [Incremental Computation Boundary](incremental-computation-design.md)
- [Associative Collection Implementation Boundary](associative-collection-implementation.md)
- [JSON Implementation Boundary](json-implementation.md)
- [Tuple Representation Boundary](tuple-design.md)
- [Unicode Scalar Representation Boundary](unicode-scalar-design.md)
- [Static Unicode Data Boundary](unicode-text-data-design.md)

## Maintenance Contracts

- [Grammar Conformance](grammar-conformance.md)
- [Standard-Library Source Design](standard-library-source-design.md)
- [Maintenance Policy](maintenance.md)
- [Documentation Site Generator](../site/README.md)

Superseded implementation design lives under `development/history/` and is excluded from the
generated website. It must not be consulted to determine current compiler behavior.

## Information Ownership

| Information | Sole owner |
|---|---|
| Public language, platform, CLI, diagnostic, and editor behavior | `spec/` |
| Exact standard-library declarations and declaration documentation | each `development/std/**/index.nct` |
| Longer observable standard-library module behavior | the colocated `development/std/**/README.md` |
| Published versions and downloads | root `releases/` |
| Compiler pipeline and cross-crate dependency direction | `architecture.md` |
| One crate's responsibility, internal modules, and invariants | that crate's `README.md` |
| Exact Rust workspace membership and dependency edges | `development/compiler/Cargo.toml` and crate manifests |
| Exact Rust API | Rust source and rustdoc |
| Milestone scope and completion gates | `development/history/milestones/` |
| Review findings and remediation evidence | `development/history/reviews/` |
| Next concrete work and blockers | `development/TODO.md` |
| Published-candidate qualification evidence | `development/history/release-audits/` |
| Website build mechanism and static inputs | `development/site/` |
| Generated website output | `docs/` |

Do not copy a crate's module layout into a cross-crate document. Do not copy milestone progress into
architecture. A cross-crate document may name the contracts on both sides of a boundary, but it must
not explain either side's private implementation.
