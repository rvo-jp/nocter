# Public Provenance Contracts and Generic Interface Bounds

This document owns the implementation design for v0.3.0 Phase 4. Public language semantics belong
in the specification. The completion gate belongs to the
[v0.3.0 Release Record](v0.3.0.md).

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

## Surface Grammar

A callable, including a typed literal definition, may append one result provenance clause after
its return type:

```nct
from self
from value
from left | right
from static
from current
```

`self` is eligible only for methods. Other identifiers must name borrow-like input parameters.
`static` means program-lifetime storage. `current` means the caller's current allocation context and
implies the existing allocation effect. Duplicate origins are invalid. Source order is retained for
formatting and diagnostics; semantic comparison uses declaration identities.

A Phase 1 sequence literal pack is a set of owned element values rather than one borrow-like input
identity, so its capture name is not an eligible origin. Literal definitions that allocate their
result use `from current`; a string literal definition may use its `&str` parameter as an origin.

The clause describes an upper bound on possible result storage. A concrete result may be
storage-independent or static when the contract permits a shorter input origin. It may not contain
an input, region, scope, or current-context origin absent from the contract.

Tracked result storage includes source-level borrows and pointer-backed owned aggregates. The
distinction is intentional: raw pointers do not participate in borrow checking, but an owning value
such as `String` or `Vec<T>` must still retain the allocation context that owns its buffer.

Generic parameters accept one interface bound:

```nct
func inspect<T: Readable<i32>>(value: &T): i32
```

Phase 4 deliberately excludes `where` clauses and multiple bounds. Bound lookup never falls back
to methods from an unconstrained receiver. Phase 10 default bodies remain attached to their exact
interface declaration identity.

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
