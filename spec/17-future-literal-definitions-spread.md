# Future Literal Definitions and Spread

This file is part of the Nocter language specification.
The specification entry point is [README.md](README.md).

This chapter is a future design note. It is not part of Nocter v0. The v0
contract is fixed in [Nocter v0 Contract](00-v0-contract.md).

The direction in this chapter is adopted as a post-v0 design target, but the
exact parser, typechecker, ownership, and lowering rules must be finalized
before implementation.

## Purpose

Adopted future direction: Nocter should allow nominal types to define how they
are constructed from a small set of literal shapes.

The design exists to make standard-library and user-defined collection or value
types feel direct without making the compiler know about every collection type.
It also keeps construction encapsulated: the type author exposes a public
literal surface without exposing private fields, allocation strategy, or helper
methods.

Examples:

```nct
let nums = Vec [1, 2, 3]
let empty = Vec<i32> []

let names = Set ["Rvo", "Nocter"]

let ages = Map {
    "Rvo": 20,
    "Nocter": 1,
}
```

The type name is mandatory. Bare `[1, 2, 3]` remains the built-in fixed-size
array literal unless another future chapter changes that rule explicitly.

## Non-v0 Boundary

The following source forms are not part of v0:

```nct
literal Vec<T> [...items: [T]]: Self {
    ...
}

Vec [1, 2, 3]
Vec [
    ...other,
    4,
]

func print_all(...values: [String]): void {
    ...
}
```

No v0 compiler or editor integration should implement special behavior for
these forms. Until this feature is promoted, ordinary named constructors and
methods remain the stable construction API.

## Literal Definitions

Adopted future syntax direction:

```nct
literal Vec<T> [...items: [T]]: Self {
    let result = Self.with_capacity(items.len())

    for item in items {
        result.push(item)
    }
    return result
}
```

A literal definition is a constructor-like declaration attached to one nominal
type.

Rules:

- A literal definition target must be a nominal type.
- A literal definition must be declared in the same module as the target type.
- A literal definition body returns `Self`.
- `Self` means the target type after substituting generic parameters.
- A literal definition is private by default. `pub literal` exposes it anywhere
  the target type is visible.
- Literal construction never bypasses the literal definition body.
- A literal definition must not expose or require access to the target type's
  private fields outside the defining module.

Same-module attachment prevents orphan literal definitions. A user cannot add a
literal surface to someone else's type from another module.

## No Overload

Nocter must not allow literal overload.

Rules:

- A nominal type may have at most one literal definition.
- The delimiter, parameter count, parameter labels, generic constraints, and
  return type do not create overload sets.
- A module must not import two visible literal definitions for the same target
  type.
- If a type needs multiple construction modes, it should expose named
  associated functions or methods instead.

This preserves Nocter's foolproof design. A typed literal expression should
have one possible meaning after the target type is known.

## Literal Shapes

User-defined literal definitions may use only literal shapes that already
belong to the language.

Adopted future shape set:

- sequence shape: `Type [elements...]`
- mapping or named shape: `Type { entries... }`
- tuple-like shape: `Type (elements...)`
- existing string literal shape: `Type "text"` or `Type """text"""`
- existing numeric literal shape: `Type 123`
- existing byte literal shape: `Type b'x'`

Not adopted:

- custom delimiters
- custom operator tokens
- bare sigils before or after the literal
- reader-macro style syntax
- implicit conversion from an untyped bare literal to an arbitrary nominal type

The language should not let each type invent a new mini-language. The literal
definition chooses behavior for an existing source shape only.

## Sequence Literals

Sequence-shaped typed literals are for ordered element collections.

```nct
Vec [1, 2, 3]
Set ["a", "b"]
Queue [job1, job2]
```

A sequence literal evaluates element expressions from left to right. Each
element is passed to the literal definition exactly once, preserving normal
move, borrow, and failure behavior.

The canonical sequence capture parameter is:

```nct
literal Vec<T> [...items: [T]]: Self
```

`...items: [T]` binds a temporary element sequence of `T`. It does not mean a
first-class `[T]` value is allocated before entering the literal body. The
spelling deliberately mirrors slice syntax because the author consumes a
sequence of `T`, but this binding is a special literal-capture form.

A non-empty collection can require leading elements before the rest capture:

```nct
literal NonEmptyVec<T> [first: T, ...rest: [T]]: Self
```

Rules:

- A sequence literal definition may contain at most one rest capture.
- The rest capture must be the final parameter.
- Required leading parameters are matched positionally.
- The compiler must reject a typed sequence literal that provides fewer
  elements than the required leading parameters.

## Mapping And Named Literals

Mapping-shaped typed literals are for key-value collections or named
construction surfaces.

```nct
let ages = Map {
    "Rvo": 20,
    "Nocter": 1,
}
```

For homogeneous map types, keys and values must typecheck against the literal
definition's key and value expectations. A heterogeneous object-like value
should use an explicit sum type or a domain-specific nominal type instead of
pretending to be a homogeneous map.

Named construction should prefer `{}` over positional `[]` when the field names
carry meaning:

```nct
struct Color {
    r: u8
    g: u8
    b: u8
}

literal Color { r: u8, g: u8, b: u8 }: Self {
    return Self {
        r: r,
        g: g,
        b: b,
    }
}

let red = Color { r: 255, g: 0, b: 0 }
```

This is more foolproof than `Color [255, 0, 0]` because it keeps the source
meaning visible at the call site.

## Existing String, Numeric, And Byte Literals

Existing literal token forms may be used only through a typed literal
expression.

Examples:

```nct
let path = Path "README.md"
let port = Port 8080
let marker = Marker b'\n'
```

Rules:

- A string typed literal receives the decoded string literal value.
- A numeric typed literal receives a normal integer literal subject to Nocter's
  existing contextual integer rules.
- A byte typed literal receives a normal `u8` byte literal.
- These forms do not create implicit conversions from `&str`, integer, or `u8`
  values.
- The bare literal keeps its normal v0 meaning.

## The `...` Operator Family

Adopted future direction: `...` is Nocter's contextual many-value operator.

The common meaning is:

> Take multiple values from one boundary and present them as a single ordered
> group, or take one grouped source and spread it into multiple values.

The exact rule depends on the syntactic context.

Planned contexts:

```nct
struct C {
    ...A
}
```

Embedding declaration. The owner stores an unnamed `A` value and promotes the
allowed public surface according to [Embedding](08-generics-interfaces-embedding-methods.md#embedding).

```nct
Vec [
    ...other,
    4,
    5,
]
```

Sequence spread. The elements of `other` are inserted at that source position.

```nct
Profile {
    ...User.new(move name, age),
    ...Article.new(move title, move text),
    visits: 0,
}
```

Embedded initializer or future aggregate composition spread. The expression
provides multiple initialized members at that source position.

```nct
func print_all(...values: [String]): void

method &+self.push_all(...items: [T]): void
```

Variadic capture. The callee receives a temporary element sequence without
requiring callers to allocate an intermediate collection.

```nct
literal Vec<T> [...items: [T]]: Self
```

Literal rest capture. The literal body receives the source elements as an
ephemeral sequence.

## Allocation And Lowering

`...items: [T]` and `...values: [T]` are not promises that the compiler has
allocated an owned array or slice value. They describe a temporary element
sequence available inside a literal body or variadic callee.

Lowering should preserve source order and ownership while avoiding unnecessary
allocation:

- A literal body may be lowered as a loop over source elements.
- A variadic call may pass a compile-time element list, a stack temporary, or a
  lowered iterator-like representation depending on ABI and escape rules.
- If the callee stores the elements in an owned collection, that collection's
  ordinary allocation API performs the allocation.
- If the sequence does not escape, the compiler should not materialize heap
  storage only to satisfy the surface syntax.

The surface may look like values are collected and then expanded again. The
implementation should instead treat the sequence as compiler-owned temporary
structure unless ordinary Nocter code explicitly constructs an owned collection.

## Ownership And Evaluation

Future implementation must preserve Nocter's existing move and borrow model.

Rules to finalize before implementation:

- Elements are evaluated left to right.
- A moved element is unavailable after it is consumed by the typed literal,
  spread, or variadic call.
- A fallible element expression propagates failure according to ordinary `T!`
  rules.
- Already initialized elements or embedded values are cleaned up in reverse
  initialization order if a later element fails.
- A spread source must define whether it is copied, borrowed, moved, or drained.

The design must reject ambiguous spread sources instead of guessing. For
example, spreading a move-only collection should require a source API that makes
copying, borrowing, or draining explicit.

## Interaction With Embedding

Embedding remains distinct from interface contracts. `...` does not change that
rule.

The shared token is intentional: embedding, spread, rest capture, and variadic
capture are all many-value boundary forms. They must still be specified as
separate contexts so diagnostics, ownership, and lowering stay simple.

Embedding may be promoted before the broader literal and variadic design only
if its `...` forms stay limited to struct declarations and struct literals.

## Non-Goals

Literal definitions and `...` spread are not:

- operator overloading
- implicit conversion
- custom syntax per type
- pattern matching syntax
- macro expansion
- textual include
- trait-style extension of foreign types
- a way to expose private fields
- a promise of hidden heap allocation

Named constructors remain the right API when a construction mode needs a name,
multiple options, validation policy, allocation source, or domain-specific
error behavior that is not obvious from the literal shape.
