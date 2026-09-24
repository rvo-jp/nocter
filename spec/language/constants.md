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

The supported constant types are `bool`, `char`, the built-in signed and unsigned integer and
floating-point types, and readonly `&str`. A constant `&str` refers to static text embedded in the
program. Owned values, nominal values, pointers, mutable borrows, slices, optionals, fallible
values, callables, and generic-dependent values are not constant types.

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

- boolean, integer, floating-point, character, and non-interpolated string literals;
- references to constants, including forward and module-qualified references;
- grouping;
- `!` and integer negation;
- integer arithmetic, remainder, shifts, equality, and ordering;
- boolean `&&` and `||`, with ordinary short-circuit behavior;
- a lossless numeric `as` conversion;
- direct calls to `const func` and readonly or owned `const method` declarations.

Construction, interpolation, allocation, mutation through aliases, runtime storage, dynamic
dispatch, asynchronous work, blocking operations, outcome propagation, destruction, and calls to
runtime-only declarations are not constant expressions. Constant dependencies form a directed
graph. A dependency cycle is an error even when source order would otherwise permit one of its
names to resolve.

Integer overflow, division by zero, an invalid shift count, and a conversion whose value is not
representable are compile errors. Left-shift bit loss follows the fixed-width shift rule and is not
integer overflow. Signed minimum values such as `-128` for `i8` are valid. Boolean short-circuiting
means an unevaluated right operand does not cause an arithmetic failure, but both operands must
still be well-typed constant expressions and every authored dependency still participates in cycle
detection.

## Compile-Time Callables

`const` before `func` or `method` promises that the ordinary callable implementation is also
available to compile-time evaluation:

```nct
const func increment(value: i32): i32 {
    return value + 1
}

const ANSWER: i32 = increment(41)
```

This modifier does not create a second function or make the callable compile-time-only. Runtime
calls use the same declaration and body. The compiler first performs ordinary name resolution,
type checking, generic specialization, operator selection, ownership checking, and dispatch
selection. Compile-time evaluation consumes those completed decisions and never resolves the
source again.

A `const` callable body may use immutable scalar, static-text, tuple, and fixed-array values;
parameters and immutable locals; primitive arithmetic, comparisons, and lossless conversions;
blocks, conditionals, short-circuit logic, and returns; and direct calls to other compatible
`const` callables. A generic `const` callable is admitted when a call supplies one closed supported
specialization. Recursive calls are permitted within deterministic evaluator work and call-depth
limits.

The modifier is a promise, not an inference from the current implementation. A body containing an
unsupported operation rejects the declaration even when no initializer currently calls it. A
runtime-only callable cannot acquire compile-time capability through assignment or interface
adaptation; the capability may only be forgotten.

An `async` declaration cannot carry `const`: invoking it creates a future rather than evaluating
its body result, and the compile-time evaluator neither constructs nor drives futures. A primitive
function also cannot carry `const` because it has no checked source body. A future language version
may define explicit compile-time intrinsics, but runtime primitive registration does not imply one.

## Fixed-Array Lengths

The length in `[T; expression]` is a structural constant expression with expected type `usize`:

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

Structural constant expressions are the call-free subset needed before declaration types are
complete. They may use literals, scalar operators, lossless conversions, and references to other
structural constants. A constant whose initializer calls a `const` callable remains valid as an
ordinary value, but it cannot determine a fixed-array length. This boundary prevents type
construction from invoking body checking before declaration types exist.

## Structural Constant Arguments

A `usize` constant generic argument uses the same structural constant-expression evaluator as a
fixed-array length. A closed argument is evaluated once while declaration types are normalized.
Its checked value, not its source expression, becomes part of type identity:

```nct
const WORD_BYTES: usize = 4

type First = Buffer<u8, WORD_BYTES>
type Second = Buffer<u8, 2 + 2>
```

`First` and `Second` denote the same specialization. A bare reference to a visible `usize` constant
parameter is represented as a symbolic parameter term and substitution replaces that term with the
caller's checked value. v0.57.0 does not accept arithmetic that remains symbolic, such as `N + 1`;
introducing normalized symbolic expression identity is separate from the fixed-capacity model.
Closed arithmetic is still evaluated at the authored argument or length that owns the expression.

Inside its declaration body, a `usize` constant parameter is also a readonly,
storage-independent value:

```nct
func capacity<T, const N: usize>(values: &[T; N]): usize {
    return N
}
```

It has no address and cannot be assigned, borrowed, moved, or captured as runtime storage. Its
semantic identity remains symbolic while the generic body is checked. Each executable or
compile-time specialization then supplies the one closed value from its canonical generic
arguments; lowering does not add a hidden runtime parameter or evaluate source text again.

The evaluator remains the sole authority for literals, constant references, arithmetic,
conversion, overflow, and target-width checks. Type construction does not copy those rules, and
layout, ABI lowering, code generation, and editor presentation cannot evaluate or parse an
argument again.

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
- direct calls to compatible `const` functions and methods;
- tuple literals whose elements recursively belong to this domain;
- enumerated fixed-array literals whose elements recursively belong to this domain.

The runtime repeat form `[value; length]` is not part of the v0.57.0 compile-time initializer
domain. Static data that repeats a value writes the elements explicitly until repeat evaluation is
admitted by the compile-time value model itself.

The declared static type must recursively contain only `bool`, integer types, `char`, readonly
`&str`, tuples, and fixed arrays of those types. Owned values, nominal values, pointers, mutable
borrows, slices, optionals, fallible values, callables, generic-dependent values, and values with
destruction are rejected. Every readonly string reference in a static initializer refers to
embedded static text.

A static initializer is evaluated exactly once before program execution. The selected target layout
determines its size, alignment, and byte encoding. The resulting representation is placed in
readonly mapped data; no source-level initializer evaluation occurs at runtime.

`const` remains a storage-independent value and does not become an alias for `static`. `static`
exists for immutable data whose address and indexed storage are part of execution.

## Tooling

Hover presents the canonical evaluated value, not the initializer's original spacing or numeric
spelling. Definition, references, rename, completion, and semantic highlighting use the same
constant identity as compilation. Constants and constant parameters have readonly value
highlighting and constant completion classification; a constant-parameter hover identifies its
`usize` parameter contract rather than one caller's specialization.

Static declarations participate in parsing, formatting, tokens and AST output, diagnostics,
navigation, references, rename, completion, hover, and semantic highlighting through one static
identity. Hover renders the canonical declaration kind, name, and type, distinguishes `static`
storage from `const` values, and never prints an initializer's potentially large contents.

## Future Direction

This chapter does not define associated or interface constants, constant parameters other than
`usize`, constant defaults or predicates, compile-time construction of owned `String` and `Vec`
values, or mutable globals.
