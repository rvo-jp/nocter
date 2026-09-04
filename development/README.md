# Contributor Documentation

Nocter development is specification-first. Public behavior belongs in [`spec/`](../spec/README.md);
implementation milestones and reviews record how the compiler reached that behavior without
becoming a second language authority. The compiler implementation removed before v0.14.0 is
available through Git history only and is not a design input.

## Current Work

- [Current handoff](TODO.md) — next concrete work and blockers only
- [Milestone catalog](milestones/README.md) — scope and status owned by each milestone record
- [Design review catalog](reviews/README.md) — findings and remediation evidence
- [Publication catalog](releases/README.md) — immutable released-candidate evidence
- [Compiler architecture](docs/architecture.md) — pipeline and cross-crate authority boundaries
- [Architecture and maintenance documents](docs/README.md)
- [Compiler workspace](compiler/README.md)
- [Development verification](verification/README.md) — disposable complete compiler gates

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
├── std/               # standard-library contracts and implementation sources
└── verification/      # disposable development verification entry points
```

Rust, Cargo, and Node.js are development requirements for the workspace and release qualification.
