# Composable Iterators and Collection Builders

This document owns the compiler and standard-library implementation design for v0.3.0 Phase 9.
Public generic and iterator semantics belong to the specification. The active completion gate
belongs to the [v0.3.0 Release Record](v0.3.0.md).

## Capability Sets

A generic parameter may require more than one interface:

```nct
func reserve_exact<T, I: Iterator<T> + ExactSizeIterator<T>>(source: &I): usize {
    return source.remaining_len()
}
```

The source order is retained for formatting and diagnostics, but the semantic value is a set of
specialized interface declaration identities. Duplicate identities are rejected. Generic method
lookup searches the complete set and rejects a name supplied by more than one distinct interface;
it never selects by import order or by comparing only method text.

`where` clauses, interface inheritance, negative bounds, runtime interface objects, and overlapping
specialization remain outside Phase 9.

## Conditional Conformance

A conformance declaration may constrain its own generic parameters:

```nct
impl<T, I: Iterator<T>> Iterator<T> for TakeIter<T, I> {
    method &+self.next(): T? {
        // advance the bounded source
    }
}

impl<T, I: ExactSizeIterator<T>> ExactSizeIterator<T> for TakeIter<T, I> {
    method &self.remaining_len(): usize {
        // return the exact bounded remainder
    }
}
```

Resolver output retains the declaration span, target pattern, interface pattern, generic
parameters, and every required bound as one immutable conformance signature. For a concrete target,
typecheck first infers the conformance substitutions from the target pattern, specializes the
interface pattern, and then proves every required bound. A failed condition means that the
conformance is absent, not that a partially valid candidate exists.

Conformance discovery, generic call checking, iteration planning, buildability specialization, IR,
analysis, and LSP consume the same query. The compiler does not recognize adapter names. Two
declarations with the same normalized target/interface pattern are rejected even if their
conditions differ; Phase 9 does not attempt overlap reasoning or specialization.

## Standard Adapter Boundary

`std/iter` retains protocol interfaces and the contiguous readonly cursor. `std/iter/sources`,
`std/iter/range`, `std/iter/chain`, `std/iter/enumerate`, and `std/iter/ops` own their focused state
and algorithms:

- `empty` and `once`
- `take` and `skip`
- `chain`
- `enumerate`, yielding public `Indexed<T> { index, item }` values
- consuming terminal operations `count` and `last`

Every adapter owns its input iterator. `next()` transfers at most one item per successful step.
Dropping an adapter drops its current owned state and unconsumed input exactly once through ordinary
aggregate cleanup. No adapter creates a hidden `Vec`, allocator context, runtime interface object,
or callback representation.

An adapter implements `ExactSizeIterator<T>` only when its remaining count is mathematically exact
and its input conditions prove the required exact-size capabilities. `empty` and `once` are always
exact. `take`, `skip`, and `enumerate` preserve exactness from one input; `chain` requires it from
both inputs. Count arithmetic is checked before publication and never grants unchecked element
access.

Phase 9 intentionally omits `map`, `filter`, callback-driven `fold`, and comparator-driven sorting.
Those APIs require one first-class callable contract covering capture ownership, indirect-call
allocation effects, result provenance, and cleanup.

## Collection Builders

`Vec.from_iter` consumes any `Iterator<T>` and grows the vector through the existing aborting
`push` path. Unknown size is explicit in the API and does not cause a hidden intermediate
collection.

`Vec.from_exact_iter` additionally requires `ExactSizeIterator<T>`, reads the initial exact count
once, reserves that capacity, then still terminates through `next()`. A dishonest exact-size
implementation cannot cause unchecked writes: excess items use ordinary checked growth, and early
exhaustion leaves a shorter valid vector.

Both builders evaluate the iterator once, transfer each yielded owner once, allocate in the current
context, retain region provenance, and clean the partial vector plus iterator suffix on every
supported exit. Recoverable `try_from_iter` is deferred until its failure contract states whether a
partially consumed input is returned to the caller.

## Compiler Ownership

| Responsibility | Owner |
|---|---|
| bound-list AST, parser, JSON, and formatter | `ast`, `parser/types`, `format`, and `ast/json` |
| conformance signatures and qualification | `resolve/conformance` and import qualification |
| capability-set validation and bound method lookup | `typecheck/interface_bounds` |
| conditional conformance matching | `typecheck/conformance` |
| static target selection | existing call, iteration, buildability, and IR consumers |
| adapter values and collection algorithms | focused modules under `std/` |
| hover, completion, definition, and recovery | compiler `analysis`; protocol conversion in `driver/lsp` |

Later phases never inspect raw generic-bound syntax after resolution. They consume specialized
interface identities or an explicit absence/ambiguity result.

## Verification

Focused tests cover parsing and canonical formatting, duplicate and ambiguous bounds, conditional
conformance presence and absence, generic forwarding, adapter order and exact remaining counts,
move-only cleanup, region escape, allocation context, builder growth, incomplete editor input, and
repository/packaged-home agreement.
