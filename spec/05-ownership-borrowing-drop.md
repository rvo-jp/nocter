# Ownership, Borrowing, and Drop

This file is part of the Nocter language specification.
The specification entry point is [README.md](README.md).

## Borrowing

Borrows distinguish readonly access from readwrite access.

```nct
func inspect(file: &File): void {
    ...
}

func write(file: &+File, data: &str): void! {
    ...
}
```

Rules:

- `&T` is a readonly borrow type.
- `&+T` is a readwrite borrow type.
- `&value` creates a readonly borrow.
- `&+value` creates a readwrite borrow.
- `&+value` may be created only from a writable place, such as a `var` binding, a writable field, or an existing `&+T` reborrow.
- Readonly borrows may coexist with other readonly borrows.
- A readwrite borrow is exclusive and cannot coexist with other readonly or readwrite borrows of the same value.
- A value cannot be moved while it is borrowed.
- A value cannot be explicitly dropped while it is borrowed.
- A borrow cannot outlive the value it refers to.
- A borrow of a stack value cannot escape the stack value's scope.
- A borrow of region-allocated memory cannot escape that region.
- Ordinary function calls require explicit borrow syntax at the call site.
- Method receivers may create the required borrow automatically.
- Lifetime annotations are not supported.
- `&+` is a single lexical token.
- Unary `+x` is not part of the language. This avoids ambiguity with `&+x`.

Examples:

```nct
var file = File.open(path)?

let a = &file
let b = &file       // OK: multiple readonly borrows
let c = &+file      // error: a and b are used below

inspect(a)
inspect(b)
```

```nct
var file = File.open(path)?

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
instance File {
    pub method &+self.write_text(text: &str): void! {
        ...
    }
}

file.write_text("hello")?
```

The method call above creates a temporary readwrite borrow of `file` for the call. This does not enable UFCS-style calls:

```nct
File.write_text(&+file, "hello") // error
```

A newly created owned temporary may be used as a readwrite receiver for one method call:

```nct
(File.open(path)?).write_text("hello")?
```

The temporary receiver is dropped according to the statement-end temporary rules in [Control Flow](03-control-flow.md#evaluation-order-and-temporaries).

## Borrow Checker

Nocter uses source-level non-lexical borrow ranges.

A borrow is active from the expression that creates it through the last source-level use of the borrow-like value derived from it. The borrow may end before the lexical scope of the borrow binding ends if the binding is not used again.

```nct
let read = &file
inspect(read)

let write = &+file // OK: read is not used after inspect(read)
```

```nct
let read = &file
let write = &+file // error: read is used below

inspect(read)
```

Rules:

- Source-level lifetime annotations are not supported.
- The compiler determines borrow live ranges from actual source uses, not only from lexical scopes.
- A use includes passing the borrow-like value to a call, method call, field access, index access, return, assignment, initialization, storing it inside an aggregate, or deriving another borrow-like value from it.
- Scope end of a plain borrow-like value does not by itself extend the borrow, because borrow-like values are non-owning and do not run `drop`.
- A readonly borrow may overlap with other readonly borrows of the same place.
- A readwrite borrow must not overlap with any other readonly or readwrite borrow of the same place.
- `move name`, `drop name`, whole-place assignment, field assignment, and reinitialization are invalid when they conflict with an active borrow.
- A borrow cannot outlive the storage it refers to.
- A borrow cannot outlive a region from which its storage was allocated.
- A borrow or borrow-like value derived from a temporary owned value cannot escape the statement that owns the temporary.
- Method receiver borrows last only for the call unless the method returns a borrow-like value derived from the receiver.
- If a method returns a borrow-like value derived from the receiver, the receiver borrow remains active for the returned value's live range.
- `&str`, `&[T]`, `&+[T]`, `ViewIter<T>`, and aggregates containing borrow-like values participate in the same live-range and provenance checks.
- Borrow-like return values are governed by [Borrow-like Return Values](#borrow-like-return-values).

### Field-Sensitive Borrows

Nocter tracks disjoint named struct fields for simple places.

Field-sensitive tracking applies only to direct named field paths whose base is a local binding, parameter binding, or borrow binding that the compiler can resolve statically.

```nct
var user = User {
    name: String.copy("alice"),
    count: 0,
}

let name = &user.name
user.count += 1 // OK: count is disjoint from name
inspect(name)
```

Rules:

- A borrow of a whole value conflicts with borrows and mutations of any field of that value.
- A borrow of one named field does not conflict with mutation or borrowing of a disjoint named field.
- Moving, dropping, reinitializing, or assigning the whole parent value conflicts with any active field borrow.
- Assigning a field conflicts with active borrows of that same field and active borrows of the whole parent value.
- Field-sensitive tracking does not apply to array indexes, collection indexes, `&[T]` or `&+[T]` elements, pointer dereferences, method-call results, enum payloads, or computed projections.
- If the compiler cannot prove two places are disjoint, it treats them as conflicting.

## Function Parameters

Parameters are immutable bindings inside the function body.

```nct
func create(name: String, count: i32, out: &+File): User! {
    out.write_text(&name as &str)?

    return User {
        name: move name,
        count: count,
    }
}
```

Rules:

- Parameters are immutable bindings.
- Parameter bindings cannot be reassigned.
- Mutable parameter bindings are not supported.
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
- Default parameters and named parameters are not supported.

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

Resource destruction uses an independent `destruct` declaration, not an `instance` method or a
`Drop` interface.

```nct
destruct File(&+self) {
    close(self)
    return
}
```

`destruct` is a reserved declaration keyword. `drop` remains an identifier token that the parser
recognizes contextually only for the explicit `drop value` statement. Outside statement position,
`drop` is an ordinary identifier.

A declaration such as `func drop(...)` or `method &self.drop(...)` declares an ordinary function or method named `drop`. It does not define destruction behavior.

Rules:

- A nominal type family may define at most one `destruct` declaration.
- A destructor has the source form `destruct TypePattern(&+self) { ... }`.
- `self` is the fixed destructor receiver name and is scoped to the destructor body.
- The drop receiver type is always exactly `&+Self`.
- A destructor is a top-level declaration. `instance` contains inherent methods, and `conform`
  contains interface members.
- A destructor has no visibility, target, generic-prefix, `where`, or return-type annotation.
- Its target pattern must cover every generic slot exactly once with a distinct binder.
- A destructor always returns no value and cannot be fallible.
- A destructor cannot be called as a normal associated function or method.
- `file.drop()` is an ordinary method call if an ordinary method named `drop` exists; it is not a destructor call.
- `File.drop(&+file)` is an ordinary associated function call if an ordinary function named `drop` exists; it is not a destructor call.
- A destructor cannot report cleanup failure through a fallible return.
- If an operation inside `drop` can fail, the `drop` body must ignore that failure, record it in already-owned state before destruction, or terminate with `trap` / `abort`.
- Terminating with `trap` or `abort` from inside `drop` does not unwind remaining caller scopes.
- Owned values are automatically dropped at scope end.
- Initialized owned values are dropped in reverse declaration order.
- Struct drop glue invokes the struct's own destructor first when present,
  then drops owned fields in reverse declaration order. Field drop glue follows
  the same rule recursively.
- Maybe initialized owned values use compiler-generated conditional drop.
- Uninitialized bindings are not dropped.
- `return` and postfix `?` propagation run the same scope-end drop behavior, including conditional drop.
- A moved value is not dropped through the original binding.

### Explicit Drop Statements

Explicit early destruction uses a `drop` statement.

```nct
var file = File.open(path)?
drop file
```

After `drop file`, the binding enters an uninitialized state.

```nct
file.read() // error
```

Rules:

- `drop name` is a statement.
- The operand of `drop` must be a local binding name or parameter binding name.
- `drop` is not a reserved keyword and is not an ordinary function call in this statement form.
- The operand must be initialized.
- The operand must be a move-only owned binding.
- Copy types cannot be explicitly dropped.
- Borrow bindings such as `&T` and `&+T` cannot be explicitly dropped because they do not own the referenced value.
- Maybe initialized bindings cannot be explicitly dropped.
- Uninitialized bindings cannot be explicitly dropped.
- A binding cannot be explicitly dropped while it is borrowed.
- `drop name` runs the same drop glue as scope-end automatic drop.
- `drop` is not fallible.
- `drop` produces no value.
- After `drop name`, the binding is uninitialized on all later reachable paths.
- A dropped `var` binding may be reinitialized by assigning to the whole binding. The detailed rules are specified in [Values and Types](02-values-types.md#reinitialization-after-move-or-drop).
- A dropped `let` binding cannot be reinitialized.
- `drop object.field`, `drop array[index]`, and `drop make_value()` are invalid.

Examples:

```nct
var file = File.open(path)?
drop file

file = File.open(other)?
file.read()?
```

Invalid:

```nct
drop count
drop ref
drop object.field
drop array[index]
drop make_value()
```

## Copy and Move

Types are move-only by default. Only copy types may be copied implicitly.

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
- A generic `copy struct` is copyable per concrete instantiation. After
  substituting type arguments into the fields, every concrete field type must
  be copyable. For example, `copy struct Box<T> { value: T }` is copyable as
  `Box<i32>` but move-only as `Box<String>`, while a field such as `&T` remains
  copyable for any `T`.
- A `copy struct` cannot define `drop`.
- A `copy struct` must not own resources that require destruction.
- Primitive numeric types, `bool`, and raw pointers are copyable.
- The built-in `error` type is copyable.
- Payloadless enum values are copyable.
- Fixed-size arrays `[T; N]` are copyable when `T` is copyable.
- Type aliases to copy types are copyable. For example, a project-local alias to `i32` is copyable.
- `&T` is copyable.
- `&+T` is not copyable.
- A generic parameter is treated as potentially move-only unless its declaration or enclosing
  callable contract requires `copy`.
- `copy T` permits ordinary copy operations in the generic body and is checked again against every
  concrete substitution. It adds no runtime metadata and cannot be implemented by user code.
- Non-copy values are not implicitly moved by assignment, argument passing, or return.
- Moving a non-copy value requires explicit `move`.

Examples:

```nct
let p1 = Point { x: 1, y: 2 }
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

Static opaque results written as `some Interface` are move-only at the public boundary. A caller
cannot infer copyability from the hidden witness. Explicit `move` transfers an existing opaque
binding, and normal scope, return, optional, fallible, and path-sensitive cleanup destroys the
hidden value exactly once.

## Move Expressions

`move` is a unary expression that explicitly transfers ownership from a binding.

```nct
let b = move a
consume(move text)
return move value
```

Rules:

- `move` is a reserved keyword, not an ordinary function.
- `move name` is an expression.
- The operand of `move` must be a local binding name or parameter binding name.
- The operand binding may be immutable or mutable.
- The operand binding must have a move-only type.
- Using `move` on a copy type is a compile error.
- `move name` has the same type as `name`.
- Evaluating `move name` transfers ownership out of that binding.
- After `move name`, the binding enters an uninitialized state on all later reachable paths.
- A moved `var` binding may be reinitialized by assigning to the whole binding. The detailed rules are specified in [Values and Types](02-values-types.md#reinitialization-after-move-or-drop).
- A moved `let` binding cannot be reinitialized.
- A moved binding is not dropped through its original binding.
- A binding cannot be moved while it is borrowed.
- `move` describes ownership transfer. It does not specify whether generated code copies bytes, passes a pointer, or elides a copy.
- Moving a newly constructed value is unnecessary and invalid.
- Moving from a field, index, dereference, call result, postfix `?` expression, conditional expression, or parenthesized complex expression is invalid.
- Partial moves from struct fields and index moves from arrays or collections are not supported.

Valid:

```nct
let b = move a
consume(move text)
return move value
user.name = move name
```

Invalid:

```nct
move make_value()
move (make_value()?)
move (condition ? a : b)
move object.field
move array[index]
move copy_value
```

Assignment may replace a live move-only field when the right-hand side produces
a complete replacement value. The replacement is evaluated first, then the old
field is dropped, and ownership of the replacement transfers into the field.
Moving a value out of a field is invalid because it would require
tracking the field as separately dead. Code that needs to transfer a field
separately must transfer or rebuild the whole owner instead. To update a field
in a functional style, move the whole owner into a new binding, replace the
field, then return or pass the whole value.

```nct
func rename(user: User, name: String): User {
    var next = move user
    next.name = move name
    return move next
}
```

## Return Values

Returning an existing move-only binding requires explicit `move`.

Rules:

- `return value` may return a copy value by copying it.
- `return value` is invalid when `value` is an existing move-only binding.
- `return move value` returns an existing move-only binding by moving it. `value` must be a binding name.
- After `return move value`, that binding is no longer valid on any remaining reachable path.
- A newly constructed owned value may be returned with `return expr` without `move`.
- Newly constructed owned values include struct literals, enum variant constructors, array literals, and function or method call results.
- `return` evaluates the returned expression first.
- When control leaves through `return`, the returned value is not dropped by the callee.
- Other live local owned values are dropped in reverse declaration order.
- Moved bindings are not dropped.
- Copy parameters may be returned with `return parameter`.
- Move-only owned parameters require `return move parameter`.
- `return none` is valid for optional return type `T?` and for fallible optional return type `T?!`, where it returns success absence.
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
    return User {
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

Borrow-like return values are allowed only when the compiler can prove the referenced storage lives after the function returns.

Borrow-like return values include:

- `&T`
- `&+T`
- `&str`
- `&[T]`
- `&+[T]`
- `ViewIter<T>`
- structs, enums, optionals, fallible values, and arrays containing borrow-like values

The detailed provenance source kinds are specified in [Strings, Arrays, Views, and Pointers](07-strings-arrays-views-pointers.md#borrow-like-provenance).

Rules:

- Borrow-like return values must carry provenance to storage that outlives the function call.
- Borrow-like values derived from static storage, such as string literals, may be returned.
- Borrow-like values derived from input borrow-like parameters may be returned when the return value's provenance is still tied to that input borrow-like value.
- A readonly borrow-like value may be returned from a readonly or readwrite input borrow-like source.
- A readwrite borrow-like value may be returned only from a readwrite input borrow-like source, such as `&+T` or `&+[T]`.
- Borrow-like values derived from local owned values cannot be returned.
- Borrow-like values derived from temporary owned values cannot be returned.
- Borrow-like values derived from owned parameters cannot be returned, because owned parameters are dropped at function scope end unless moved.
- Borrow-like values derived from region-allocated storage cannot escape the region.
- Nocter has no source-level lifetime parameters or lifetime annotations.
- If provenance cannot be proven by the compiler, returning the borrow-like value is a compile error.

Examples:

```nct
func greeting(): &str {
    return "hello" // OK: string literal storage is static
}
```

```nct
func first_byte(bytes: &[u8]): u8? {
    if bytes.len() == 0 {
        return none
    }

    return bytes[0] // OK: u8 is copy
}
```

```nct
func bad(): &str {
    var text = String.copy("hello")
    return &text as &str // error: view points to local owned value
}
```

```nct
func also_bad(text: String): &str {
    return &text as &str // error: view points to an owned parameter dropped at return
}
```
