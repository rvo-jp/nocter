# Public Provenance Contracts and Generic Interface Bounds

This document owns the compiler implementation boundary for source-visible result allocation and
external provenance contracts. Public language semantics belong in the specification. The
implementation completion evidence belongs to the
[v0.6.0 Release Qualification Record](../releases/v0.6.0.md).

## Design Boundary

Source contracts expose only result-storage relationships that cannot safely be guessed at an
abstraction boundary. They do not expose lexical lifetime arithmetic or the compiler's hidden
execution allocation requirement. Concrete bodies retain lossless inference.

The dependency flow is:

```text
source `alloc` and `from` contracts
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
- `typecheck/provenance/result_allocation` owns newly allocated result projections independently
  from external origins and the execution allocation requirement.
- `typecheck/provenance/storage_capability` treats an unresolved type parameter as a storage
  capability boundary; only a concrete scalar substitution proves storage independence.
- `typecheck/provenance/container_transfer` removes the lexical scope of a transferred container
  without removing region or scope origins carried by the transferred element.
- `typecheck/provenance/storage_projection` projects lossless aggregate dataflow onto
  storage-bearing fields and outcome branches when validating public `from` and `alloc` contracts.
- `typecheck/returns/borrow_returns/mutation_effects` preserves allocation retained through
  readwrite inputs, including allocator-origin inheritance and neutral-storage fallback.
- return checking and summary inference consume the same retained-input mutation effects, so a
  mutation performed before `return` cannot disappear from region escape validation.
- `typecheck/returns/borrow_returns/collection_iteration_provenance` instantiates the resolved
  conversion and step summaries for protocol `for`; loop bindings are not synthetic independent
  locals.
- `resolve/signatures` and `resolve/imports/qualification` preserve bound and conformance identities
  across module boundaries.
- `typecheck/interface_bounds` owns generic-receiver lookup and conformance substitution.
- `typecheck/facts` records bound-call targets and specializations; `analysis/call_specializations`
  redirects reachable concrete calls to conformance implementation members.
- `analysis/presentation` renders only declared source contracts; protocol code converts source
  ranges, Markdown, and edits without reconstructing semantic prose.
- `target/trusted_iteration` validates the complete iterator method result contract, and
  `target/trusted_pointer` attaches ownership-transfer behavior only to an exact compiler-owned
  primitive shape.

Contract validation is covariant in safety: an implementation may return storage that outlives the
declared source, but never storage that may die earlier or comes from an undeclared peer source.
Unknown provenance never satisfies an explicit contract.

## Static Dispatch

Generic source is checked against the interface method signature. Each reachable concrete
specialization must have explicit conformance to the canonical interface instantiation. The
selected conformance member is lowered through the ordinary static method call path. No vtable,
witness table, runtime type identity, or name-based backend lookup is introduced.

## Editor Contract

Hover, completion, and signature help show the same normalized declaration, including written
`alloc` and `from` contracts. They do not append inferred phrases such as `allocates`, `from
inferred storage`, or a private aggregate provenance dump. Completion on
a generic receiver lists only accessible methods from its declared bound. Definition from a generic
call targets the interface declaration; concrete-specialization facts may additionally identify the
implementation internally. Recovery may insert delimiters or placeholder identifiers, but it may
not invent bound or origin identities.

## Result Allocation and Execution Requirements

`alloc` means that newly allocated storage may survive in a returned storage-bearing projection.
It does not mean that evaluation merely performs allocation. Body-backed declarations are checked
exactly after result summaries converge; trusted bodyless allocation operations must agree with
their compiler metadata. Interface declarations and structural callable types use `alloc` as an
upper bound.

Generic result types remain storage-capable until specialization proves otherwise. This prevents
`T = String`, `T = &U`, optional results, aggregate fields containing `T`, and protocol-yielded
values from losing allocation or external provenance. Iterator construction is allocation-free,
but `Iterator<T>.next` is an `alloc ... from self` upper bound because a source or stored callback
may produce allocated storage. Scalar terminal results discard that possibility through the
semantic return type.

`from X` remains a separate external-origin contract. A named allocator is eligible because its
capability carries storage provenance. The ambient allocation context remains an internal origin;
source `from current` is rejected. Result-independent temporary allocation and the need to pass a
hidden current context remain inferred implementation facts and are not presented as source text.

## Historical Foundation

v0.3.0 Phase 4 established identity-resolved `from` contracts and generic-interface substitution.
v0.6.0 Phase 1 extends that foundation with result allocation, type-directed public-contract
projection, allocator-backed mutation provenance, canonical editor presentation, and shared source
edits. v0.6.0 Phase 2 adds recursive bottom-seeded summaries, returned-expression evidence,
closure-bound variance, conservative generic storage capabilities, protocol-loop provenance, and
identity-validated container ownership transfer. Historical release qualification remains in the
versioned release records.
