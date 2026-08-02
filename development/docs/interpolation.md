# Owned String Interpolation and Formatting

This document owns the compiler and standard-library design for v0.3.0 Phase 3. Public string and
formatting semantics belong to [Strings, Arrays, Views, and Pointers](../../spec/07-strings-arrays-views-pointers.md)
and [Standard Library, Primitives, and OS](../../spec/11-stdlib-primitives-os.md). The active gate
belongs to the [v0.3.0 Development Contract](v0.3.0.md).

## Boundary

Phase 3 promotes an interpolated string from a check-only expression to an ordinary buildable
owned `String`:

```nct
let message = "hello ${name}, count = ${count}"
```

A non-interpolated string literal remains a static `&str`. Interpolation allocates through the
current aborting allocation context and returns `String`, not `String!`. Recoverable formatting is
available only through explicit `try_*` standard-library operations.

## Runtime Capability

Lowering must not search for `String`, `with_capacity`, `append`, or `std/fmt` by source spelling.
The frontend validates an atomic interpolation runtime capability from the trusted Nocter home.
The capability records declaration identities for:

- the owned string type
- zero-capacity construction in the current allocation context
- text and owned-string append operations
- boolean append
- every supported signed and unsigned integer append operation

Validation checks module identity and the complete declaration signature. A missing or mismatched
member rejects interpolation before IR lowering. User declarations with the same names have no
compiler-defined behavior.

The capability is compiler input, not a public source symbol. Typecheck uses it to select the
result type and formatting operation. IR uses the same declaration identities to emit ordinary
calls. Analysis exposes the resulting facts without reconstructing the capability from names.

## Semantic Plan

Typecheck produces one interpolation plan per expression. It contains:

- the owned result type and current-allocation effect
- current-context result provenance
- the selected append declaration for each decoded text or expression part
- the concrete input type and evaluation mode for each expression part
- source spans used by diagnostics and editor queries

`&str` is copied as a view. An existing `String` place is borrowed for the append and remains owned
by the caller. A temporary `String` remains live through its append and is then dropped. Scalars
are copied. Unsupported optional, fallible, aggregate, pointer, enum, and nominal values are
rejected before lowering.

The plan is the only semantic input to interpolation lowering. IR does not repeat type dispatch or
resolve standard-library declarations.

## Standard-Library Formatting

`std/fmt` has paired policy surfaces:

```text
append_*      -> void, allocation failure aborts without unwinding
try_append_*  -> void!, allocation failure is returned
```

Both surfaces share the fallible implementation core. Signed formatting is centralized around an
`i64` digit implementation and unsigned formatting around `u64`; narrower types use lossless
widening. `isize` and `usize` use their target-width signed or unsigned implementation. Minimum
signed values are handled without overflowing negation.

Interpolation uses only the aborting surface. Explicit recoverable builders may call the
`try_append_*` surface directly.

## Allocation and Provenance

The result starts with a zero-capacity `String` that retains the current allocator identity without
allocating. It must not start from a canonical root-owned empty buffer, because a later append must
still grow through the lexical region selected at the interpolation site.

The interpolation expression therefore always has a current-allocation effect, including when all
rendered parts are empty. Its owned result carries the current allocation origin. A result created
inside a child region cannot escape that region directly or through an aggregate, optional, or
fallible channel.

## Evaluation and Cleanup

Decoded text and expression parts are processed in source order. Each expression is evaluated
exactly once. A part is appended before the next expression is evaluated.

The partially initialized result is an ordinary owned temporary. Normal completion publishes it.
`return`, propagation, and other exiting edges before publication drop the partial result and any
live expression temporary according to the existing scope-drop model. Allocation abort does not
unwind Nocter scopes.

## LSP Boundary

`analysis/interpolation` exposes owned results derived from the semantic plan. LSP presentation may
show:

- the concrete `String` result type
- current allocation effect and result origin
- the accepted type of an interpolation part
- the same diagnostics as command-line checking

Completion, hover, and nested call signature help inside `${...}` reuse ordinary expression
analysis. Incomplete `${...` recovery must preserve cursor identity and must not invent a result
when the trusted runtime capability or expression type is unresolved.

## Non-goals

- user-defined formatting interfaces or implicit `to_string` lookup
- formatting options, width, precision, radix, locale, or debug representations
- optional, fallible, collection, pointer, enum, or arbitrary nominal formatting
- recoverable interpolation syntax or ambient `TryAllocator`
- sequence spread, variadic calls, collection `for`, or iterator adapters
- Unicode scalar or grapheme APIs
- native targets other than `arm64-darwin`
