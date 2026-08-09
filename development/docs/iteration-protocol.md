# Protocol-Driven Collection Iteration

This document owns the compiler design for v0.3.0 Phase 7 explicit readonly and consuming
collection iteration. Public control-flow semantics belong to the specification. The completed
gate belongs to the [v0.3.0 Release Record](../releases/v0.3.0.md).

Phase 7 completed on 2026-08-03. The compiler, standard library, native backend, and
LSP implement the boundaries and acceptance observations recorded below.

## Boundary

Phase 2 supplied concrete readonly and consuming iterator values. Phase 4 supplied explicit generic
interface conformance and static bound dispatch. Phase 6 made optional step results ordinary stored
values. Phase 7 composes those foundations without introducing method-name lowering.

The adopted forms are:

```nct
for item in &values { ... }
for item in move values { ... }
for item in iterator { ... }
```

The first form borrows a collection, the second transfers it, and the third accepts only a value
that already satisfies the iterator role. `for item in values` does not guess whether a collection
should be borrowed or moved.

## Trusted Protocol Roles

The trusted Nocter home supplies ordinary generic interfaces for iterator step, readonly
conversion, owned conversion, and exact remaining length. Frontend validation checks each complete
interface shape and records the interface and required method declaration identities in
`TrustedDeclarationFacts`.

Later phases consume a role enum and declaration spans. They do not search source names or module
paths. A missing or malformed trusted bundle produces a source-backed availability diagnostic
before lowering. Explicit user conformance remains the only way a nominal type participates.

The generic interface parameters carry the yielded item and concrete iterator types. Phase 7 does
not require associated types or broaden the one-bound generic model. A conversion is usable only
when its concrete iterator type also has exactly one matching iterator-role conformance.

Phase 8 validates `ExactSizeIterator<T>` beside `Iterator<T>` for sequence spread. Its readonly
`remaining_len(): usize` method is an ordinary statically specialized call. The compiler records
the method identity in the same iteration runtime plan; it does not recognize the spelling or a
standard-library nominal type.

## Semantic Plan

Typecheck records one immutable plan per collection-for statement:

- source mode: direct iterator, readonly conversion, or owned conversion
- source and iterator concrete types
- conversion interface, conformance, method declaration, and concrete call target when applicable
- iterator interface, conformance, step declaration, and concrete call target
- yielded item type and optional outcome shape
- binding and source spans used by editor queries
- whether source evaluation transfers ownership

Resolver and typecheck produce the plan. Ownership, regions, buildability, IR, analysis, and LSP
consume it without repeating protocol lookup. The selected declaration identities connect implicit
conversion and step calls to the ordinary result-provenance and allocation-effect summaries; those
summaries are not duplicated inside the plan.

Collection loops use conversion and step fields from this plan. Sequence spread additionally uses
its exact-count target and an explicit copy, readonly-reference, or move projection. Hidden literal
entry calls every count target once, checks and caches the total pack length, then reuses the same
optional-step lowering as collection loops to stream items.

## Ownership and Cleanup

The source expression evaluates once into a compiler-owned iterator local. Each step consumes one
optional layer. Success initializes the loop binding; absence exits without touching item storage.

An owned item has a per-iteration obligation. Normal body completion and `continue` drop it unless
the body moved it. `break`, `return`, propagation, and other exiting edges clean the item before the
iterator. Iterator drop then destroys only its still-initialized state. Readonly item provenance
retains the original collection loan through its last use.

The loop does not maintain a parallel ownership state in IR. Hidden locals use the same move,
drop-kind, partial-initialization, scope-mark, and region cleanup machinery as source bindings.

## Compiler Ownership

| Responsibility | Owner |
|---|---|
| trusted interface-shape validation and role identities | `target/trusted_iteration` and `semantics` |
| collection-for AST and recovery | `parser/collection_for` and `analysis/collection_for_recovery` |
| conformance resolution and semantic plan | `typecheck/iteration` and typecheck facts |
| source loan, item move, and loop state | `typecheck/ownership` consuming the plan |
| provenance and region constraints | existing provenance/region analyses consuming the plan |
| iterator, step, branch, and cleanup lowering | `ir/lower/collection_for` |
| standard interfaces and concrete conformances | `std/iter` and `std/vec`, with focused implementation sources |
| hover, completion, and semantic presentation | `analysis/iteration`; protocol conversion in `driver/lsp` |

New responsibilities use focused modules. Existing exhaustive AST visitors gain collection-for
edges but do not acquire protocol lookup.

## Verification

Tests observe item order, source moves and loans, active item drop, remaining suffix drop, storage
release, nested cleanup, region escape, generic specialization, diagnostics, incomplete editor
input, and packaged execution. Instruction snapshots alone do not satisfy the runtime gate.

The completed Phase 7 gate covers empty, readonly, consuming, direct, nested, user-conformance,
`continue`, `break`, `return`, and propagation paths. LSP protocol tests cover all three source
modes, exact element completion, semantic-token range remapping, parser diagnostics, and implicit
allocation-effect presentation.

Phase 8 tests add exact-size role validation, unknown-size rejection, repeated and direct-iterator
spread, cached pack length, copy constraints, readonly provenance, owned suffix cleanup, implicit
effect presentation, and incomplete spread recovery against the packaged standard library.
