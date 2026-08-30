# Compiler Design and Maintenance Documentation

This directory contains current cross-crate compiler contracts and repository maintenance policy.
Public language and standard-library behavior belongs exclusively in
[`spec/`](../../spec/README.md). A workspace crate's private mechanism belongs in that crate's
colocated `README.md`. Implementation history and qualification evidence belong in reviews and
milestones, not in current design documents.

## Reading Order

1. [Compiler Architecture](architecture.md) — pipeline, side authorities, and dependency rules.
2. The relevant [workspace crate README](../compiler/README.md#crate-documentation) — one crate's
   inputs, outputs, internal responsibilities, and invariants.
3. A cross-crate contract below when a decision spans more than one crate.
4. The [milestone index](../milestones/README.md) or [review index](../reviews/README.md) only when
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

## Maintenance Contracts

- [Grammar Conformance](grammar-conformance.md)
- [Standard-Library Source Design](standard-library-source-design.md)
- [Maintenance Policy](maintenance.md)
- [Documentation Site Generation](site-generation.md)

Superseded implementation design lives under `development/archive/` and is excluded from the
generated website. It must not be consulted to determine current compiler behavior.

## Information Ownership

| Information | Sole owner |
|---|---|
| Public language, standard-library, CLI, diagnostic, and editor behavior | `spec/` |
| Published versions and downloads | root `releases/` |
| Compiler pipeline and cross-crate dependency direction | `architecture.md` |
| One crate's responsibility, internal modules, and invariants | that crate's `README.md` |
| Exact Rust workspace membership and dependency edges | `development/compiler/Cargo.toml` and crate manifests |
| Exact Rust API | Rust source and rustdoc |
| Milestone scope and completion gates | `development/milestones/` |
| Review findings and remediation evidence | `development/reviews/` |
| Next concrete work and blockers | `development/TODO.md` |
| Published-candidate qualification evidence | `development/releases/` |

Do not copy a crate's module layout into a cross-crate document. Do not copy milestone progress into
architecture. A cross-crate document may name the contracts on both sides of a boundary, but it must
not explain either side's private implementation.
