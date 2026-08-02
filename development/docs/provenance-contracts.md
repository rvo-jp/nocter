# Public Provenance Contracts and Generic Interface Bounds

This document owns the implementation design for v0.3.0 Phase 4. Public language semantics belong
in the specification. The completion gate belongs to the
[v0.3.0 Development Contract](v0.3.0.md).

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

A callable may append one result provenance clause after its return type:

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

The clause describes an upper bound on possible result storage. A concrete result may be
storage-independent or static when the contract permits a shorter input origin. It may not contain
an input, region, scope, or current-context origin absent from the contract.

Generic parameters accept one interface bound:

```nct
func inspect<T: Readable<i32>>(value: &T): i32
```

Phase 4 deliberately excludes `where` clauses and multiple bounds. Bound lookup never imports
extension methods and never falls back to inherent methods for an unconstrained parameter.

## Internal Ownership

- `ast/provenance` owns source clauses and source spans.
- `resolve/provenance` binds origin names to `InputId` values.
- `typecheck/provenance/contracts` builds and validates semantic provenance values.
- `resolve/bounds` owns canonical interface-bound identities.
- `typecheck/bound_calls` owns generic-receiver lookup and specialization facts.
- `analysis` presents compiler facts; protocol code only converts source ranges and labels.

Contract validation is covariant in safety: an implementation may return storage that outlives the
declared source, but never storage that may die earlier or comes from an undeclared peer source.
Unknown provenance never satisfies an explicit contract.

## Static Dispatch

Generic source is checked against the interface method signature. Each reachable concrete
specialization must have explicit conformance to the canonical interface instantiation. The
conformance target's public inherent method is then lowered through the ordinary method call path.
No vtable, witness table, runtime type identity, or name-based backend lookup is introduced.

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
