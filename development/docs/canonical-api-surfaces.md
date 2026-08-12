# Canonical Standard-Library API Surfaces

This document owns the implementation architecture for v0.13.0 Phase 5. Public standard-library
behavior belongs in `spec/`; the milestone scope and completion gate belong in
`development/milestones/v0.13.0.md`.

## One Public Identity

Every semantic operation has one public declaration identity. The declaration kind follows the
operation rather than implementation convenience:

| Operation | Canonical declaration |
|---|---|
| create a named type | member of `construct Type` |
| observe or mutate one receiver | member of `instance Type` |
| participate in language syntax | `operator` member |
| expose an implicit borrowed view | `coerce Type` member |
| support generic named dispatch | `interface` member |
| obtain a global capability or unnamed source | free function |

A body helper may remain a module-private `func`. A helper used across the implicit standard
package may use `pub(/)`. Neither form is a second user API.

## Semantic Equivalence Test

Two declarations are removable duplicates only when all of these properties match:

- principal receiver or construction target;
- input and result types after ordinary specialization;
- readonly, readwrite, or consuming capability;
- success, optional, fallible, or trapping behavior;
- allocation and ownership transfer;
- result provenance;
- externally observable ordering and cleanup.

For example, `slice[index]` traps and `slice.get(index)` returns `none`, so both belong in the
canonical surface. An expansion operator and `iter()` may produce the same iterator, but expansion
cannot be called as a general expression, so the named method remains necessary for lazy chains.

## Borrowed Observation

The borrowed representation owns observation behavior. `[T]` owns length, emptiness, pointer,
optional access, equality, and ordering. `str` owns UTF-8 view observation and search. `Vec<T>` and
`String` expose initialized readonly views through coercion rather than forwarding each borrowed
method.

One-step coercion selection already records the exact target declaration, owner provenance, and
receiver adjustment. Standard-library nominal types must not duplicate an operator merely to make
that existing path visible in hover or completion.

## Generic Contracts

An interface represents named dynamic structure required by generic source. It is not a collection
of aliases for methods already unified by a view or structural operator requirement. `Iterator`,
`ExactSizeIterator`, `Format`, `Reader`, and `Writer` retain independent contracts.

`Sequence<T>` does not. Its only conformance repeats `Vec<T>` observation, its only algorithm is
`first`, and it cannot state the access-complexity property that binary search would need. Slice
methods plus borrow coercion replace it.

## Migration Discipline

Phase 5 is a source-breaking cleanup before v1. Removed exports receive no aliases, deprecated
wrappers, parser compatibility, resolver rewrite, or compiler diagnostic special case. Tests move
to canonical APIs. Focused negative coverage proves obsolete exports are absent through ordinary
module visibility and name resolution.

LSP services consume the same resolved declarations as compilation. They do not synthesize legacy
functions or nominal methods to preserve an old presentation.
