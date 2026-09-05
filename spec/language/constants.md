# Compile-Time Constants

## Declaration

A constant gives one immutable, storage-independent value a semantic name.

```nct
const RETRY_LIMIT: usize = 4
pub const PROTOCOL_NAME: &str = "nocter"
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
Every constant name uses ASCII `UPPER_SNAKE_CASE`: it begins with `A` through `Z`, contains only
uppercase letters, digits, and single interior underscores, and has no trailing underscore.
Constants occupy the ordinary value namespace. A direct `see` exposes a same-module constant
unqualified; an external module constant is reached through a module-qualified reference.
Selected `use` never imports a constant.

## Contract and Initializer Separation

A visible bodyless declaration in `index.nct` may act as a public contract:

```nct
//! index.nct
see ./limits.nct

pub const BUFFER_SIZE: usize
```

The reciprocally seen implementation source supplies exactly one private initializer with the
same name, type, and module:

```nct
//! limits.nct
see ./index.nct

const BUFFER_SIZE: usize = 4096
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
const LANE_COUNT: usize = 4
const BLOCK_COUNT: usize = 2

type Block = [u8; LANE_COUNT * BLOCK_COUNT]
```

Each fixed-array length is evaluated once by the semantic context that owns its name scope. A
declaration-header length uses header imports and bound header types. A body annotation uses the
exact lexical scope at that annotation, including block imports:

```nct
func receive(): void {
    use ./protocol.Byte
    use ./protocol

    let frame: [Byte; protocol.FRAME_WIDTH] = []
    return
}
```

Both contexts use the same constant-expression typing, arithmetic, conversion, short-circuit, and
failure rules while retaining their own lexical name scopes. Once a fixed-array length is resolved,
every later use observes the same normalized fixed-array type and never reinterprets the expression
in another scope.

## Immutable Static Data

`static` declares one immutable, addressable value whose initialized representation is embedded in
the executable before program execution:

```nct
static ASCII_LIMITS: [u32; 2] = [65, 90]

pub static PROTOCOL_MARKERS: [u8; 3]
```

A bodyless public declaration in a module root joins exactly one private definition with the same
declaration kind, name, and type, using the same contract/implementation rule as `const`. An
initialized public static may remain inline when its value is itself the intended public contract.

A static name uses `UPPER_SNAKE_CASE` and denotes a readonly place with `static` provenance. It can
be read when its type is copyable, indexed according to its type, or borrowed as `&STATIC_NAME`.
It cannot be assigned, moved, mutably borrowed, dropped, or used as an allocation context. No
runtime initializer or initialization-order relation exists.

The static-initializer domain contains:

- boolean, integer, character, and non-interpolated string literals;
- references to `const` values;
- the pure unary, binary, and conversion constant expressions defined for `const`;
- fixed-array literals whose elements recursively belong to this domain.

The declared static type must recursively contain only `bool`, integer types, `char`, readonly
`&str`, and fixed arrays of those types. Owned values, nominal values, pointers, mutable borrows,
slices, optionals, fallible values, callables, generic-dependent values, and values with destruction
are rejected. Every readonly string reference in a static initializer refers to embedded static
text.

A static initializer is evaluated exactly once before program execution. The selected target layout
determines its size, alignment, and byte encoding. The resulting representation is placed in
readonly mapped data; no source-level initializer evaluation occurs at runtime.

`const` remains a storage-independent value and does not become an alias for `static`. `static`
exists for immutable data whose address and indexed storage are part of execution.

## Tooling

Hover presents the canonical evaluated value, not the initializer's original spacing or numeric
spelling. Definition, references, rename, completion, and semantic highlighting use the same
constant identity as compilation. A constant completion item is classified as a constant, and its
semantic highlight is readonly.

Static declarations participate in parsing, formatting, tokens and AST output, diagnostics,
navigation, references, rename, completion, hover, and semantic highlighting through one static
identity. Hover renders the canonical declaration kind, name, and type, distinguishes `static`
storage from `const` values, and never prints an initializer's potentially large contents.

## Future Direction

This chapter does not define constant functions, associated or interface constants, constant
generic parameters, compile-time construction of owned `String` and `Vec` values, or mutable
globals.
