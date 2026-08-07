# Typed Literals and Composable Element Packs

This document owns the compiler design for v0.3.0 Phase 1 typed literal definitions and v0.3.0
Phase 8 composable sequence packs. Public semantics belong to
[Literal Definitions and Sequence Spread](../../spec/17-literal-definitions-sequence-spread.md),
and the completed gates belong to the [v0.3.0 Release Record](../releases/v0.3.0.md).

## Separate Concepts

| Concept | Meaning |
|---|---|
| literal shape | compiler-defined source delimiter category such as sequence `[]` or string `""` |
| literal definition | ordinary source body attached to one nominal declaration and one shape |
| literal expression | construction syntax that resolves to exactly one visible definition |
| element pack | compiler-owned, non-escaping sequence of fixed-value and spread-iterator segments |
| allocation-context override | optional established aborting context selected before element evaluation |

The compiler must not infer literal behavior from nominal names such as `Vec` or `String`, method
names, private representation, or delimiter-adjacent text rewriting.

## Definition Identity

The parser records literal shapes without assigning nominal meaning. Resolution keys each
definition by nominal declaration identity and shape; parameter types, generic arguments, and
visibility never create an overload set. Expression resolution selects that exact key and stores
the declaration identity for typechecking, lowering, navigation, and presentation. Public shape and
spelling rules remain exclusively in the linked specification chapter.

## Element Pack

`...items: T` introduces an owned ephemeral element pack. It is not `[T]`, a slice, an allocated
collection, a normal ABI parameter, or a general variadic parameter. Phase 8 composes the pack from
fixed values and statically resolved iterator segments without changing that non-escaping model.

The pack supports only the operations needed by the Phase 1 literal body:

- `items.len()` reads one checked total cached before body execution
- `for item in items` consumes elements once from left to right
- each loop binding is an ordinary owned `T`
- Phase 1 rejects `break` or `continue` that directly targets this consuming loop; nested ordinary
  loops retain their normal control flow
- an unconsumed element retains its drop obligation
- the pack itself cannot be returned, assigned outside the body, stored in an aggregate, borrowed
  beyond the body, or passed to an ordinary callable

Phase 8 accepts three sequence-element segment forms:

- `...source` creates a readonly iterator and copies each referent; its item must be `Copy`
- `...&source` creates a readonly iterator and contributes the readonly references themselves
- `...move source` transfers a collection or direct owning iterator and contributes owned items

Every spread iterator must satisfy the trusted `Iterator<T>` and `ExactSizeIterator<T>` roles. An
unknown-size iterator is rejected before lowering; the compiler never materializes a hidden
collection to recover a length.

Lowering owns a `LiteralElementPack` fact rather than pretending the pack has a public Nocter type.
Diagnostics may display `literal pack of T`, but resolver and typechecker identify it by binding
identity.

## Evaluation and Cleanup

Construction order is fixed:

1. resolve and evaluate an optional `using` place
2. install the selected allocation context for the construction
3. prepare fixed-value and spread-iterator segments once from left to right
4. enter the hidden literal implementation and call each spread's exact-count target once in
   segment order
5. cache the checked sum of fixed values and exact spread counts
6. enter the literal body and stream each segment only when pack iteration requests an item
7. transfer each consumed item obligation to its loop binding
8. publish the completed result
9. drop the current item, remaining iterator suffixes, and later segments exactly once on exit
10. restore the surrounding allocation context

Failure during segment preparation drops the completed prefix in reverse order. `return`, `?`, and
other body exits drop the current loop item and unconsumed segments exactly once. Exact length is
independent of consumption: repeated `items.len()` calls return the cached initial total. The pack
is compiler-owned stack/runtime state and never causes an implicit heap allocation.

## Resolution and Typechecking

- The target must resolve to a nominal type declared in the same module as the definition.
- Visibility belongs to the definition; target visibility does not implicitly publish it.
- An expression resolves by target declaration identity and source shape.
- Generic arguments are inferred from the expected result and every element, using the ordinary
  specialization engine.
- An empty sequence requires explicit target arguments or sufficient expected type information.
- A string expression passes its decoded static `&str` value to the single declared parameter.
- Sequence capture is final and unique. Phase 1 does not implement required leading parameters.
- Result provenance, result allocation, and the internal execution allocation requirement use the
  ordinary callable-summary model while remaining separate facts.
- A literal declaration accepts the same `from` result-provenance clause after its return type as
  functions and methods. The clause is checked against the body, drives typed-literal call-site
  provenance, and appears in formatter and editor signatures.
- A string literal parameter may be an input origin, such as `from text`. A Phase 1 sequence
  element pack is not one borrow-like input identity and therefore is not an eligible named
  origin. A collection literal that may retain newly allocated storage declares `alloc`; it does
  not expose the ambient allocation context through `from`.

## Allocation Context

Without `using`, the literal body receives the current statically propagated aborting context.
`using place` selects a different established aborting allocator/context before any element is
evaluated. It does not alter the literal result type and never accepts `TryAllocator`.

Context override is a typed literal fact consumed by lowering. It is not a synthetic region, a
mutable global, a hidden source parameter, or a search for allocator names. Storage allocated by the
literal body carries the selected context's Phase 0 provenance.

## Compiler Ownership

| Responsibility | Owner |
|---|---|
| definition/expression syntax and recovery | `parser/literals` and dedicated AST nodes |
| declaration and shape identity | `resolve/literals` |
| specialization, pack rules, context validation | `typecheck/literals` |
| iteration conversion, exact count, and item projection | `typecheck/iteration` and immutable spread plans |
| element ownership and escape checks | ownership consuming typed literal facts |
| shared iterator step lowering | `ir/lower/collection_for` |
| pack lowering, cached length, and cleanup | `ir/lower/literal_packs`, `ir/lower/literal_pack_lengths`, and `ir/lower/typed_literals` |
| per-expression context override | `ir/lower/allocation_contexts` |
| editor-facing literal facts | `analysis/literals`; protocol conversion in `driver/lsp` |

AST consumers must handle literal nodes explicitly. They must not desugar them into calls before
resolution, because doing so loses source shape, pack ownership, and context-selection spans.

## Phase Boundaries

Phase 1 deliberately excluded sequence spread, iteration protocols, and interpolation. Phases 3,
7, and 8 subsequently added those features through shared callable, iteration, and pack facts.
Normal variadic callables, mapping/tuple/numeric/byte literal shapes, aggregate spread, embedding,
mutable spread, and recoverable literals remain outside the completed Phase 8 boundary.
