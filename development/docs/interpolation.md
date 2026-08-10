# Owned String Interpolation and Formatting

This document owns the compiler and distributed-standard-library design for extensible string
interpolation. Public behavior belongs to
[Strings, Arrays, Views, and Pointers](../../spec/07-strings-arrays-views-pointers.md) and
[Practical Standard Library](../../spec/21-practical-standard-library.md). The v0.12.0 Phase 0
acceptance record belongs to [the active milestone](../milestones/v0.12.0.md).

## Boundary

An interpolated source form constructs an owned `String` in the current aborting allocation
context. A plain string literal remains a static `&str`.

```nct
let message = "hello ${name}, count = ${count}"
```

Formatting is an ordinary static interface contract:

```nct
pub interface Format {
    pub method &self.format_into(output: &+String): void
}
```

`std/fmt` supplies source conformances for `str`, `String`, `bool`, and every built-in integer.
Project-owned nominal types participate through an explicit conformance to the exact selected
standard-library interface. No compiler table enumerates interpolatable types.

## Trusted Runtime Capability

The frontend validates one atomic capability from the selected Nocter home. It records semantic
identities for:

- the owned `std/string.String` type;
- zero-capacity construction in the current allocation context;
- the public `std/fmt.Format` interface;
- its single readonly `format_into(output: &+String): void` contract method.

Validation checks the selected package, module, declaration kinds, visibility, generic shape, and
complete callable shape. A project interface with the same spelling has no trusted role. The
capability stores no concrete formatter functions.

## Conformance Surfaces

Built-in types remain compiler identities rather than synthetic nominal declarations. A shared
`BuiltinTypeOwner` registry maps `str`, slices, `bool`, and every integer to source-backed resolver
surfaces. The exact implicit standard-library package may attach ordinary interface conformances
to those surfaces; project packages cannot define competing built-in conformances.

Nominal conformances declared by a loaded standard-library source are package-coherent. Resolver
qualification attaches each conformance to matching canonical type-symbol views across module
boundaries. This is required for `String: Format`, whose type and conformance belong to different
standard modules. Project conformances retain ordinary source and import ownership.

Conformance lookup, interface method lookup, associated-type normalization, generic
specialization, buildability, and editor analysis all read the same resolver records.

## Semantic Plan

Type checking creates one immutable interpolation plan per expression. It contains:

- the owned result declaration and constructor identity;
- the exact `Format` interface declaration;
- each decoded text or expression span;
- the accepted source type;
- a resolved protocol-method plan containing the selected method declaration, concrete `Self`,
  receiver mode, callable target, and free type parameters.

The protocol-method record is shared with iterator conversion and stepping. Generic `where T:
Format` bodies and concrete conformances therefore use the same substitution and reachable-call
specialization machinery. Missing conformance is a type-checking error before buildability or IR.

## Lowering and Cleanup

IR consumes only the semantic plan. It does not search for `Format`, `format_into`, `append_*`, or
type names. Each part becomes an ordinary static method call with a readonly receiver followed by
the mutable output borrow.

Readonly receiver preparation reuses common expression lowering:

- `str` keeps its ordinary two-word view representation;
- scalar places borrow their existing ABI word;
- computed scalars materialize once in temporary readonly storage;
- aggregate places borrow their stable slot or parameter location;
- aggregate results initialize a temporary slot and remain live through the call.

A move-only temporary is destroyed exactly once immediately after its formatting call. Existing
values are not consumed and remain usable. Partial `String` cleanup uses the ordinary pending-drop
model on propagation or control-flow exit. Allocation abort does not unwind Nocter scopes.

## Standard-Library Policy

`format_into` uses the ordinary aborting `String` surface. Its conformances delegate to `append_*`
operations. Explicit builders may use paired `try_append_*` operations to recover allocation
failure; interpolation itself remains `String`, not `String!`.

Scalar conformances receive `&self`. A package-private generic copy helper implements the copy from
that borrow using existing typed pointer operations. It does not add a formatting primitive or
special scalar ABI path.

## Editor Boundary

Hover derives the result and accepted input from the semantic interpolation plan and names the
`Format` contract. Nested hover, completion, and signature help inside `${...}` remain ordinary
expression analysis. Source declarations and conformances retain their real definition,
reference, rename, completion, and semantic-token identities; the LSP does not perform a second
formatting lookup.

## Verification Boundary

Focused IR tests prove call ordering and partial-result cleanup. Distributed-home tests prove:

- exact output for text, `String`, `bool`, and every integer boundary;
- user nominal, generic-bound, and imported conformance dispatch;
- continued use of an existing value after interpolation;
- exactly-once destruction of a move-only temporary;
- rejection of a same-spelling project interface and project-owned built-in conformance;
- lexical-region escape rejection and allocation-abort behavior;
- compiler-backed hover, completion, and signature recovery.

## Non-goals

- width, precision, radix, locale, debug, or user-defined format specifiers;
- automatic derived conformance;
- runtime interface objects or dynamic formatting dispatch;
- optional, fallible, pointer, callable, array, or opaque formatting without a legal explicit
  conformance;
- recoverable interpolation syntax;
- Unicode scalar or grapheme formatting;
- native targets other than `arm64-darwin`.
