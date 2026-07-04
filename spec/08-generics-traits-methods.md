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
- Calling a `Self` method consumes or copies the receiver according to the receiver type.

Call rules:

- `Type.function(args)` calls an associated `func`.
- `value.method(args)` calls a `method`.
- `Type.method(&value, args)` and `Type.method(&+value, args)` are invalid in the initial design.
- `value.function(args)` is invalid when `function` is only an associated `func`.
- `func` and `method` share the same member namespace for a type. Defining both with the same name for the same type is an error in the initial design.
- If method lookup finds multiple valid candidates, the call is ambiguous and is a compile error.
- The initial design has no qualified method-call escape hatch for ambiguity resolution.

```nct
try file.write("hello")          // OK: method call
try File.write(&+file, "hello")  // error: methods are not UFCS functions
```

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
    method (out: &+Self).write(text: StringView): void!IOError
}
```

`Self` is also available inside a trait declaration and means the implementing type.

Trait implementation uses `impl Trait for Type`.

```nct
impl Writer for File {
    method (file: &+Self).write(text: StringView): void!IOError {
        ...
    }
}
```

Generic functions may use trait bounds.

```nct
func print_line<W: Writer>(writer: &+W, text: StringView): void!IOError {
    try writer.write(text)
    try writer.write("\n")
    return
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

Multiple bounds use `+`.

```nct
func hash_key<K: Hash + Equal>(key: &K): u64
```

Generic implementation uses monomorphization. Each concrete instantiation is compiled as concrete code.

```nct
Buffer<Int>
Buffer<String>
```

This keeps generic dispatch static, avoids runtime type metadata for basic generics, and fits the no-runtime direction.

Initial generic scope:

- type parameters on structs
- type parameters on functions
- type parameters on impl blocks where needed
- inline trait bounds in the form `T: Trait`
- multiple bounds in the form `T: A + B`
- compile-time monomorphization

Deferred generic features:

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
