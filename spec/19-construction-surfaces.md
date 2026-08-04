# Construction Surfaces

This file is part of the Nocter language specification.
The specification entry point is [README.md](README.md).

This chapter defines the adopted v0.3.0 construction-surface model. Its stabilization implementation
is in progress on `develop`; until that gate completes, the implemented top-level literal and
associated-function forms remain described in their owning chapters. A construction surface groups
the public operations that directly create one nominal type so source readers and editor tooling do
not have to discover field literals, typed literals, and associated functions independently.

## Construct Declarations

A `construct` declaration belongs to one nominal type in the same module:

```nct
construct Vec<T> {
    pub default literal [](...items: T): Self from current {
        ...
    }

    pub func new(): Self {
        ...
    }

    pub func with_capacity(capacity: usize): Self from current {
        ...
    }
}
```

The target type is written once. A literal member therefore starts with its shape, and a function
member uses its unqualified member name. At call sites the established syntax remains unchanged:

```nct
let values = Vec [1, 2, 3]
let empty = Vec.new()
let reserved = Vec.with_capacity(16)
```

Rules:

- The target must be a nominal struct or enum declared in the same module.
- The target arguments must bind every generic parameter in declaration order.
- A nominal type may have at most one `construct` declaration.
- Every construction member must carry explicit `pub` visibility. `pub(nocter)` and private
  construction members are invalid.
- A construction function must produce `Self` as its direct result or as the success/present payload
  of a supported outcome type.
- A literal member follows the literal-shape, ownership, allocation-context, and no-overload rules
  from [Literal Definitions and Sequence Spread](17-literal-definitions-sequence-spread.md).
- `Self` denotes the specialized construct target throughout member signatures and bodies.
- `construct` declarations cannot be imported separately. Their accessible members travel with the
  target type.

Public functions that do not directly produce the target remain ordinary functions. Receiver
methods and drop members remain in `impl`; interface implementation members remain in
`impl Interface for Type`.

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
Interfaces and type aliases do not own construction surfaces: interfaces are not values, and a type
alias preserves the construction surface of its resolved target.

## Legacy Declaration Forms

The earlier top-level forms place construction behavior outside its owner and are removed when this
model's stabilization gate completes:

```nct
literal Vec<T> [](...items: T): Self { ... }
pub func Vec.new<T>(): Vec<T> { ... }
```

The compiler diagnoses these forms with a direct migration to `construct Vec<T> { ... }`. It does
not maintain a second compatibility representation or silently synthesize a construct declaration.
