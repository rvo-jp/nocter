# Generic Arguments and Constant Parameters

## Scope

This chapter owns the adopted v2.0.0 source contracts for generic arguments, requirements and
constant parameters. [Constant evaluation](constant-evaluation.md) owns callable staging, inferred
eligibility, constants, immutable statics and evaluator limits. [Type foundations](values-and-types.md)
own nominal/structural identity and generic checking; [interfaces](interfaces.md) own operation
binding; [module contracts](modules.md) own publication and definition pairing. The
[grammar](syntactic-grammar.md#generic-arguments-and-requirements) owns recognition.

This is a design contract, not an implementation claim. Full generic proof, constant evaluation,
serialization and platform arithmetic must be qualified together before compiler admission.
In particular this chapter supplies no foreign primitive authority, dynamic interface object,
runtime type metadata, evaluator I/O, arbitrary compile-time allocation API or symbolic arithmetic
type solver.

## Parameter Schemas and Arguments

An ordered generic schema contains type parameters (`T`) and value parameters (`const N: usize`).
Names are unique across both kinds and cannot shadow an enclosing generic name. No defaults,
named arguments, partial lists or `_` argument placeholders are introduced. Constant parameters
other than `usize` require a separate extension. `void` is ordinary storable data and is a valid
type argument; `never` is not an arbitrary storable payload substitution.

```text
struct Fixed<T, const N: usize> {
    items: [T; N]
}

func decode<T>(bytes: Bytes): T!
let config = decode<Config>(bytes)?
```

Callable arguments belong after the callable name, including a member name. Owner arguments
belong after the owner. In `Table<Key>.from_entries<Entry>(entries)`, the two lists specialize different
schemas; each explicit list is complete for its own schema. The caller may independently omit
either list if its entire substitution can be inferred uniquely. Type-owned construction must
still deliver its owner; this example does not authorize a receiverless utility returning an
unrelated Entry.

Infer omitted arguments bidirectionally from selected receiver/input data, contextual closure
contracts and expected result data. Normalize established associated bindings to propagate those
constraints. Requirements validate that solution; they do not search implementations to guess
an unknown type. Names, later use, private body contents and allocation context supply no type
guess. An unknown or non-unique solution requires an explicit argument list or local expected
type. Do not choose a different target or passing operation to repair inference.

Generic argument syntax preserves a single source tree before its parameter kind is known. A
bare path, grouped path or array-shaped argument can have both a type and a constant-expression
reading. The selected declaration's parameter slot determines elaboration, not whether a name
happens to resolve as a type. A literal expression cannot satisfy a type slot; a type-only shape
cannot satisfy a constant slot. Reject a kind mismatch without reparsing or looking for another
generic declaration. Explicit call delimiters follow the syntactic jointness rules below.

A generic body is checked under its declared assumptions before executable specialization.
Substitution preserves selected operations and dependencies. Concrete Copy evidence at a caller
can create a snapshot into a take input; it cannot turn an already checked generic transfer into
a non-invalidating copy. Specialization creates no hidden runtime generic parameter or dictionary.

## Requirements and Associated Projections

Use `T impl Interface` for nominal capability, `copy T` for intrinsic Copy, `F: func(T): R` for
a structural callable contract and `T = Type` for type equality/refinement. `impl` does not accept
Copy, callable or execution-effect pseudo-interfaces. Associated bindings belong to their
immediately preceding interface application:

```text
where T impl Source { .Item = Item }, copy Item
```

Resolve subjects to generic parameters, `Self` where permitted by an interface prerequisite, or
their established associated projections. Reject duplicate requirements/bindings, unknown
associated declarations, contradictory equalities, non-finite normalization and requirements
that do not follow from the declared environment. Equality is semantic type equality, not a
request for layout compatibility or permission conversion. Binder refinement cannot rank
overlapping implementations. Every applicable operation/conformance must be unique over its
whole admitted domain; the compiler may reject an overlap it cannot disprove, with the competing
declarations identified. It cannot silently choose one specialization.

`T.Item` requires one owning associated declaration under one normalized interface application
in the selected capability environment. Different arguments of the same generic interface can
therefore require selection too, such as `T.(Source<Key>.Item)`.
When distinct interfaces expose Item, use `T.(Source.Item)` to select the declaration. This is
the type-position counterpart of the existing interface operation selector; it does not add a
second spelling for unique lookup. Qualification is required for a collision and rejected when
redundant. Equal normalized types do not merge distinct associated identities. A prerequisite
diamond reaching the same declaration through the same normalized application is not a collision.
The selected interface application
must already be justified; the selector cannot invent a new requirement or conformance.

[Comparison requirements](comparisons.md#selection-and-requirements) name the fixed equality or
strict-comparison operation. Projection, coercion and expansion requirements use their respective
owning contracts. They carry input operations separately from ordinary data types.
Authored `noalloc`, `notrap` and `blocking` requirements are invalid. Inferred cleanup/effect
evidence is independent of Copy and must survive generic forwarding and specialization.
Declared semantic laws are not additional type equality or memory-safety evidence.

## Constant Parameters and Type Identity

A fixed-array length and a constant argument require a target `usize` value from the same
[compile-time evaluator](constant-evaluation.md) as a value initializer. A closed supported const
call is not excluded merely because the result determines a type:

```text
const func width(): usize {
    var result: usize = 1
    var index: usize = 0
    while index < 6 {
        result *= 2
        index += 1
    }
    result
}

// Ordinary implementation source; a public root would declare only the constant header.
const WIDTH: usize = width()
type Block = [u8; WIDTH]
```

Closed arguments compare by evaluated typed value. `[u8; 2 + 2]` and `[u8; 4]` have the same
array type. A visible constant parameter used alone retains its own symbolic identity until
substitution supplies the value. Arbitrary arithmetic still containing such parameters,
including `N + 1` or a const call requiring symbolic N, is not admitted in type identity by this
extension. Supporting ordinary closed evaluation is not adoption of a symbolic theorem prover.

Within a generic body N is also a readonly storage-independent value, available to ordinary
computation after substitution. It has no source storage to alias, mutate or transfer. `let n = N`
constructs an ordinary value; `read n = N` does not acquire a place. Capturing N as a constant
uses the substituted value, not a fictitious runtime generic environment field.
