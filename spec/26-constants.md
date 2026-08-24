# Compile-Time Constants

This file is part of the Nocter language specification.
The specification entry point is [README.md](README.md).

## Declaration

A constant gives one immutable, storage-independent value a semantic name.

```nct
const retry_limit: usize = 4
pub const protocol_name: &str = "nocter"
```

The type annotation is mandatory. The initializer must be a constant expression whose type is
exactly the declared type. A constant has no address, ownership state, destructor, allocation
context, or result provenance. Referring to it produces its value; it does not create a place that
can be borrowed, assigned, moved from, or dropped.

The supported constant types are `bool`, the built-in signed and unsigned integer types, and
readonly `&str`. A constant `&str` refers to static text embedded in the program. Owned values,
nominal values, pointers, mutable borrows, slices, optionals, fallible values, callables, and
generic-dependent values are not constant types.

Visibility and target directives apply in the same way as for other targetable declarations.
Constants occupy the ordinary value namespace and may be selected through `use`, direct
`see`, or a module-qualified reference.

## Contract and Initializer Separation

A visible bodyless declaration in `index.nct` may act as a public contract:

```nct
//! index.nct
see ./limits.nct

pub const buffer_size: usize
```

The reciprocally seen implementation source supplies exactly one private initializer with the
same name, type, and module:

```nct
//! limits.nct
see ./index.nct

const buffer_size: usize = 4096
```

The pair denotes one constant identity. Contract joining sees only sources selected for the current
target, so a target-gated private initializer may complete a target-independent public contract in
the same way as a target-gated callable body. A missing, duplicate, or mismatched initializer is an
error. A bodyless private constant and a bodyless constant in an implementation source are errors.
An initialized public constant may remain inline when the value itself is the clearest contract.

## Constant Expressions

A constant expression may contain:

- boolean, integer, and non-interpolated string literals;
- references to constants, including forward and module-qualified references;
- grouping;
- `!` and integer negation;
- integer arithmetic, remainder, shifts, equality, and ordering;
- boolean `&&` and `||`, with ordinary short-circuit behavior;
- an integer `as` conversion when the evaluated value is representable by the destination type.

Function and method calls, construction, interpolation, allocation, mutation, borrowing, moves,
control expressions, outcome propagation, user-defined operators, and runtime values are not
constant expressions. Constant dependencies form a directed graph. A dependency cycle is an
error even when source order would otherwise permit one of its names to resolve.

Integer overflow, division by zero, an invalid shift count, and a conversion whose value is not
representable are compile errors. Left-shift bit loss follows the fixed-width shift rule and is not
integer overflow. Signed minimum values such as `-128` for `i8` are valid. Boolean short-circuiting
means an unevaluated right operand does not cause an arithmetic failure, but both operands must
still be well-typed constant expressions and every authored dependency still participates in cycle
detection.

## Fixed-Array Lengths

The length in `[T; expression]` is a constant expression with expected type `usize`:

```nct
const lane_count: usize = 4
const block_count: usize = 2

type Block = [u8; lane_count * block_count]
```

Each fixed-array length is evaluated once by the semantic context that owns its name scope. A
declaration-header length uses header imports and bound header types. A body annotation uses the
exact lexical scope at that annotation, including block imports:

```nct
func receive(): void {
    use ./protocol.{Byte, frame_width}

    let frame: [Byte; frame_width] = []
    return
}
```

Both contexts use the same constant-expression typing, arithmetic, conversion, short-circuit, and
failure rules. Declaration lowering does not inspect body blocks, and body checking does not retry
name lookup in a header namespace. Later lowering and code generation consume only the normalized
fixed-array type.

## Tooling

Hover presents the canonical evaluated value, not the initializer's original spacing or numeric
spelling. Definition, references, rename, completion, and semantic highlighting use the same
constant identity as compilation. A constant completion item is classified as a constant, and its
semantic highlight is readonly.

## Future Direction

This chapter does not define constant functions, associated or interface constants, constant
generic parameters, named static storage, addressable globals, or compile-time construction of
owned `String` and `Vec` values. Those features require separate storage and evaluation contracts;
they are not compatibility aliases for `const`.
