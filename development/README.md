# Contributor Documentation

Nocter has published v0.21.0 and started the v0.22.0 practical JSON milestone with a compiler built
from the language specification. The v0.13.0 compiler implementation was not an input to the
rewrite.

The rewrite closed each normative gap in [`spec/`](../spec/README.md) before implementing the
corresponding syntax, static semantics, dynamic semantics, target behavior, or tooling contract.

## Current Work

- [Current handoff](TODO.md)
- [Active v0.22.0 milestone](milestones/v0.22.0.md)
- [v0.22.0 Phase 0 JSON design review](reviews/v0.22.0-phase-0.md)
- [v0.22.0 Phase 1 lexical and Unicode review](reviews/v0.22.0-phase-1.md)
- [v0.22.0 Phase 2 owning DOM parser review](reviews/v0.22.0-phase-2.md)
- [v0.22.0 Phase 3 shared generator review](reviews/v0.22.0-phase-3.md)
- [v0.22.0 Phase 4 practical integration review](reviews/v0.22.0-phase-4.md)
- [v0.22.0 Phase 5 standard-library stabilization review](reviews/v0.22.0-phase-5.md)
- [JSON implementation boundary](docs/json-implementation.md)
- [Completed v0.21.0 milestone](milestones/v0.21.0.md)
- [v0.21.0 release preparation](milestones/v0.21.0-release-preparation.md)
- [v0.21.0 Phase 5 practical qualification review](reviews/v0.21.0-phase-5.md)
- [v0.21.0 Phase 4 iteration and Set review](reviews/v0.21.0-phase-4.md)
- [v0.21.0 Phase 0 associative-collection design review](reviews/v0.21.0-phase-0.md)
- [Associative collection implementation boundary](docs/associative-collection-implementation.md)
- [Completed v0.20.0 compiler-foundation milestone](milestones/v0.20.0.md)
- [v0.20.0 Phase 3 dependency-local exact-selection review](reviews/v0.20.0-phase-3.md)
- [v0.20.0 Phase 2 unified query-entry review](reviews/v0.20.0-phase-2.md)
- [v0.20.0 Phase 1 incremental semantic-computation review](reviews/v0.20.0-phase-1.md)
- [v0.20.0 Phase 0 interface-prerequisite review](reviews/v0.20.0-phase-0.md)
- [Completed v0.19.0 milestone](milestones/v0.19.0.md)
- [v0.19.0 release preparation](milestones/v0.19.0-release-preparation.md)
- [Completed v0.18.0 milestone](milestones/v0.18.0.md)
- [v0.18.0 release preparation](milestones/v0.18.0-release-preparation.md)
- [Design reviews](reviews/README.md)
- [Latest published release qualification](releases/v0.21.0.md)
- [Compiler architecture](docs/architecture.md)
- [Architecture and maintenance documents](docs/README.md)
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
