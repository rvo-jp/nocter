# Contributor Documentation

Nocter is rebuilding its compiler from the language specification. The published v0.13.0 release
remains available to users, but its compiler implementation is not an input to the rewrite.

The active work first closes every normative gap in [`spec/`](../spec/README.md). Compiler code is
added only after the relevant syntax, static semantics, dynamic semantics, target behavior, and
tooling contract can be implemented without guessing.

## Current Work

- [Current handoff](TODO.md)
- [v0.14.0 rewrite milestone](milestones/v0.14.0.md)
- [New compiler architecture](docs/architecture.md)
- [Checked program design](docs/checked-program-design.md)
- [Target and executable program design](docs/target-program-design.md)
- [Declaration diagnostic boundary](docs/declaration-diagnostic-boundary.md)
- [Semantic presentation design](docs/semantic-presentation-design.md)
- [Grammar conformance plan](docs/grammar-conformance.md)
- [Rewrite maintenance policy](docs/maintenance.md)
- [New compiler boundary](compiler/README.md)

The new source/syntax workspace is under `development/compiler/`. The previous compiler is
preserved by commit `f6c08da3` and Git history. Do not use it as a behavioral oracle or
implementation reference.

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
├── docs/              # rewrite architecture and maintenance policy
├── milestones/        # active and historical milestone records
├── packaging/         # published-package inputs, unchanged during specification closure
├── releases/          # immutable published qualification records
└── std/               # existing source, not a bootstrap oracle for the new compiler
```

Rust and Cargo are development requirements for the new workspace. Users of the published
v0.13.0 release remain unaffected by the rewrite.
