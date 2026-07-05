# Generics, Traits, and Methods

This file is part of the Nocter language specification.
The specification entry point is [../SPEC.md](../SPEC.md).

## Impl Blocks

Adopted: `impl` associates functions and methods with a type. It is not a class declaration and does not introduce inheritance.

`func` inside an `impl` defines an associated function. It has no receiver and is called through the type.

```nct
impl WordStats {
    pub func empty(): WordStats {
        return WordStats{
            bytes: 0,
            lines: 0,
            words: 0,
        }
    }
}
```

```nct
let stats = WordStats.empty()
```

`method` inside an `impl` defines a receiver method. The receiver is explicit and appears before the method name.

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

`Self` is a contextual type name inside an `impl` block. In `impl WordStats`, `Self` means `WordStats`.

Nocter does not reserve `self` or `this`. The receiver name is chosen by the author. `self` may be used as an ordinary receiver name, but it has no special meaning.

The target of an `impl` block must be a nominal type declaration, such as a `struct` or `enum`. An `impl` block cannot target a type alias because aliases do not create distinct types.

```nct
type Int = i32

impl Int {
    ...
}
// error: Int is a type alias, not a nominal type
```

Initial receiver forms:

```nct
method (value: &Self).name(...): Return
method (value: &+Self).name(...): Return
method (value: Self).name(...): Return
```

Meaning:

- `&Self` is a readonly receiver.
- `&+Self` is a readwrite receiver.
- `Self` is a consuming receiver. It requires copy or explicit move according to the normal ownership rules.
- Calling a `&Self` method borrows the receiver readonly.
- Calling a `&+Self` method borrows the receiver readwrite and requires a writable receiver place.
- A newly created owned temporary may be used as a `&+Self` receiver for that single method call because it has no existing aliases.
- Calling a `Self` method consumes or copies the receiver according to the receiver type.
- Borrow-like values derived from a temporary receiver cannot escape the current statement.

Call rules:

- `Type.function(args)` calls an associated `func`.
- `value.method(args)` calls a `method`.
- Associated function and method arguments follow the positional argument rules in [Control Flow](03-control-flow.md#function-calls-and-arguments).
- `Type.method(&value, args)` and `Type.method(&+value, args)` are invalid in the initial design.
- `value.function(args)` is invalid when `function` is only an associated `func`.
- `func` and `method` share the same member namespace for a type. Defining both with the same name for the same type is an error in the initial design.
- Enum variants also occupy the type member namespace. An associated `func` or `method` cannot reuse an enum variant member name in v0.
- If method lookup finds multiple valid candidates, the call is ambiguous and is a compile error.
- The initial design has no qualified method-call escape hatch for ambiguity resolution.

```nct
try file.write_text("hello")          // OK: method call
try File.write_text(&+file, "hello")  // error: methods are not UFCS functions
```

## Method Lookup

Adopted: method lookup is deliberately small and deterministic in v0.

For `value.method(args)`, the compiler first determines the static type of `value`.

If the receiver has a concrete nominal type, the compiler looks only for inherent methods declared in `impl Type` blocks for that nominal type. Trait methods are not extension methods on concrete values in v0. If a concrete type should support `file.write(...)`, define an inherent `method` on `File`.

If the receiver is a generic type parameter, the compiler looks at the receiver type parameter's explicit trait bound. A method declared by that bound may be called through the generic value.

```nct
func write_line<W: Writer>(writer: &+W, text: StringView): void ! IOError {
    try writer.write(text)
    try writer.write("\n")
    return
}
```

Lookup order:

1. inherent method on a concrete nominal receiver type
2. trait-bound method on a generic type parameter receiver
3. no candidate, producing a compile error

In v0, the compiler does not search all visible trait implementations to resolve `value.method(args)`. This avoids trait-import-dependent behavior and keeps method calls readable from the receiver type and the generic bounds in the current declaration.

Ambiguity is a compile error. The initial language has no syntax such as `Trait.method(value, args)` or `<Type as Trait>.method(value, args)` to force one candidate.

Initial implementation order:

1. `impl Type { ... }`
2. `Self` inside `impl`
3. associated function calls such as `Type.function(...)`
4. method declarations
5. method calls such as `value.method(...)`

## Traits

Adopted: traits describe required behavior without class inheritance.

```nct
trait Writer {
    method (out: &+Self).write(text: StringView): void ! IOError
}
```

`Self` is also available inside a trait declaration and means the implementing type.

Trait implementation uses `impl Trait for Type`.

```nct
impl Writer for File {
    method (file: &+Self).write(text: StringView): void ! IOError {
        ...
    }
}
```

The `impl Trait for Type` block may contain only the members required by the trait. Extra associated functions or methods belong in an inherent `impl Type` block.

Each required trait method must be implemented exactly once, and its signature must match the trait declaration after substituting `Self` with the implementing type.

Generic functions may use trait bounds.

```nct
func print_line<W: Writer>(writer: &+W, text: StringView): void ! IOError {
    try writer.write(text)
    try writer.write("\n")
    return
}
```

### Trait Implementation Coherence

Adopted: a trait implementation is allowed only where either the trait or the implementing nominal type is defined.

Rules:

- A module may implement a trait for a type if that module defines the trait.
- A module may implement a trait for a type if that module defines the implementing nominal type.
- A module may not implement an external trait for an external type.
- A type alias does not count as defining a new nominal type.
- Implementing a trait for a borrow, pointer, fixed-size array, function type, or other non-nominal type is not part of v0.
- There must be at most one implementation for the same resolved trait and implementing nominal type in the whole loaded program.
- Blanket implementations such as `impl<T: Writer> Debug for T` are not part of v0.

```nct
// Each block below is a separate module situation, not a set of simultaneous declarations.

// In the module that defines File: OK.
impl Writer for File {
    method (file: &+Self).write(text: StringView): void ! IOError {
        ...
    }
}

// In the module that defines Writer: OK, even if File is external.
impl Writer for File {
    method (file: &+Self).write(text: StringView): void ! IOError {
        ...
    }
}

// In a third module that defines neither Writer nor File: error.
impl Writer for File {
    ...
}
```

## Generics

Adopted: generic type parameters use angle brackets.

```nct
struct Buffer<T> {
    ...
}

func first<T>(items: View<T>): T? {
    ...
}
```

Trait bounds are written inline with `:`.

```nct
func print<T: Format>(value: T): void
```

Multiple bounds such as `T: Hash + Equal` are not part of v0.

Generic parameter grammar in v0:

```text
GenericParameters = "<" GenericParameter ("," GenericParameter)* ">"
GenericParameter  = Name (":" TraitName)?
```

`TraitName` is a resolved trait name imported into the current file. `where` clauses, default type parameters, negative bounds, and bound expressions are not part of v0.

Generic implementation uses monomorphization. Each concrete instantiation is compiled as concrete code.

```nct
Buffer<i32>
Buffer<String>
```

This keeps generic dispatch static, avoids runtime type metadata for basic generics, and fits the no-runtime direction.

Initial generic scope:

- type parameters on structs
- type parameters on functions
- type parameters on impl blocks where needed
- inline trait bounds in the form `T: Trait`
- compile-time monomorphization

Deferred generic features:

- multiple bounds such as `T: A + B`
- full `where` clauses
- higher-kinded types
- generic associated types
- const generics beyond the minimum needed for fixed-size arrays
- dynamic dispatch through `dyn Trait`

## Trait Scope

Initial trait scope:

- trait declarations
- `impl Trait for Type`
- generic bounds in the form `T: Trait`
- method declarations in traits
- method calls through trait bounds
- ambiguity is a compile error

Deferred trait features:

- trait objects such as `dyn Trait`
- trait inheritance
- associated types
- default methods
- blanket impls
- specialization
- full `where` clauses

Class inheritance is not part of the core language direction.
