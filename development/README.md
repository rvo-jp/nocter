# Contributor Documentation

Nocter has published v0.16.0 with a compiler built from the language specification. The v0.13.0
compiler implementation was not an input to the rewrite.

The rewrite closed each normative gap in [`spec/`](../spec/README.md) before implementing the
corresponding syntax, static semantics, dynamic semantics, target behavior, or tooling contract.

## Current Work

- [Current handoff](TODO.md)
- [v0.17.0 practical application foundations](milestones/v0.17.0.md)
- [v0.16.0 practical failure values](milestones/v0.16.0.md)
- [v0.16.0 release preparation](milestones/v0.16.0-release-preparation.md)
- [v0.16.0 publication and audit](releases/v0.16.0.md)
- [Compiler architecture](docs/architecture.md)
- [Checked program design](docs/checked-program-design.md)
- [Target and executable program design](docs/target-program-design.md)
- [Machine program and native target design](docs/machine-program-design.md)
- [Declaration diagnostic boundary](docs/declaration-diagnostic-boundary.md)
- [Semantic presentation design](docs/semantic-presentation-design.md)
- [Grammar conformance plan](docs/grammar-conformance.md)
- [Maintenance policy](docs/maintenance.md)
- [Compiler workspace](compiler/README.md)

The active compiler workspace is under `development/compiler/`. The previous compiler is preserved
by commit `f6c08da3` and Git history. Do not use it as a behavioral oracle or implementation
reference.

## v0.14.0 Rewrite Record

- [Rewrite milestone](milestones/v0.14.0.md)
- [Implementation qualification](milestones/v0.14.0-qualification.md)
- [Final design review](reviews/v0.14.0-final-design.md)
- [Release preparation](milestones/v0.14.0-release-preparation.md)
- [Publication and audit](releases/v0.14.0.md)

## Specification Workflow

When a public rule is incomplete, implementation pauses. The ambiguity is reduced to a minimal
program, alternatives are compared, and the user selects the language behavior. The adopted rule
is written in the owning specification chapter before implementation or conformance tests proceed.

The specification is the sole source for:

- lexical and syntactic grammar
- name resolution and visibility
- type identity, inference, conversion, and dispatch
- ownership, borrowing, provenance, regions, and destruction
- evaluation order, failure, allocation, and cleanup
- target ABI and executable behavior
- command-line and editor contracts
- public standard-library APIs

## Repository Layout

```text
development/
├── AGENTS.md
├── README.md
├── TODO.md
├── compiler/          # specification-first compiler workspace
├── docs/              # active compiler architecture and maintenance policy
├── milestones/        # active and historical milestone records
├── packaging/         # release identity, deterministic assembly, and artifact qualification
├── reviews/           # cross-cutting design criteria, findings, and remediation evidence
├── releases/          # immutable published qualification records
└── std/               # standard-library contracts and implementation sources
```

Rust, Cargo, and Node.js are development requirements for the workspace and release qualification.
