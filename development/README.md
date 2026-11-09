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
   links the non-normative pre-S24 prototype snapshot. It accepts implicit moves that the current
   mandatory postfix transfer contract rejects; it is not a current-language subset or compatibility mode.
   [Modules](spec/language/modules.md), [type behavior](spec/language/type-behavior.md) and
   [interfaces](spec/language/interfaces.md) define adopted source roles and operation binding;
   their [migration review](history/reviews/v2.0.0-module-and-operation-contracts.md) records the
   reading exercise and remaining design gates without claiming compiler support.
   [Generic/constant contracts](spec/language/generics-and-constants.md) and
   [opaque results](spec/language/opaque-results.md) select their source/staging boundary;
   their [review](history/reviews/v2.0.0-generic-and-constant-contracts.md) distinguishes that
   design from missing evaluator, solver and native evidence.
   [Outcomes](spec/language/outcomes.md) and their
   [access review](history/reviews/v2.0.0-outcome-access-contracts.md) distinguish payload values
   from scoped projections and explicit failure decisions, without claiming compiler admission.
   [Async phases](spec/language/asynchronous-computations.md) and
   [inferred effects](spec/language/execution-effects.md) own completion/cancellation and
   construction/drive/cleanup distinctions; their
   [review](history/reviews/v2.0.0-async-and-effect-contracts.md) records design corrections and
   pending runtime/proof gates.
   [Enum schemas](spec/language/structs-and-enums.md#enum-schemas-and-construction) and
   [patterns](spec/language/patterns.md) own branch access, coverage, tuple paths and cleanup;
   their [review](history/reviews/v2.0.0-enum-and-pattern-contracts.md) separates those rules
   from missing compiler/evaluator/layout evidence.
   [Tuples and fixed arrays](spec/language/tuples-and-arrays.md) own structural construction,
   repetition, component selection and partial initialization; their
   [review](history/reviews/v2.0.0-fixed-aggregate-contracts.md) records empty-array Copy and
   dynamic owning-element proof obligations separately from general sequence protocols.
   [Variadic inputs](spec/language/variadic-inputs.md) own ordinary/keyed argument sequences,
   built-in expansion, capture and component cleanup; their
   [review](history/reviews/v2.0.0-variadic-input-contracts.md) separates those source rules from
   missing compiler/evaluator/target qualification.
   [Synchronous traversal](spec/language/iteration.md) and its
   [library protocols](spec/library/iteration.md) own for/current-place and user expansion;
   their [review](history/reviews/v2.0.0-iteration-contracts.md) records sequential versus
   accumulated access, public schema conditions and unimplemented qualification obligations.
   [Explicit transfer](spec/language/value-access.md#explicit-transfer-sources) and its
   [migration review](history/reviews/v2.0.0-explicit-transfer-contracts.md) close source retirement
   across those subjects, while compiler migration remains pending.
2. **Current compiler contracts:** the [architecture overview](architecture/overview.md),
   [private-unit handoff](architecture/pipeline/value-access-declarations.md), and the relevant
   [crate README](compiler/README.md#crate-documentation).
3. **Implemented pre-S24 prototype scope and verification:** the [captured code instances](history/reviews/v2.0.0-captured-code-instances.md),
   [owned-capture producers](history/reviews/v2.0.0-owned-capture-producers.md),
   [type foundation and interrupted results](history/reviews/v2.0.0-type-foundation-and-interruption.md),
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

Resolve an incomplete public rule before implementing the affected behavior. Reduce the ambiguity
to a minimal program and compare alternatives against the documented goals. The current
[development agent instructions](AGENTS.md) own delegated decision scope and approval boundaries.
Write the adopted rule in its owning specification chapter before implementation or conformance
admission. Independent work can continue while a required decision remains pending.

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

The [repository instructions](../AGENTS.md) and [development instructions](AGENTS.md) own review,
defect handling, authority replacement and approval policy. Exact compiler verification commands
live in the [compiler workspace guide](compiler/README.md), evidence selection in the
[verification guide](verification/README.md), and documentation build details in the
[site generator guide](site/README.md).

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
