# Ownership, Borrowing, and Drop

This file is part of the Nocter language specification.
The specification entry point is [../SPEC.md](../SPEC.md).

## Borrowing

Borrows distinguish readonly access from readwrite access.

```nct
func inspect(file: &File): void {
    ...
}

func write(file: &+File, data: StringView): void!IOError {
    ...
}
```

Rules:

- `&T` is a readonly borrow type.
- `&+T` is a readwrite borrow type.
- `&value` creates a readonly borrow.
- `&+value` creates a readwrite borrow.
- `&+value` may be created only from a writable place, such as a `var` binding, a writable field, a writable index, or an existing `&+T` reborrow.
- Readonly borrows may coexist with other readonly borrows.
- A readwrite borrow is exclusive and cannot coexist with other readonly or readwrite borrows of the same value.
- A value cannot be moved while it is borrowed.
- A value cannot be explicitly dropped while it is borrowed.
- A borrow cannot outlive the value it refers to.
- A borrow of a stack value cannot escape the stack value's scope.
- A borrow of region-allocated memory cannot escape that region.
- Ordinary function calls require explicit borrow syntax at the call site.
- Method receivers may create the required borrow automatically.
- Lifetime annotations are not part of the initial design.
- `&+` is a single lexical token.
- Unary `+x` is not part of the language. This avoids ambiguity with `&+x`.

Examples:

```nct
var file = try File.open(path)

let a = &file
let b = &file       // OK: multiple readonly borrows
let c = &+file      // error: a and b are used below

inspect(a)
inspect(b)
```

```nct
var file = try File.open(path)

let w = &+file
drop file           // error: w is used below

write_more(w)
```

Ordinary function calls are explicit:

```nct
func inspect(file: &File): void {
    ...
}

inspect(&file)
```

Method receiver borrows are automatic:

```nct
impl File {
    pub method (file: &+Self).write(text: StringView): void!IOError {
        ...
    }
}

try file.write("hello")
```

The method call above creates a temporary readwrite borrow of `file` for the call. This does not enable UFCS-style calls:

```nct
File.write(&+file, "hello") // error
```

## Drop

Adopted: resource destruction uses a dedicated `drop` member inside `impl`, not a `Drop` trait.

```nct
impl File {
    drop(file: &+Self) {
        std.os.close(file.fd).ignore()
    }
}
```

`drop` is not a normal function name. It is a special member allowed only inside an `impl` block.

Rules:

- A type may define at most one `drop` member.
- `drop` has no return type annotation.
- `drop` always returns no value.
- `drop` cannot be fallible.
- `drop` cannot be marked `pub`.
- The first parameter must be `&+Self`.
- `drop` cannot be called as a normal associated function or method.
- `file.drop()` is invalid.
- `File.drop(&+file)` is invalid.
- Owned values are automatically dropped at scope end.
- Owned values are dropped in reverse declaration order.
- `return`, `fail`, and `try` propagation run the same scope-end drop behavior.
- A moved value is not dropped through the original binding.

Explicit early destruction uses a `drop` statement.

```nct
var file = try File.open(path)
drop file
```

After `drop file`, the binding is no longer valid.

```nct
file.read() // error
```

## Copy and Move

Adopted: types are move-only by default. Only copy types may be copied implicitly.

Copyable structs are declared with `copy struct`.

```nct
copy struct Point {
    pub x: Int
    pub y: Int
}
```

Rules:

- Types are move-only by default.
- `copy struct` types are implicitly copyable.
- Every field of a `copy struct` must be copyable.
- A `copy struct` cannot define `drop`.
- A `copy struct` must not own resources that require destruction.
- Primitive numeric types, `bool`, and raw pointers are copyable.
- Payloadless enum values are copyable.
- `Int` is copyable because it is an alias of `i32`.
- `&T` is copyable.
- `&+T` is not copyable.
- Non-copy values are not implicitly moved by assignment, argument passing, or return.
- Moving a non-copy value requires explicit `move`.

Examples:

```nct
let p1 = Point{x: 1, y: 2}
let p2 = p1 // OK: Point is copy

let text1 = String.new()
let text2 = text1      // error: String is not copy
let text3 = move text1 // OK
```

Function calls follow the same rule.

```nct
func consume(text: String): void {
    ...
}

let text = String.new()
consume(text)      // error
consume(move text) // OK
```

Returning non-copy owned values also uses explicit `move`.

```nct
func make_text(): String {
    let text = String.new()
    return move text
}
```
