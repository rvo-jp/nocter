# Contributor Documentation

Nocter development is specification-first. Public behavior belongs in [`spec/`](../spec/README.md);
implementation milestones and reviews record how the compiler reached that behavior without
becoming a second language authority. The compiler implementation removed before v0.14.0 is
available through Git history only and is not a design input.

## Current Work

- [Current handoff](TODO.md) — next concrete work and blockers only
- [Compiler architecture](design/architecture.md) — pipeline and cross-crate authority boundaries
- [Architecture and maintenance documents](design/README.md)
- [Compiler workspace](compiler/README.md)
- [Development verification](verification/README.md) — disposable complete compiler gates
- [Documentation site generator](site/README.md) — authored website inputs and output boundary
- [Development history](history/README.md) — milestones, reviews, legacy design, and publication audits

The active compiler workspace is under `development/compiler/`. The previous compiler is preserved
by commit `f6c08da3` and Git history. Do not use it as a behavioral oracle or implementation
reference.

## v0.14.0 Rewrite Record

- [Rewrite milestone](history/milestones/v0.14.0.md)
- [Implementation qualification](history/milestones/v0.14.0-qualification.md)
- [Final design review](history/reviews/v0.14.0-final-design.md)
- [Release preparation](history/milestones/v0.14.0-release-preparation.md)
- [Publication and audit](history/release-audits/v0.14.0.md)

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
├── design/            # active cross-crate architecture and maintenance policy
├── history/           # non-normative engineering records, excluded from the website
├── packaging/         # release identity, deterministic assembly, and artifact qualification
├── site/              # documentation generator and static website inputs
├── std/               # standard-library contracts and implementation sources
├── unicode/           # generated Unicode source data and its generator
└── verification/      # disposable development verification entry points
```

Rust, Cargo, and Node.js are development requirements for the workspace and release qualification.
