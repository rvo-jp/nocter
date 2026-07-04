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

## Function Parameters

Adopted: parameters are immutable bindings inside the function body.

```nct
func create<W: Writer>(name: String, count: i32, out: &+W): User!IOError {
    try out.write(name.view())

    return User{
        name: move name,
        count: count,
    }
}
```

Rules:

- Parameters are immutable bindings.
- Parameter bindings cannot be reassigned.
- `var` parameters are not part of v0.
- Parameter names must be unique within the parameter list.
- Parameter shadowing is not allowed, following the normal name-resolution rules.
- An owned parameter is owned by the function body.
- A move-only owned parameter is dropped at function scope end unless it is moved.
- Moving a move-only parameter requires `move parameter`.
- After a move-only parameter is moved, that parameter binding is no longer valid.
- A copy parameter may be copied by ordinary use.
- `&T` parameters are readonly borrow bindings.
- `&+T` parameters are readwrite borrow bindings.
- A borrow parameter does not own the referenced value and does not drop it at function scope end.
- The `&+T` parameter binding itself cannot be reassigned, but the referenced value may be mutated through it.
- Method receivers are explicit parameters and follow the same binding, ownership, and borrow rules.
- Default parameters and named parameters are not part of v0.

Examples:

```nct
func rename(user: &+User, name: String): void {
    user.name = move name
}
```

```nct
func invalid_reassign(name: String): void {
    name = String.empty() // error: parameters are immutable bindings
}
```

```nct
func normalize(value: i32): i32 {
    var current = value

    if current < 0 {
        current = -current
    }

    return current
}
```

```nct
func increment(value: &+Counter): void {
    value.count += 1 // OK: mutates the referenced Counter
}

func invalid_rebind(value: &+Counter, other: &+Counter): void {
    value = other // error: parameter binding is immutable
}
```

## Drop

Adopted: resource destruction uses a dedicated `drop` member inside `impl`, not a `Drop` trait.

```nct
import std/os as os

impl File {
    drop(file: &+Self) {
        os.close(file.fd).ignore()
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
    pub x: i32
    pub y: i32
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
- Type aliases to copy types are copyable. For example, a standard-library `Int` alias to `i32` is copyable.
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

## Return Values

Adopted: returning an existing move-only binding requires explicit `move`.

Rules:

- `return value` may return a copy value by copying it.
- `return value` is invalid when `value` is an existing move-only binding.
- `return move value` returns an existing move-only binding by moving it.
- After `return move value`, that binding is no longer valid on any remaining reachable path.
- A newly constructed owned value may be returned with `return expr` without `move`.
- Newly constructed owned values include struct literals, enum variant constructors, array literals, and function or method call results.
- `return` evaluates the returned expression first.
- When control leaves through `return`, the returned value is not dropped by the callee.
- Other live local owned values are dropped in reverse declaration order.
- Moved bindings are not dropped.
- Copy parameters may be returned with `return parameter`.
- Move-only owned parameters require `return move parameter`.
- `return none` is valid only for optional return type `T?`.
- Bare `return` is valid only for `void` return type.

Examples:

```nct
func make_text(): String {
    let text = String.new()
    return move text
}
```

```nct
func make_user(name: String): User {
    return User{
        name: move name,
    }
}
```

```nct
func take_user(user: User): User {
    return move user
}
```

```nct
func invalid(user: User): User {
    return user // error: User is move-only
}
```

### Borrow-like Return Values

Adopted: v0 allows borrow-like return values only when the compiler can prove the referenced storage lives after the function returns.

Borrow-like return values include:

- `&T`
- `&+T`
- `StringView`
- `View<T>`
- `WriteView<T>`
- structs, enums, optionals, fallible values, and arrays containing borrow-like values

Rules:

- Borrow-like return values must carry provenance to storage that outlives the function call.
- Borrow-like values derived from static storage, such as string literals, may be returned.
- Borrow-like values derived from input borrow parameters may be returned when the return value's provenance is still tied to that input borrow.
- A readonly borrow-like value may be returned from an input `&T` or `&+T` source.
- A readwrite borrow-like value may be returned only from an input `&+T` source.
- Borrow-like values derived from local owned values cannot be returned.
- Borrow-like values derived from temporary owned values cannot be returned.
- Borrow-like values derived from owned parameters cannot be returned, because owned parameters are dropped at function scope end unless moved.
- Borrow-like values derived from region-allocated storage cannot escape the region.
- v0 has no source-level lifetime parameters or lifetime annotations.
- If provenance cannot be proven by the compiler, returning the borrow-like value is a compile error.

Examples:

```nct
func greeting(): StringView {
    return "hello" // OK: string literal storage is static
}
```

```nct
func first_byte(bytes: View<u8>): u8? {
    if bytes.len() == 0 {
        return none
    }

    return bytes[0] // OK: u8 is copy
}
```

```nct
func bad(allocator: &+Allocator): StringView!AllocError {
    var text = try String.copy(allocator, "hello")
    return text.view() // error: view points to local owned value
}
```

```nct
func also_bad(text: String): StringView {
    return text.view() // error: view points to an owned parameter dropped at return
}
```
