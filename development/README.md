# Contributor Documentation

Nocter development is contract-first. Language, platform, and tooling behavior belongs in
[`spec/`](../spec/README.md). Standard-library behavior belongs with its compiler-checked public
surface under [`std`](../std/README.md). Implementation milestones and reviews record how
the compiler reached those contracts without becoming another authority.

## Current Work

- [Current handoff](TODO.md) — next concrete work and blockers only
- [Compiler architecture](architecture/overview.md) — pipeline and cross-responsibility authority boundaries
- [Architecture contracts](architecture/README.md)
- [Compiler workspace](compiler/README.md)
- [Performance measurement](benchmarks/README.md) — external latency evidence and query-count boundary
- [Development verification](verification/README.md) — disposable complete compiler gates
- [Documentation site generator](site/README.md) — authored website inputs and output boundary
- [Development history](history/README.md) — milestones, reviews, legacy design, and publication audits

The active compiler workspace is under `development/compiler/`. Superseded implementations and
their rewrite records remain available through the [development history](history/README.md), but
are not behavioral or architectural inputs.

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

Compiler-checked standard-library declarations and declaration documentation belong to module
`index.nct` files. The [standard-library catalog](../std/README.md#behavior-guide-authority) assigns
each longer observable subject to one behavior guide without repeating signatures.

## Maintenance Workflow

Escalate a language decision only when at least two materially different observable behaviors
remain consistent after the owning specification chapter and its cross-references have been
audited. Reduce the ambiguity to one minimal source example, compare concrete consequences, record
the selected behavior in `spec/`, and derive conformance cases before implementation continues.
Internal representation, implementation order, and diagnostic wording not fixed by the
specification do not require a language decision.

Keep one authority replacement coherent: do not introduce a new producer while retaining an
undocumented compatibility path. Passing tests are necessary but do not replace an audit for
duplicate producers, reverse lookup, order dependence, or hidden fallback behavior. Exact compiler
verification commands live in the [compiler workspace guide](compiler/README.md); documentation
changes also run the [site generator](site/README.md) into an explicit directory outside the
repository.

## Repository Layout

```text
development/
├── AGENTS.md
├── README.md
├── TODO.md
├── benchmarks/        # repeatable external compiler and editor measurements
├── compiler/          # specification-first compiler workspace
├── architecture/      # active cross-responsibility compiler and runtime contracts
├── history/           # non-normative engineering records, excluded from the website
├── packaging/         # release identity, deterministic assembly, and artifact qualification
├── site/              # documentation generator and static website inputs
├── unicode/           # generated Unicode source data and its generator
└── verification/      # disposable development verification entry points
```

Rust, Cargo, and Node.js are development requirements for the workspace and release qualification.
