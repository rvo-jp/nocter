# Expansion and Iteration Architecture

This document owns the compiler architecture for source-defined expansion, collection `for`, and
typed-sequence spread. Public semantics belong to the
[Expansion Operators specification](../../../spec/23-expansion-operators.md).

## Boundary

Expansion and iteration are separate operations:

1. a readonly, readwrite, or owned expansion operator converts a collection into an iterator;
2. the trusted `Iterator` interface advances that iterator;
3. typed-sequence spread additionally requires the trusted `ExactSizeIterator` interface.

A source value that already conforms to `Iterator` uses the direct plan and performs no expansion.
The compiler never treats `iter`, `iter_mut`, `into_iter`, a collection type name, or a standard
module path as semantic evidence.

## Source-Owned Expansion

Parser and AST represent equality, index, and expansion as source-owned `OperatorDecl` variants.
Expansion has three stable internal callable identities, one for each receiver capability. Those
identities connect operator bodies to the ordinary method resolution, specialization, provenance,
ownership, and lowering pipelines. They are never presentation strings.

The common expansion selector receives:

- the concrete or generic source type;
- readonly, readwrite, or owned capability;
- the lexical `TypeEnvironment`, including `where (...source): result` evidence;
- resolver declarations and the source span.

It returns the exact iterator type and either a concrete operator declaration or lexical generic
evidence. Concrete call specialization reruns selection after generic substitutions, so a generic
plan never relies on an operator body having been specialized by an unrelated call. Generic call
inference also uses expansion requirements to infer result binders that do not occur in ordinary
parameters.

## Trusted Roles

`TrustedDeclarationFacts` validates only behavioral contracts used after expansion:

- `Iterator`, its `Item` associated type, and `next` method;
- `ExactSizeIterator` and its remaining-count method.

There is no trusted collection-conversion interface. `Iterable` and `IntoIterator` are not aliases,
fallbacks, or compatibility roles. Malformed or unavailable trusted iterator contracts produce a
source-backed availability diagnostic before lowering.

## Immutable Plans

Typecheck records one immutable collection plan containing:

- source mode: direct, readonly expansion, readwrite expansion, or owned expansion;
- source, iterator, and yielded item types;
- the selected expansion method when concrete;
- the selected iterator step declaration;
- binding and source spans.

A sequence-spread plan adds copy, readonly-reference, or move projection, the exact-count method,
and pack item type. Ownership, provenance, analysis, buildability, specialization, IR, and native
lowering consume these facts instead of repeating lookup.

Lexical generic evidence may leave the expansion method empty during generic body checking. After
context substitutions, both call-specialization collection and IR lowering use the same concrete
specialization helper to fill it. An absent method then means only direct iteration, never an
implicit name-based conversion.

## Mutable Iteration

Readwrite expansion owns the exclusive source loan for the iterator lifetime. `MutableViewIter<T>`
stores `&+[T]`, advances a monotonically increasing index, and yields `&+T?`. The loop binding is a
first-class aggregate borrow when `T` is aggregate; it is not copied into a synthetic aggregate
slot.

IR models aggregate fields through `AggregateLocation::Borrow`. Address calculation, field loads
and stores, aggregate copy, call result locations, syscall results, validation, and parameter spill
discovery all consume that location kind. This keeps borrowed aggregate mutation on the common
aggregate path instead of adding a mutable-iteration backend exception.

An existing `&+T` value is a writable source for `&+` reborrowing. Expression typing flattens that
reborrow to `&+T`, rather than constructing `&+&+T`; readonly reborrowing similarly reduces
capability. Collection iteration therefore works across a function parameter such as
`values: &+Vec<T>` as well as a local `var` binding.

Each item loan ends before the next step. `continue`, `break`, `return`, propagation, normal body
completion, and iterator cleanup use the ordinary ownership, liveness, region, and drop machinery.
There is no iterator-specific runtime token or parallel ownership state.

## Sequence Spread

Bare and explicit readonly spread use readonly expansion. Consuming spread uses owned expansion or
a direct owning iterator. Each selected iterator must satisfy `ExactSizeIterator`; the total pack
length is checked and cached before the literal body executes.

Readwrite spread is rejected before planning. A literal pack can retain all elements at once, so it
would require a pairwise-disjoint provenance proof that mutable collection iteration does not need.

## Editor Presentation

AST-backed declarations, resolved method presentation, and implicit iteration hover all render the
authored operator form. Type labels go through the shared presentation service so workspace hover
uses imported or short names rather than canonical module paths. Completion offers the three
missing receiver forms independently. Operator declaration tokens use the exact `...` span and
private callable names remain filtered from members, hover, semantic tokens, and navigation.

## Responsibility Map

| Responsibility | Owner |
|---|---|
| operator AST, JSON, and callable identity | `ast/operators` and `ast/json/items` |
| declaration and requirement grammar | `parser/items/operators` and `parser/generic_requirements` |
| canonical formatting | `format/items` |
| selection and concrete specialization | `typecheck/expansion` |
| loop and spread protocol planning | `typecheck/iteration` and typecheck facts |
| source loans, item moves, and cleanup | `typecheck/ownership` and provenance/region analyses |
| iterator construction and stepping | `ir/lower/collection_for` and `ir/lower/literal_packs` |
| borrowed aggregate storage | `ir/model`, lowering locals, and backend aggregate-value modules |
| trusted step and exact-count validation | `target/trusted_iteration` and `semantics` |
| standard implementations | `std/iter`, `std/vec`, and collection-owned instances |
| hover, completion, tokens, and declarations | `analysis/iteration`, `analysis/presentation`, and editor indexes |

## Verification

The gate covers readonly, readwrite, owned, and direct iteration; generic requirement inference and
specialization; nested and empty loops; source loan conflicts; aggregate element mutation; cleanup
on every exit; copy, borrow, move, and direct-iterator spread; unknown-size and mutable-spread
rejection; formatter and JSON stability; exact editor ranges and normalized labels; distributed
Nocter-home execution; and all public examples.
