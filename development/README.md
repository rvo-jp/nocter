# Contributor Documentation

Nocter development is contract-first. Published language, platform and tooling behavior belongs in
[`spec/`](../spec/README.md); adopted unreleased contracts belong in the
[development specification](spec/README.md). Each version has one owning rule, and an unclosed
v2.0.0 subject cannot inherit released behavior through a fallback. Standard-library behavior belongs with its compiler-checked public
surface under [`std`](../std/README.md). Implementation milestones and reviews record how
the compiler reached those contracts without becoming another authority.

## Current Work

- [Development specification](spec/README.md) — adopted unreleased contracts, excluded from the user documentation site
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

## Value Binding Reading Order

These entry points cover the v2.0.0 redesign. Read them in order; each owns a different kind
of information:

1. **Source semantics:** the future [Value Access Model](spec/language/value-access.md) and its
   linked owning chapters. The [bounded value-binding grammar](spec/language/syntactic-grammar.md#bounded-value-binding-fragment)
   defines the private-unit source boundary; the broader future grammar is a separate scope.
2. **Current compiler contracts:** the [architecture overview](architecture/overview.md),
   [private-unit handoff](architecture/pipeline/value-access-declarations.md), and the relevant
   [crate README](compiler/README.md#crate-documentation).
3. **Implemented scope and verification:** the [type foundation and interrupted results](history/reviews/v2.0.0-type-foundation-and-interruption.md),
   [input-dependent callable results](history/reviews/v2.0.0-dependent-callable-results.md),
   [deferred value supply](history/reviews/v2.0.0-deferred-value-supply.md),
   [callable result witnesses](history/reviews/v2.0.0-callable-result-witnesses.md),
   [private instance closure](history/reviews/v2.0.0-private-instance-closure.md),
   [callable input witnesses](history/reviews/v2.0.0-callable-input-witnesses.md),
   [named function witnesses](history/reviews/v2.0.0-function-witnesses.md),
   [record source integration](history/reviews/v2.0.0-record-source-integration.md),
   earlier [bounded front-end record](history/reviews/v2.0.0-value-binding-front-end.md), and
   [production source corpus](compiler/tests/fixtures/value-bindings/README.md).
4. **Next work:** the [handoff](TODO.md). The [milestone](history/milestones/v2.0.0.md) records
   broader work order; the [implementation design](history/reviews/v2.0.0-value-binding-implementation.md)
   retains obligations beyond the implemented fragment.
5. **Decision rationale and research:** the [classified review catalog](history/reviews/README.md#value-binding-reviews).
   Earlier source proposals and finite model explorations are labeled separately there.

A selected source design, a grammar draft, compiler acceptance, native execution, and a performance
measurement are different evidence. Read the owning record for the specific claim. Historical
examples do not extend the implemented source boundary or introduce an alternate spelling.

## Specification Workflow

When a public rule is incomplete, implementation pauses. The ambiguity is reduced to a minimal
program and alternatives are compared. Under an explicitly delegated design scope, the agent
resolves the decision and records its evidence; otherwise the user selects the language behavior.
The [input-default and delegation record](history/reviews/v2.0.0-parameter-defaults.md#decision-and-scope)
records the v2.0.0 model/source-design and normal work-sequencing delegation. The adopted rule is
written in the owning version's specification chapter before implementation or conformance tests proceed.

The published and development specifications own the following subjects within their respective
version scopes:

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
the selected behavior in its owning version's specification, and derive conformance cases before
implementation continues.
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
