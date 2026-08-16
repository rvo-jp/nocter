# Construction Surfaces

This file is part of the Nocter language specification.
The specification entry point is [README.md](README.md).

This chapter defines the construction-surface model. A construction surface
groups the public operations that directly create one nominal type so source readers and editor
tooling do not have to discover field literals, typed literals, and construction functions
independently.

## Construct Declarations

A `construct` declaration belongs to one nominal type in the same module:

```nct
construct Vec<T> {
    pub default literal [](...items: T): Self {
        ...
    }

    pub func new(): Self {
        ...
    }

    pub func with_capacity(capacity: usize): Self {
        ...
    }
}
```

The target type is written once. A literal member therefore starts with its shape, and a function
member uses its unqualified member name. At call sites the established syntax remains unchanged:

```nct
let values = Vec [1, 2, 3]
let empty: Vec<i32> = Vec.new()
let reserved = Vec<i32>.with_capacity(16)
```

Rules:

- In an ordinary package, the target must be a nominal struct or enum declared in the same module.
- The exact active standard-library package may additionally declare compiler-authorized
  construction surfaces for built-in types such as integer types. This authority follows package
  identity, not path spelling or visibility.
- The target arguments must bind every generic parameter in declaration order.
- A nominal type may have at most one `construct` declaration.
- Every construction member must carry an explicit non-private visibility: `pub(./)`, an ancestor
  scope, `pub(/)`, or bare `pub`. Private construction members are invalid.
- A construction function must produce `Self` as its direct result or as the success/present payload
  of a supported outcome type.
- A literal member follows the literal-shape, ownership, allocation-context, and no-overload rules
  from [Literal Definitions and Sequence Spread](17-literal-definitions-sequence-spread.md).
- `Self` denotes the specialized construct target throughout member signatures and bodies.
- `construct` declarations cannot be imported separately. Their accessible members travel with the
  target type.

Functions that do not directly produce the target are ordinary module functions and cannot use a
qualified type owner. Receiver methods remain in `instance`; destruction uses an independent
`destruct Type(&+self)` declaration; interface conformance members remain in
`conform Interface for Type`.

## Generic Owner Arguments

Every generic construction entry uses one owner-argument rule. This includes construction
functions, typed literals, named-field struct literals, and enum variants.

The caller may omit all owner type arguments when one unique substitution follows from construction
arguments, field initializers, literal elements, or the expected type of the complete construction
expression:

```nct
let empty: Vec<i32> = Vec.new()
let one = Vec.from_value(1)
let boxed = Box { value: 1 }
let present = Maybe.some(1)
```

When those inputs do not determine every owner parameter, the caller writes the complete owner type:

```nct
let reserved = Vec<i32>.with_capacity(16)
let empty = Vec<i32> []
let boxed = Box<i32> { value: 1 }
```

Rules:

- Explicit owner arguments precede the construction member name or typed-literal delimiter.
- Explicit owner arguments supply every owner parameter in declaration order. Partial lists and `_`
  placeholders are invalid.
- Omitted owner arguments are inferred only from ordinary parameter/argument matching, field and
  element matching, and the expected result type.
- A generic requirement validates an inferred or explicit substitution but never chooses one.
- Return provenance, allocation context, declaration order, default-entry status, and body contents
  do not infer owner arguments.
- If a parameter remains unknown or multiple substitutions remain viable, construction is an error.
- Nocter does not define default generic arguments.

These rules concern the generic parameters of the constructed owner. In
`Vec<i32>.from_iter(source)`, `i32` specializes the `Vec` owner. Any generic parameters declared by
`from_iter` itself are inferred and cannot be written at the call site, as specified by
[Callable Type-Argument Inference](08-generics-interfaces-embedding-methods.md#callable-type-argument-inference).

## Default Construction Entry

At most one member may carry the contextual `default` modifier:

```nct
construct Vec<T> {
    pub literal [](...items: T): Self {
        ...
    }

    pub default func new(): Self {
        ...
    }
}
```

`default` identifies the primary construction entry for documentation, completion ordering, and
type hover. It does not introduce an implicit conversion, change a member's call syntax, or rewrite
`Type { ... }` into a function or typed-literal call. `default` is contextual inside a `construct`
member and remains available as an identifier elsewhere.

Without an explicit default member, an externally accessible named-field struct literal is the
implicit default. Its availability continues to follow field visibility. If that structural form
is not externally accessible and the construct declaration exposes members, one member must be
marked `default`.

With an explicit default member, named-field `Type { ... }` construction is restricted to the
target's defining module, even when every field is public. This restriction hides raw
initialization, not field access after a value exists.

An empty construct declaration explicitly states that the type has no direct public construction
entry:

```nct
construct RuntimeToken {}
```

## Construction Surface

The effective construction surface of a nominal type contains:

- its structural named-field entry when externally accessible
- typed-literal members
- associated construction functions
- enum variants
- the optional default entry

The compiler owns this surface as resolved type information. Diagnostics, hover, completion,
signature help, and go-to-definition must query that information rather than scan source text or
reconstruct a list independently. Entries shown at a use site must respect type and member
visibility and must use the visible type spelling rather than an internal canonical module path.

Enum variants are intrinsic construction entries and are not duplicated inside `construct`.
Interfaces and type aliases do not own construction surfaces. Interfaces are not values. An alias
does not acquire a second construction API under its alias spelling; callers use the nominal target
or an ordinary module function when an alias-specific factory is needed.

## Legacy Declaration Forms

The earlier top-level forms place construction behavior outside its owner and have been removed:

```nct
literal Vec<T> [](...items: T): Self { ... }
pub func Vec.new<T>(): Vec<T> { ... }
```

The compiler diagnoses a top-level literal directly. Every qualified top-level function is also
invalid. When its result, present payload, or success payload is the named owner, the diagnostic
directs it into `construct Vec<T> { ... }`; otherwise it directs the declaration to an unqualified
module function or a receiver method. Factories for aliases of builtin representations are module
functions because aliases cannot own construction surfaces. The compiler does not maintain a
second compatibility AST or silently synthesize a construct declaration.
