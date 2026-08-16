# Explicit Iteration and Collection Access

This document owns the compiler and standard-library design for v0.3.0 Phase 2. Public semantics
belong to [Strings, Arrays, Views, and Pointers](../../spec/07-strings-arrays-views-pointers.md), and
the completed gate belongs to the [v0.3.0 Release Record](../releases/v0.3.0.md).

Phase 2 completed on 2026-08-02. This document records the implemented boundary; it is no longer an
active checklist.

## Separate Concepts

| Concept | Meaning |
|---|---|
| readonly view iteration | an allocation-free cursor borrowing contiguous source storage |
| owned vector iteration | a cursor that owns vector storage and moves elements out once |
| iterator result provenance | the source storage origin retained by a returned element borrow |
| initialized prefix | completed vector elements currently owned by `Vec<T>` |
| remaining iterator range | initialized elements still owned by `VecIntoIter<T>` |
| transient shift hole | one temporarily uninitialized slot during a non-fallible insert/remove shift |

Iterator type names and method names have no compiler meaning. Resolution uses ordinary declaration
identity, generic specialization, receiver capability, and callable provenance summaries.

## Readonly Iteration

`ViewIter<T>` stores a readonly `&[T]` and the next index. Construction does not allocate. Its
`next()` method advances the cursor only after establishing the returned element borrow.

```nct
pub struct ViewIter<T> {
    view: &[T]
    next: usize
}

instance ViewIter<T> {
    pub method &+self.next(): &T? { ... }
}
```

The result origin is the stored view's origin, not the iterator object's stack location and not an
untracked raw pointer. Returning the iterator from a helper maps that origin through the ordinary
aggregate and callable-summary model. A raw pointer may be used by trusted lowering but cannot be
the semantic source of provenance.

Distributed std provides a constructor from `&[T]`, `Vec<T>.iter()`, and `String.bytes_iter()`.
Until structural inherent methods on built-in view types have their own reviewed language design,
`&[T]` uses the named constructor rather than compiler or resolver special casing.

## Owned Iteration

`VecIntoIter<T>` owns the original raw storage and a remaining half-open range. Converting from
`Vec<T>` transfers storage and every initialized element obligation into the iterator, then leaves
the consumed vector with no live element or storage obligation.

`next()` removes the lowest remaining element, advances the range, and returns ownership as `T?`.
The iterator destructor drops the still-owned range in reverse index order. `RawBuffer` then performs
the one final storage release through its ordinary backend identity.

No byte copy may leave two live semantic owners. Extracted elements are removed with the same
trusted take operation used by `Vec.pop()`.

## Access and Mutation

`[T].get(index)` and `[T].get_mut(index)` return optional borrows. `Vec<T>` reaches both through its
readonly and readwrite slice coercions, retaining the vector storage origin without copying generic
elements. Trapping `values[index]` uses the same slice coercions and remains a distinct contract.

`try_insert` completes all fallible capacity work before shifting elements. After capacity succeeds,
the shift contains no fallible call or early control edge. It maintains one transient hole, moves
the hole toward the insertion index, fills it with the new value, and publishes `len` last.

`remove` takes the selected element, moves later elements left through one transient hole, decrements
`len`, and returns the removed owner. Bounds absence returns `none` before creating a hole. This is
not a sparse vector model: no reachable function boundary or cleanup edge can observe the temporary
hole.

## Compiler Boundaries

| Responsibility | Owner |
|---|---|
| element-place type and borrow checks | `typecheck/arrays` and ownership place analysis |
| aggregate/view provenance projection | `typecheck/provenance` and callable summaries |
| optional borrowed/owned return lowering | `ir/lower/optional_fallible` and expression lowering |
| raw element borrow/take/store/drop operations | focused `ir/lower` pointer operation modules |
| remaining-range cleanup | ordinary `VecIntoIter<T>` drop body plus recursive drop lowering |
| editor-facing specialization/provenance | `analysis` facts; protocol conversion in `driver/lsp` |

Buildability must reject an iterator body before IR if a required general operation is not promoted.
IR and backend code do not inspect `ViewIter`, `VecIntoIter`, `iter`, `next`, `get`, or `remove`
spellings.

## LSP Boundary

Existing compiler-backed method queries must specialize `T` from the receiver. Hover and signature
help show `&T?` or `T?` exactly. Completion respects `&self` versus `&+self`, so repeated `next()`
requires a writable iterator binding. Provenance detail comes from callable semantic facts.

Incomplete `.next(` edits reuse ordinary member and call recovery. Phase 2 does not add an
iterator-specific recovery parser.

Zero-argument calls and calls waiting for a first argument are syntactically ambiguous while open.
Generic signature recovery therefore tries both a closed empty call and a typed placeholder
overlay, then accepts the first compiler analysis that resolves a callable. This also improves
ordinary non-iterator calls.

## Implemented Foundations

- borrowed local and call ABI values preserve concrete slice-element addresses and callable
  provenance
- optional and fallible control-flow wrappers preserve every possible returned borrow source
- generic `take_value_at_ptr` bindings specialize at the IR binding boundary and reuse ordinary
  scalar or aggregate drop tracking
- optional aggregate failure branches mark uninitialized destinations inactive before lowering
  `break`, `return`, and other fallback control flow
- distributed `std/iter`, `std/vec`, and `std/string` contain the public API, with owning iterator
  bodies in `std/vec/into_iter.nct`;
  compiler and backend code contain no iterator-name dispatch
