# Public Provenance Contracts and Generic Interface Bounds

This document owns the implementation design for v0.3.0 Phase 4. Public language semantics belong
in the specification. The completion gate belongs to the
[v0.3.0 Release Record](../releases/v0.3.0.md).

## Design Boundary

Phase 4 exposes only result-storage relationships that cannot safely be guessed at an abstraction
boundary. It does not expose lexical lifetime arithmetic. Concrete bodies retain inference, and
allocation effects remain compiler-owned inferred facts.

The dependency flow is:

```text
source `from` clause
  -> resolver input identities
  -> callable provenance contract
  -> inferred-summary validation or bodyless-summary seed
  -> call-site origin substitution
  -> ownership and region escape
  -> analysis and LSP
```

Generic interface dispatch is a separate layer over that summary:

```text
generic bound
  -> canonical interface identity and type arguments
  -> bound method declaration
  -> concrete explicit conformance at specialization
  -> public inherent implementation
  -> ordinary static call lowering
```

Neither layer searches for standard-library names.

## Contract Representation

Public provenance and generic-bound rules are defined by [Memory, Regions, and
Allocators](../../spec/06-memory-region-allocator.md) and [Generics, Interfaces, Embedding, and
Methods](../../spec/08-generics-interfaces-embedding-methods.md). Internally, source order is
retained for formatting and diagnostics while contract comparison uses resolved declaration
identities. Tracked storage includes both source borrows and pointer-backed owned aggregates because
owned buffers must retain their allocation-context origin even though raw pointers do not
participate in loan checking.

## Internal Ownership

- `ast/provenance` owns source clauses and source spans.
- `resolve/body` binds origin spans to receiver and parameter symbols for navigation.
- `typecheck/provenance/contracts` converts eligible declarations into semantic storage origins.
- `resolve/signatures` and `resolve/imports/qualification` preserve bound and conformance identities
  across module boundaries.
- `typecheck/interface_bounds` owns generic-receiver lookup and conformance substitution.
- `typecheck/facts` records bound-call targets and specializations; `analysis/call_specializations`
  redirects reachable concrete calls to conformance implementation members.
- `analysis` presents compiler facts; protocol code only converts source ranges and labels.

Contract validation is covariant in safety: an implementation may return storage that outlives the
declared source, but never storage that may die earlier or comes from an undeclared peer source.
Unknown provenance never satisfies an explicit contract.

## Static Dispatch

Generic source is checked against the interface method signature. Each reachable concrete
specialization must have explicit conformance to the canonical interface instantiation. The
selected conformance member is lowered through the ordinary static method call path. No vtable,
witness table, runtime type identity, or name-based backend lookup is introduced.

## Editor Contract

Hover and signature help show normalized types and the resolved provenance relation. Completion on
a generic receiver lists only accessible methods from its declared bound. Definition from a generic
call targets the interface declaration; concrete-specialization facts may additionally identify the
implementation internally. Recovery may insert delimiters or placeholder identifiers, but it may
not invent bound or origin identities.

## Allocation Effects

There is no Phase 4 allocation-effect annotation. Source bodies infer their effect to a fixed point;
trusted bodyless declarations retain compiler metadata. `from current` necessarily seeds the
current-context effect. Result-independent temporary allocation remains inferred and visible in
hover without becoming source syntax.

## Completion Status

Phase 4 completed on 2026-08-03. The parser, formatter, AST JSON, resolver, type checker, ownership
checker, allocation propagation, buildability validation, IR, analysis, and LSP consume the shared
models described here. The distributed `std/sequence.Sequence<T>` interface and generic `first`
helper verify exact receiver-to-result provenance through `Vec<T>` across source modules. Import
qualification retains both generic bounds and explicit conformance contracts, and IR selects a
concrete method target only when the reachable callable index proves it unique.

Repository-home and packaged-home checks retain the source loan, packaged native execution observes
the specialized element result, and protocol tests verify bound hover/completion plus definition
ranges. No Phase 4 work remains active.
