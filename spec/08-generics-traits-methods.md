# Generics and Methods

This file is part of the Nocter language specification.
The specification entry point is [README.md](README.md).

## v0 Scope

Nocter v0 includes generic type parameters, associated functions, inherent
`impl` blocks, receiver methods, and `Self` type syntax inside inherent member
contexts.

Nocter v0 does not include traits.

Deferred after v0:

- `trait` declarations
- `impl Trait for Type`
- generic bounds such as `T: Trait`
- trait method lookup
- trait objects such as `dyn Trait`
- trait inheritance, associated types, default methods, blanket impls,
  specialization, and `where` clauses

`trait` is not a reserved keyword in v0. It is lexed as an identifier. A source
form that starts a top-level item with `trait` is diagnosed as a deferred
feature, but the spelling remains available as an ordinary identifier in
positions such as a function name.

## Impl Blocks

Adopted: `impl` associates receiver methods and destructor members with a
nominal type. It is not a class declaration and does not introduce inheritance.

Associated functions are declared at top level with a qualified function name.
They have no receiver and are called through the type.

```nct
pub func WordStats.empty(): WordStats {
    return WordStats {
        bytes: 0,
        lines: 0,
        words: 0,
    }
}
```

```nct
let stats = WordStats.empty()
```

`method` inside an `impl` defines a receiver method. The receiver is explicit
and appears before the method name.

```nct
impl WordStats {
    pub method (stats: &+Self).add_byte(byte: u8): void {
        stats.bytes += 1
    }

    pub method (stats: &+Self).add_word(): void {
        stats.words += 1
    }
}
```

```nct
stats.add_word()
```

`Self` is type-position syntax inside an inherent `impl` block and inside a
qualified associated function declaration such as `func WordStats.empty`.
It is not an ordinary identifier and is not resolved through normal name lookup.
In `impl WordStats` or `func WordStats.empty`, `Self` means `WordStats`.

Nocter does not reserve `self` or `this`. The receiver name is chosen by the
author. `self` may be used as an ordinary receiver name, but it has no special
meaning. The restrictions on `Self` are specified in
[Values and Types](02-values-types.md#self-type-syntax).

The target of an `impl` block must be a nominal type declaration, such as a
`struct` or `enum`. An `impl` block cannot target a type alias because aliases
do not create distinct types.

```nct
type Int = i32

impl Int {
    ...
}
// error: Int is a type alias, not a nominal type
```

`impl Trait for Type` is not part of v0. The parser must diagnose this form as a
deferred feature. If a concrete type should support a receiver call, define an
inherent method on that concrete type.

Initial receiver forms:

```nct
method (value: &Self).name(...): Return
method (value: &+Self).name(...): Return
method (value: Self).name(...): Return
```

Meaning:

- `&Self` is a readonly receiver.
- `&+Self` is a readwrite receiver.
- `Self` is a consuming receiver. It requires copy or explicit move according to
  the normal ownership rules.
- Calling a `&Self` method borrows the receiver readonly.
- Calling a `&+Self` method borrows the receiver readwrite and requires a
  writable receiver place.
- A newly created owned temporary may be used as a `&+Self` receiver for that
  single method call because it has no existing aliases.
- Calling a `Self` method consumes or copies the receiver according to the
  receiver type.
- Borrow-like values derived from a temporary receiver cannot escape the current
  statement.

Call rules:

- `Type.function(args)` calls an associated `func`.
- `value.method(args)` calls a `method`.
- Associated function and method arguments follow the positional argument rules
  in [Control Flow](03-control-flow.md#function-calls-and-arguments).
- `Type.method(&value, args)` and `Type.method(&+value, args)` are invalid in
  v0.
- `value.function(args)` is invalid when `function` is only an associated
  `func`.
- `func Type.name` and `method` share the same member namespace for a type.
  Defining both with the same member name for the same type is an error in v0.
- Enum variants also occupy the type member namespace. An associated `func` or
  `method` cannot reuse an enum variant member name in v0.
- If method lookup finds multiple valid inherent candidates, the call is
  ambiguous and is a compile error.
- v0 has no qualified method-call escape hatch for ambiguity resolution.

```nct
file.write_text("hello")?          // OK: method call
File.write_text(&+file, "hello")?  // error: methods are not UFCS functions
```

## Method Lookup

Adopted: method lookup is deliberately small and deterministic in v0.

For `value.method(args)`, the compiler first determines the static type of
`value`.

If the receiver has a concrete nominal type, the compiler looks only for
inherent methods declared in `impl Type` blocks for that nominal type.

If the receiver is a generic type parameter, v0 has no trait-bound method lookup.
A method call through an unconstrained generic receiver is invalid unless a
future feature supplies a bound and lookup rule.

Lookup order:

1. inherent method on a concrete nominal receiver type
2. no candidate, producing a compile error

The compiler does not search visible trait implementations to resolve
`value.method(args)` in v0. This avoids import-dependent method lookup and keeps
calls readable from the receiver type.

Initial implementation order:

1. `impl Type { ... }` receiver methods
2. `Self` inside `impl` and `func Type.name`
3. associated function declarations such as `func Type.function(...)`
4. associated function calls such as `Type.function(...)`
5. method declarations
6. method calls such as `value.method(...)`

## Generics

Adopted: generic type parameters use angle brackets.

```nct
struct Buffer<T> {
    ...
}

func first<T>(items: &[T]): T? {
    ...
}
```

Generic parameter grammar in v0:

```text
GenericParameters = "<" GenericParameter ("," GenericParameter)* ">"
GenericParameter  = Name
```

Generic bounds are deferred after v0. The parser must diagnose a generic
parameter colon such as `T: Format` as a deferred feature.

Generic implementation uses monomorphization. Each concrete instantiation is
compiled as concrete code.

```nct
Buffer<i32>
Buffer<String>
```

This keeps generic dispatch static, avoids runtime type metadata for basic
generics, and fits the no-runtime direction.

Initial generic scope:

- type parameters on structs
- type parameters on functions
- type parameters on `impl` blocks where needed
- compile-time monomorphization

Deferred generic features:

- inline bounds such as `T: Trait`
- multiple bounds such as `T: A + B`
- full `where` clauses
- higher-kinded types
- generic associated types
- const generics beyond the minimum needed for fixed-size arrays

## Future Trait Direction

Traits remain a possible post-v0 feature, but they are not part of the v0
contract. A future trait design must specify at least:

- declaration syntax
- implementation syntax
- coherence rules
- method lookup interaction with inherent methods
- generic bound checking
- LSP hover, semantic token, completion, and diagnostic facts
- backend dispatch model, including whether dispatch is static only or includes
  explicit dynamic dispatch

Class inheritance is not part of the core language direction.
